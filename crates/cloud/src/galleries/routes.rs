use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use actix_web::{delete, get, patch, post, HttpRequest};
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

#[get("/id/{id}")]
async fn view_gallery(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_dgal = auth::try_view_gallery(&app, &req, &id).await?;

    Ok(HttpResponse::Ok().json(auth_dgal.metadata))
}

#[post("/id/{id}/projects/")]
async fn create_gallery_project() {
    todo!();
}

//FIXME: this function should return all projects in the gallery.
#[get("/id/{id}/projects/")]
async fn view_gallery_projects(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_dgal = auth::try_view_gallery(&app, &req, &id).await?;

    Ok(HttpResponse::Ok().json(auth_dgal.metadata))
}

#[get("/id/{id}/projects/xml")]
async fn view_gallery_project_xml() {
    todo!("return the xml string");
}

#[patch("/id/{id}/name/{name}")]
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

// acceptable state values are public/pu/1 or private/pr/0
#[patch("/id/{id}/state/{state}")]
async fn change_gallery_state(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, state) = path.into_inner();
    let auth_egal = auth::try_edit_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.rename_gallery(&auth_egal, &state).await?;

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
    cfg.service(view_gallery);
    cfg.service(view_gallery_projects);
    cfg.service(rename_gallery);
    cfg.service(delete_gallery);
}

#[cfg(test)]
mod tests {
    use crate::test_utils;
    use actix_web::{http, test, web, App};
    // use mongodb::bson::doc;
    use netsblox_cloud_common::{
        api::{self, UserRole},
        Gallery, User,
    };

    use super::*;

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

                let req = test::TestRequest::post()
                    .uri(&format!("/user/{}/{}", &user.username, "gallery"))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
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

                let req = test::TestRequest::post()
                    .uri(&format!("/user/{}/{}", &user.username, "gallery"))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_ne!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_admin() {
        let user: User = api::NewUser {
            username: "user2".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), admin.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/user/{}/{}", &user.username, "gallery"))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "owner".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&owner.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }
}
