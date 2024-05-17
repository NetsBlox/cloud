use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use actix_web::{delete, get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

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

#[get("/user/{owner}")]
async fn view_galleries(
    app: web::Data<AppData>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let owner = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.view_galleries(&auth_eu).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/id/{id}")]
async fn view_gallery(
    app: web::Data<AppData>,
    path: web::Path<api::GalleryId>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let id = path.into_inner();
    let auth_vgal = auth::try_view_gallery(&app, &req, &id).await?;

    Ok(HttpResponse::Ok().json(auth_vgal.metadata))
}

#[patch("/id/{id}")]
async fn change_gallery(
    app: web::Data<AppData>,
    path: web::Path<api::GalleryId>,
    body: web::Json<api::ChangeGalleryData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let id = path.into_inner();
    let try_change = body.into_inner();
    let auth_egal = auth::try_edit_gallery(&app, &req, &id).await?;
    let actions = app.as_gallery_actions();
    let metadata = actions.change_gallery(&auth_egal, try_change).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[delete("/id/{id}")]
async fn delete_gallery(
    app: web::Data<AppData>,
    path: web::Path<api::GalleryId>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let id = path.into_inner();
    let auth_dgal = auth::try_delete_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.delete_gallery(&auth_dgal).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[post("/id/{id}")]
async fn add_gallery_project(
    app: web::Data<AppData>,
    path: web::Path<api::GalleryId>,
    body: web::Json<api::CreateGalleryProjectData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let id = path.into_inner();
    let data = body.into_inner();

    let auth_ap = auth::try_add_gallery_project(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let project = actions.add_gallery_project(&auth_ap, data).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[get("/id/{id}/project/{prid}")]
async fn view_gallery_project(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid) = path.into_inner();
    let auth_vgal = auth::try_view_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let project = actions.get_gallery_project(&auth_vgal, &prid).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[get("/id/{id}/project/{prid}/thumbnail")]
async fn view_gallery_project_thumbnail(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId)>,
    params: web::Query<api::ThumbnailParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid) = path.into_inner();
    let auth_vgal = auth::try_view_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let thumbnail = actions
        .get_gallery_project_thumbnail(&auth_vgal, &prid, params.aspect_ratio)
        .await?;

    Ok(HttpResponse::Ok().content_type("image/png").body(thumbnail))
}

#[get("/id/{id}/projects")]
async fn view_gallery_projects(
    app: web::Data<AppData>,
    path: web::Path<api::GalleryId>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let id = path.into_inner();
    let auth_vgal = auth::try_view_gallery(&app, &req, &id).await?;

    let actions = app.as_gallery_actions();
    let all_projects = actions.get_all_gallery_projects(&auth_vgal).await?;

    Ok(HttpResponse::Ok().json(all_projects))
}

#[patch("/id/{id}/project/{prid}")]
async fn add_gallery_project_version(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId)>,
    body: web::Json<String>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid) = path.into_inner();
    let data = body.into_inner();

    let auth_ep = auth::try_edit_gallery_project(&app, &req, &id, &prid).await?;

    let actions = app.as_gallery_actions();
    let project = actions.add_gallery_project_version(&auth_ep, data).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[get("/id/{id}/projectid/{prid}/xml")]
async fn view_gallery_project_xml(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid) = path.into_inner();
    let auth_vgalp = auth::try_view_gallery_project(&app, &req, &id, &prid).await?;

    let actions = app.as_gallery_actions();
    let project_xml = actions.get_gallery_project_xml(&auth_vgalp).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(project_xml))
}

#[get("/id/{id}/projectid/{prid}/version/{index}/xml")]
async fn view_gallery_project_xml_version(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId, usize)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid, index) = path.into_inner();
    let auth_vgalp = auth::try_view_gallery_project(&app, &req, &id, &prid).await?;

    let actions = app.as_gallery_actions();
    let project_xml = actions
        .get_gallery_project_xml_version(&auth_vgalp, index)
        .await?;

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(project_xml))
}

#[delete("/id/{id}/projectid/{prid}/xml")]
async fn delete_gallery_project(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid) = path.into_inner();
    let auth_dp = auth::try_delete_gallery_project(&app, &req, &id, &prid).await?;

    let actions = app.as_gallery_actions();
    let project = actions.remove_project_in_gallery(&auth_dp).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[delete("/id/{id}/projectid/{prid}/version/{index}/xml")]
async fn delete_gallery_project_version(
    app: web::Data<AppData>,
    path: web::Path<(api::GalleryId, api::ProjectId, usize)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id, prid, index) = path.into_inner();
    let auth_dp = auth::try_delete_gallery_project(&app, &req, &id, &prid).await?;

    let actions = app.as_gallery_actions();
    let project = actions
        .remove_project_version_in_gallery(&auth_dp, index)
        .await?;

    Ok(HttpResponse::Ok().json(project))
}

// TODO: Create endpoints for the other operations that need to be supported
// (make a function - like above - then add them to `config` - like below)

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_gallery);
    cfg.service(view_galleries);
    cfg.service(view_gallery);
    cfg.service(change_gallery);
    cfg.service(delete_gallery);
    cfg.service(view_gallery_project);
    cfg.service(view_gallery_projects);
    cfg.service(add_gallery_project);
    cfg.service(add_gallery_project_version);
    cfg.service(view_gallery_project_xml);
    cfg.service(delete_gallery_project);
    cfg.service(delete_gallery_project_version);
}

// use tests that tests functionality of the code in this file.
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
    async fn test_create_gallery_bad_name() {
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
                    .uri(&format!("/user/{}/{}", &user.username, "fuck"))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_ne!(response.status(), http::StatusCode::OK);
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
