use crate::app_data::AppData;
use crate::errors::UserError;
use actix_session::Session;
use actix_web::{get, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

#[post("/")]
async fn create_link(
    app: web::Data<AppData>,
    body: web::Json<api::CreateMagicLinkData>,
) -> Result<HttpResponse, UserError> {
    let actions = app.as_magic_link_actions();

    let data = body.into_inner();
    actions.create_link(&data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/login")]
async fn login(
    app: web::Data<AppData>,
    req: HttpRequest,
    session: Session,
    params: web::Query<api::MagicLinkLoginData>,
) -> Result<HttpResponse, UserError> {
    let req_addr = req.peer_addr().map(|addr| addr.ip());
    if let Some(addr) = req_addr {
        app.ensure_not_tor_ip(&addr).await?;
    }

    let data = params.into_inner();
    let actions = app.as_magic_link_actions();
    let user = actions.login(&data.username, &data.link_id).await?;

    let helper = app.as_login_helper();
    helper.login(session, &user, data.client_id).await?;

    if let Some(url) = data.redirect_uri {
        Ok(HttpResponse::Found()
            .insert_header(("Location", url.as_str()))
            .finish())
    } else {
        Ok(HttpResponse::Ok().body(format!("Logged in as {}", user.username)))
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_link).service(login);
}

#[cfg(test)]
mod tests {
    use actix_web::{http, test, App};
    use netsblox_cloud_common::{Group, MagicLink, User};

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    async fn test_login_banned() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let l1 = MagicLink::new(user.email.clone());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_magic_links(&[l1.clone()])
            .with_banned_users(&[user.username.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/login?linkId={}&username=user", &l1.id.as_str()))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }
}
