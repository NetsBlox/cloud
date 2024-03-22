use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use actix_web::{delete, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

// Question:
// I could get the owner from the HttpRequest.
// However, this would mean that admins cant create
// Galleries for other users?
#[post("/user/{owner}/{name}")]
async fn create_gallery(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions
        .create_gallery(
            &auth_eu,
            &name,
            netsblox_cloud_common::api::PublishState::Private,
        )
        .await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[patch("/id/{id}/{name}")]
async fn rename_gallery(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, name) = path.into_inner();
    let auth_egal = auth::try_edit_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.rename_gallery(&auth_egal, &name).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

// prefix with user or id to avoid collisions
#[delete("/id/{id}")]
async fn delete_gallery(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_dgal = auth::try_delete_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.delete_gallery(&auth_dgal).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

// TODO: Create endpoints for the other operations that need to be supported
// (make a function - like above - then add them to `config` - like below)

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_gallery);
    cfg.service(rename_gallery);
    cfg.service(delete_gallery);
}

#[cfg(test)]
mod tests {
    use crate::test_utils;
    use actix_web::{test, web, App};
    use netsblox_cloud_common::{api, Gallery, Library, User};

    #[actix_web::test]
    async fn test_create_gallery() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}/", &user.username))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}/", &user.username))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]

    async fn test_create_gallery_admin() {
        todo!("Check that an admin can create a gallery for another user");
    }
}
