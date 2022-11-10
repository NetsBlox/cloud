use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};

use crate::app_data::AppData;
use crate::errors::UserError;

#[get("/")]
async fn authorization_page(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: If not logged in, redirect
    todo!();
}

#[post("/code")]
async fn authorize_client(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    todo!();
}

#[post("/token")]
async fn create_token(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    todo!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(authorization_page)
        .service(authorize_client)
        .service(create_token);
}
