use actix_session::Session;
use actix_web::http::header;
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use derive_more::Display;
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use passwords::PasswordGenerator;
use serde::Deserialize;
use std::time::SystemTime;
use uuid::Uuid;

use crate::app_data::AppData;
use crate::auth;
use crate::common::api::oauth;
use crate::common::{OAuthClient, OAuthToken};
use crate::errors::{InternalError, OAuthFlowError, UserError};
use crate::oauth::actions::OAuthActions;
use crate::utils::sha512;

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
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let session = req.get_session();
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
pub(super) struct AuthorizeClientParams {
    pub(super) client_id: oauth::ClientId,
    pub(super) client_secret: String,
    pub(super) redirect_uri: Option<String>,
    pub(super) error: Option<String>,
    pub(super) error_description: Option<String>,
    pub(super) state: String,
}

#[post("/{user}/code")]
async fn authorize_client(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    params: web::Query<AuthorizeClientParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: OAuthActions = app.into();
    let url = actions.authorize(&auth_eu, &params.into_inner()).await?;
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
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let query = doc! {"name": &params.name};

    let secret = PasswordGenerator::new()
        .length(12)
        .spaces(false)
        .exclude_similar_characters(true)
        .generate_one()
        .map_err(|_err| InternalError::PasswordGenerationError)?;

    let client = OAuthClient::new(params.name.clone(), secret.clone());
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
            secret,
        }))
    }
}

#[get("/clients/")]
async fn list_clients(app: web::Data<AppData>) -> Result<HttpResponse, UserError> {
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
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let (client_id,) = path.into_inner();
    let query = doc! {"id": client_id};
    app.oauth_clients
        .find_one_and_delete(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::OAuthClientNotFoundError)?;

    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(authorization_page)
        .service(authorize_client)
        .service(create_token)
        .service(create_client)
        .service(list_clients)
        .service(remove_client);
}
