use actix_web::http::header;
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use derive_more::Display;
use mongodb::bson::doc;
use netsblox_cloud_common::api;
use serde::Deserialize;

use crate::app_data::AppData;
use crate::common::api::oauth;
use crate::errors::{InternalError, UserError};
use crate::oauth::actions::OAuthActions;
use crate::{auth, utils};

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
    let auth_eu = if let Some(username) = utils::get_username(&req) {
        auth::try_edit_user(&app, &req, None, &username).await
    } else {
        Err(UserError::LoginRequiredError)
    };

    let response = if let Ok(auth_eu) = auth_eu {
        let actions: OAuthActions = app.as_oauth_actions();
        let html = actions
            .render_auth_page(&auth_eu, &params.client_id)
            .await?;

        HttpResponse::Ok()
            .content_type(header::ContentType::html())
            .body(html)
    } else {
        let url = app
            .settings
            .login_url
            .as_ref()
            .ok_or(UserError::LoginRequiredError)?;

        HttpResponse::Found()
            .insert_header(("Location", url.as_str()))
            .finish()
    };

    Ok(response)
}

#[derive(Deserialize)]
pub(crate) struct AuthorizeClientParams {
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

    let actions: OAuthActions = app.as_oauth_actions();
    let url = actions.authorize(&auth_eu, &params.into_inner()).await?;
    let response = HttpResponse::Found()
        .insert_header(("Location", url.as_str()))
        .finish();

    Ok(response)
}

#[post("/token/")]
async fn create_token(
    app: web::Data<AppData>,
    params: web::Json<api::oauth::CreateTokenParams>,
) -> Result<HttpResponse, UserError> {
    let actions: OAuthActions = app.as_oauth_actions();
    let token = actions.create_token(params.into_inner()).await?;

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
    let auth_cc = auth::try_manage_client(&app, &req).await?;

    let actions: OAuthActions = app.as_oauth_actions();
    let client = actions.create_client(&auth_cc, &params.name).await?;
    Ok(HttpResponse::Ok().json(client))
}

#[get("/clients/")]
async fn list_clients(
    app: web::Data<AppData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_cc = auth::try_manage_client(&app, &req).await?;

    let actions: OAuthActions = app.as_oauth_actions();
    let clients = actions.list_clients(&auth_cc).await?;

    Ok(HttpResponse::Ok().json(clients))
}

#[delete("/clients/{client_id}")]
async fn remove_client(
    app: web::Data<AppData>,
    path: web::Path<(oauth::ClientId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_cc = auth::try_manage_client(&app, &req).await?;
    let (client_id,) = path.into_inner();

    let actions: OAuthActions = app.as_oauth_actions();
    let client = actions.delete_client(&auth_cc, &client_id).await?;

    Ok(HttpResponse::Ok().json(client))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(authorization_page)
        .service(authorize_client)
        .service(create_token)
        .service(create_client)
        .service(list_clients)
        .service(remove_client);
}
