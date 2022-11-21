mod html_template;

use actix_session::Session;
use actix_web::http::header;
use actix_web::{delete, get, post, web, HttpResponse};
use derive_more::Display;
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use passwords::PasswordGenerator;
use serde::Deserialize;
use std::time::SystemTime;
use uuid::Uuid;

use crate::app_data::AppData;
use crate::common::api::oauth;
use crate::common::{OAuthClient, OAuthToken};
use crate::errors::{InternalError, OAuthFlowError, UserError};
use crate::users::{ensure_can_edit_user, ensure_is_super_user, sha512};

#[derive(Deserialize)]
struct AuthorizeParams {
    client_id: oauth::ClientId,
}

// TODO: should we define scopes?
// TODO: view username/email
// TODO: view projects and libraries
// TODO: edit projects and libraries

#[derive(Debug, Display)]
pub(crate) enum Scope {
    #[display(fmt = "View created Alexa skills")]
    ViewAlexaSkills,
    #[display(fmt = "Execute blocks on your behalf")]
    ExecuteBlocks,
}

#[get("/authorize")]
async fn authorization_page(
    app: web::Data<AppData>,
    params: web::Query<AuthorizeParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let current_user = session.get::<String>("username").ok().flatten();
    if let Some(username) = current_user {
        let query = doc! {"id": &params.client_id};
        let client = app
            .oauth_clients
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::OAuthClientNotFoundError)?;

        // FIXME: the requestor should pass the scopes
        let scopes = [Scope::ViewAlexaSkills, Scope::ExecuteBlocks];
        let html = html_template::authorize_page(&username, &client.name, &scopes);
        Ok(HttpResponse::Ok()
            .content_type(header::ContentType::html())
            .body(html))
    } else {
        let url = app
            .settings
            .login_url
            .as_ref()
            .ok_or(UserError::LoginRequiredError)?;

        let response = HttpResponse::Found()
            .insert_header(("Location", url.as_str()))
            .finish();

        Ok(response)
    }
}

#[derive(Deserialize)]
struct AuthorizeClientParams {
    client_id: oauth::ClientId,
    client_secret: String,
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
    let hashed_secret = sha512(&params.client_secret);
    let query = doc! {
        "id": &params.client_id,
        "hash": hashed_secret
    };

    let client_exists = app
        .oauth_clients
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .is_some();

    // TODO: is incorrect secret a different error?
    if !client_exists {
        return Err(UserError::OAuthClientNotFoundError);
    }

    let code = oauth::Code {
        id: oauth::CodeId::new(Uuid::new_v4().to_string()),
        username: username.to_owned(),
        client_id: params.client_id.to_owned(),
        redirect_uri: redirect_uri.to_owned(),
        created_at: SystemTime::now(),
    };

    app.oauth_codes
        .insert_one(&code, None)
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

#[post("/token/")]
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

    let token = OAuthToken::new(code.client_id, code.username);
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

#[get("/token/{tokenId}")]
async fn get_token(
    app: web::Data<AppData>,
    path: web::Path<(oauth::TokenId,)>,
) -> Result<HttpResponse, UserError> {
    // TODO: limit the number of requests from a single source?
    // TODO: ensure they are an authorized service host?
    let (token_id,) = path.into_inner();
    let query = doc! {"id": &token_id};
    let token: oauth::Token = app
        .oauth_tokens
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::OAuthTokenNotFoundError)?
        .into();

    Ok(HttpResponse::Ok().json(token))
}

#[post("/clients/")]
async fn create_client(
    app: web::Data<AppData>,
    params: web::Json<oauth::CreateClientData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let query = doc! {"name": &params.name};

    let password = PasswordGenerator::new()
        .length(12)
        .spaces(false)
        .exclude_similar_characters(true)
        .generate_one()
        .map_err(|_err| InternalError::PasswordGenerationError)?;

    let client = OAuthClient::new(params.name.clone(), password.clone());
    let client_id = client.id.clone();

    let update = doc! {"$setOnInsert": client};
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
        Ok(HttpResponse::Ok().json(oauth::CreatedClientData {
            id: client_id,
            password,
        }))
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
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|c| c.into())
        .collect();

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
