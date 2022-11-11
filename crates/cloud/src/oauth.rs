use std::time::SystemTime;

use actix_session::Session;
use actix_web::http::header;
use actix_web::{delete, get, post, web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use netsblox_core::oauth;
use serde::Deserialize;
use uuid::Uuid;

use crate::app_data::AppData;
use crate::errors::{InternalError, OAuthFlowError, UserError};
use crate::users::{ensure_can_edit_user, ensure_is_super_user};

#[derive(Deserialize)]
struct AuthorizeParams {
    client_id: oauth::ClientId,
}

// TODO: should we define scopes?
// TODO: view username/email
// TODO: view projects and libraries
// TODO: edit projects and libraries

#[get("/")]
async fn authorization_page(
    app: web::Data<AppData>,
    params: web::Query<AuthorizeParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: allow authorizing clients for others (potentially useful for classroom use)
    // TODO: If not logged in, redirect
    let logged_in = session.get::<String>("username").ok().is_some();
    if !logged_in {
        let url = app
            .settings
            .login_url
            .as_ref()
            .ok_or(UserError::LoginRequiredError)?;

        let response = HttpResponse::Found()
            .insert_header(("Location", url.as_str()))
            .finish();

        Ok(response)
    } else {
        todo!()
        //params.client_id
        // TODO: look up the client
        // TODO: return the sign up page?
    }
}

#[derive(Deserialize)]
struct AuthorizeClientParams {
    client_id: oauth::ClientId,
    redirect_uri: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    state: String,
}

#[post("/{user}/code")]
async fn authorize_client(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    params: web::Query<AuthorizeClientParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or(OAuthFlowError::InvalidRedirectUrlError)?;
    // TODO: Make sure the uri error is correct

    if let Some(error) = &params.error {
        let url = params
            .error_description
            .as_ref()
            .map(|desc| {
                format!(
                    "{}?error={}&error_description={}",
                    redirect_uri, error, desc
                )
            })
            .unwrap_or_else(|| format!("{}?error={}", redirect_uri, error));

        let response = HttpResponse::Found()
            .insert_header(("Location", url.as_str()))
            .finish();

        return Ok(response);
    }

    // TODO: Check that the client exists
    // TODO: create a new code for the user
    // TODO: return the codeId
    let query = doc! {"id": &params.client_id};
    let client_exists = app
        .oauth_clients
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .is_some();

    if !client_exists {
        return Err(UserError::OAuthClientNotFoundError);
    }

    let query = doc! {
        "username": &username,
        "clientId": &params.client_id,
    };
    let code = oauth::Code {
        id: oauth::CodeId::new(Uuid::new_v4().to_string()),
        username: username.to_owned(),
        client_id: params.client_id.to_owned(),
        redirect_uri: redirect_uri.to_owned(),
        created_at: SystemTime::now(),
    };
    let update = doc! {"$setOnInsert": &code};
    let options = mongodb::options::FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::Before)
        .upsert(true)
        .build();

    let existing_code = app
        .oauth_codes
        .find_one_and_update(query, update, options)
        .await
        // FIXME: we may need to handle these errors differently
        .map_err(InternalError::DatabaseConnectionError)?;

    let url = &format!(
        "{}?code={}&state={}",
        redirect_uri,
        code.id.as_str(),
        params.state
    );
    let response = HttpResponse::Found()
        .insert_header(("Location", url.as_str()))
        .finish();

    Ok(response)
}

#[derive(Deserialize)]
struct CreateTokenParams {
    code: Option<String>,
    redirect_uri: Option<String>,
    grant_type: Option<String>,
}

#[post("/token")]
async fn create_token(
    app: web::Data<AppData>,
    params: web::Json<CreateTokenParams>,
) -> Result<HttpResponse, UserError> {
    let is_valid_grant = params
        .grant_type
        .as_ref()
        .map(|grant_type| grant_type == "authorization_code")
        .unwrap_or(false);

    if !is_valid_grant {
        return Err(OAuthFlowError::InvalidGrantTypeError.into());
    }

    let code_id = params
        .code
        .as_ref()
        .ok_or(OAuthFlowError::NoAuthorizationCodeError)?;
    let redirect_uri = params
        .redirect_uri
        .as_ref()
        .ok_or(OAuthFlowError::InvalidRedirectUrlError)?;

    let query = doc! {"id": &code_id};
    let code = app
        .oauth_codes
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(OAuthFlowError::InvalidAuthorizationCodeError)?;

    if redirect_uri != &code.redirect_uri {
        return Err(OAuthFlowError::InvalidRedirectUrlError.into());
    }

    let token = oauth::Token {
        id: oauth::TokenId::new(Uuid::new_v4().to_string()),
        client_id: code.client_id,
        username: code.username,
        created_at: SystemTime::now(),
    };

    app.oauth_tokens
        .insert_one(&token, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let response = HttpResponse::Ok()
        .insert_header(header::CacheControl(vec![header::CacheDirective::NoStore]))
        .insert_header(("Pragma", "no-cache"))
        .json(token.id);
    Ok(response)
}

#[post("/clients/")]
async fn create_client(
    app: web::Data<AppData>,
    params: web::Json<oauth::CreateClientData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let query = doc! {"name": &params.name};
    let client = oauth::Client {
        id: oauth::ClientId::new(Uuid::new_v4().to_string()),
        name: params.name.clone(),
    };
    let update = doc! {"$setOnInsert": &client};
    let options = mongodb::options::FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::Before)
        .upsert(true)
        .build();

    let existing_client = app
        .oauth_clients
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if existing_client.is_some() {
        Err(UserError::OAuthClientAlreadyExistsError)
    } else {
        Ok(HttpResponse::Ok().json(client.id))
    }
}

#[get("/clients/")]
async fn list_clients(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let cursor = app
        .oauth_clients
        .find(doc! {}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let clients: Vec<oauth::Client> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(HttpResponse::Ok().json(clients))
}

#[delete("/clients/{client_id}")]
async fn remove_client(
    app: web::Data<AppData>,
    path: web::Path<(oauth::ClientId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let (client_id,) = path.into_inner();
    let query = doc! {"id": client_id};
    let del_result = app
        .oauth_clients
        .delete_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if del_result.deleted_count == 0 {
        Err(UserError::OAuthClientNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(authorization_page)
        .service(authorize_client)
        .service(create_token)
        .service(create_client)
        .service(list_clients)
        .service(remove_client);
}
