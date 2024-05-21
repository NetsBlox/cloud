use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use actix_web::{delete, get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

#[post("/")]
async fn create_gallery(
    app: web::Data<AppData>,
    body: web::Json<api::CreateGalleryData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let data = body.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &data.owner).await?;

    let actions = app.as_gallery_actions();
    let metadata = actions.create_gallery(&auth_eu, data).await?;

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

    let auth_ap = auth::try_add_gallery_project(&app, &req, &id, &data).await?;

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

#[post("/id/{id}/project/{prid}")]
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

#[get("/id/{id}/project/{prid}/xml")]
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

#[get("/id/{id}/project/{prid}/version/{index}/xml")]
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

#[delete("/id/{id}/project/{prid}")]
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

#[delete("/id/{id}/project/{prid}/version/{index}")]
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

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_gallery);
    cfg.service(view_galleries);
    cfg.service(view_gallery);
    cfg.service(change_gallery);
    cfg.service(delete_gallery);
    cfg.service(add_gallery_project);
    cfg.service(view_gallery_project);
    cfg.service(view_gallery_project_thumbnail);
    cfg.service(view_gallery_projects);
    cfg.service(add_gallery_project_version);
    cfg.service(view_gallery_project_xml);
    cfg.service(view_gallery_project_xml_version);
    cfg.service(delete_gallery_project);
    cfg.service(delete_gallery_project_version);
}

// use tests that tests functionality of the code in this file.
#[cfg(test)]
mod tests {
    use crate::test_utils;
    use crate::utils::get_thumbnail;

    use actix_web::{http, test, web, App};
    // use mongodb::bson::doc;
    use netsblox_cloud_common::{
        api::{self, UserRole},
        Gallery, GalleryProjectMetadata, User,
    };

    use super::*;

    #[actix_web::test]
    async fn test_create_gallery_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let data = api::CreateGalleryData {
            owner: "user".into(),
            name: "gallery".into(),
            state: api::PublishState::Private,
        };

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
                    .uri("/")
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_duplicate() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        let data = api::CreateGalleryData {
            owner: "user".into(),
            name: "mygallery".into(),
            state: api::PublishState::Private,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/")
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_owner_bad_name() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let data = api::CreateGalleryData {
            owner: "user".into(),
            name: "FUCK".into(),
            state: api::PublishState::Private,
        };

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
                    .uri("/")
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_other_403() {
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
        let data = api::CreateGalleryData {
            owner: "user".into(),
            name: "gallery".into(),
            state: api::PublishState::Private,
        };

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
                    .uri("/")
                    .cookie(test_utils::cookie::new(&other.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let data = api::CreateGalleryData {
            owner: "user".into(),
            name: "gallery".into(),
            state: api::PublishState::Private,
        };
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
                    .uri("/")
                    .cookie(test_utils::cookie::new(&admin.username))
                    .set_json(&data)
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_galleries_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        let gallery2 = Gallery::new(
            "user".into(),
            "mygallery2".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone(), gallery2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}", &user.username))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _galleries: Vec<Gallery> = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_galleries_other() {
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

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        let gallery2 = Gallery::new(
            "user".into(),
            "mygallery2".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .with_galleries(&[gallery.clone(), gallery2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}", &user.username))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_galleries_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        let gallery2 = Gallery::new(
            "user".into(),
            "mygallery2".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone(), admin.clone()])
            .with_galleries(&[gallery.clone(), gallery2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}", &user.username))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let _galleries: Vec<Gallery> = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }
    #[actix_web::test]
    async fn test_view_gallery_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_other_private_403() {
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

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_other_public() {
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

        let gallery = Gallery::new("user".into(), "mygallery".into(), api::PublishState::Public);

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_change_gallery_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::ChangeGalleryData {
            name: Some("mygallery2".into()),
            state: Some(api::PublishState::Public),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let gallery: Gallery = test::call_and_read_body_json(&app, req).await;
                assert_eq!(gallery.name, data.name.unwrap());
                assert_eq!(gallery.state, data.state.unwrap());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_change_gallery_other_403() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::ChangeGalleryData {
            name: Some("mygallery2".into()),
            state: Some(api::PublishState::Public),
        };

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_change_gallery_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::ChangeGalleryData {
            name: Some("mygallery2".into()),
            state: Some(api::PublishState::Public),
        };

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &gallery.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .set_json(&data)
                    .to_request();

                let gallery: Gallery = test::call_and_read_body_json(&app, req).await;
                assert_eq!(gallery.name, data.name.unwrap());
                assert_eq!(gallery.state, data.state.unwrap());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
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
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_other_403() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
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
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }
    #[actix_web::test]
    async fn test_delete_gallery_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
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
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let _gallery: Gallery = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::CreateGalleryProjectData {
            owner: "user".into(),
            name: "gallery_project".into(),
            project_xml: "xml".into(),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", gallery.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(data.name, project.name);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_bad_project_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::CreateGalleryProjectData {
            owner: "other".into(),
            name: "gallery_project".into(),
            project_xml: "xml".into(),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", gallery.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_other_403() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::CreateGalleryProjectData {
            owner: "other".into(),
            name: "gallery_project".into(),
            project_xml: "xml".into(),
        };

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(
            "user".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let data = api::CreateGalleryProjectData {
            owner: "admin".into(),
            name: "gallery_project".into(),
            project_xml: "xml".into(),
        };

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", gallery.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .set_json(&data)
                    .to_request();

                let project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(data.name, project.name);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(project.versions.len(), 3);
                assert_eq!(response.id, project.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_other_private_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_other_public() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let other: User = api::NewUser {
            username: "parrytheplatypus".into(),
            email: "perry@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(project.versions.len(), 3);
                assert_eq!(response.id, project.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let response: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(project.versions.len(), 3);
                assert_eq!(response.id, project.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_thumbnail_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let thumbnail_xml = format!(
            "<thumbnail>{}</thumbnail>",
            test_utils::gallery_projects::TestThumbnail::new(additional_versions).as_str()
        );
        let exp_thumbnail =
            get_thumbnail(&thumbnail_xml, None).expect("failed to get expected thumbnail");

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/thumbnail",
                        gallery.id, project.id,
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let thumbnail = test::call_and_read_body(&app, req).await;
                assert_eq!(thumbnail, exp_thumbnail);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_thumbnail_owner_resize() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 1;
        let aspect_ratio = 2.0;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let thumbnail_xml = format!(
            "<thumbnail>{}</thumbnail>",
            test_utils::gallery_projects::TestThumbnail::new(additional_versions).as_str()
        );
        let exp_thumbnail = get_thumbnail(&thumbnail_xml, Some(aspect_ratio))
            .expect("failed to get expected thumbnail");

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/thumbnail?aspectRatio={}",
                        gallery.id, project.id, aspect_ratio
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let thumbnail = test::call_and_read_body(&app, req).await;
                assert_eq!(thumbnail, exp_thumbnail);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_thumbnail_other_private_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 4;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/thumbnail",
                        gallery.id, project.id,
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_thumbnail_other_public() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 3;

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let thumbnail_xml = format!(
            "<thumbnail>{}</thumbnail>",
            test_utils::gallery_projects::TestThumbnail::new(additional_versions).as_str()
        );
        let exp_thumbnail =
            get_thumbnail(&thumbnail_xml, None).expect("failed to get expected thumbnail");

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/thumbnail",
                        gallery.id, project.id,
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let thumbnail = test::call_and_read_body(&app, req).await;
                assert_eq!(thumbnail, exp_thumbnail);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_thumbnail_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 7;

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let thumbnail_xml = format!(
            "<thumbnail>{}</thumbnail>",
            test_utils::gallery_projects::TestThumbnail::new(additional_versions).as_str()
        );
        let exp_thumbnail =
            get_thumbnail(&thumbnail_xml, None).expect("failed to get expected thumbnail");

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/thumbnail",
                        gallery.id, project.id,
                    ))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let thumbnail = test::call_and_read_body(&app, req).await;
                assert_eq!(thumbnail, exp_thumbnail);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_projects_owner() {
        let (owner, name) = ("user", "mygallery");
        let (project_name1, project_name2) = ("myproject", "mysecondproject");

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project1 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name1, 3);

        let project2 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name2, 1);

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project1.clone(), project2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/projects", gallery.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let projects: Vec<GalleryProjectMetadata> =
                    test::call_and_read_body_json(&app, req).await;

                for project in projects {
                    assert!(project.id == project1.id || project.id == project2.id);
                }
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_projects_other_private_403() {
        let (owner, name) = ("user", "mygallery");
        let (project_name1, project_name2) = ("myproject", "mysecondproject");

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project1 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name1, 3);

        let project2 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name2, 1);

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project1.clone(), project2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/projects", gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_projects_other_public() {
        let (owner, name) = ("user", "mygallery");
        let (project_name1, project_name2) = ("myproject", "mysecondproject");

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project1 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name1, 3);

        let project2 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name2, 1);

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project1.clone(), project2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/projects", gallery.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let projects: Vec<GalleryProjectMetadata> =
                    test::call_and_read_body_json(&app, req).await;

                for project in projects {
                    assert!(project.id == project1.id || project.id == project2.id);
                }
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_projects_admin() {
        let (owner, name) = ("user", "mygallery");
        let (project_name1, project_name2) = ("myproject", "mysecondproject");

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project1 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name1, 3);

        let project2 =
            test_utils::gallery_projects::with_version_count(&gallery, owner, project_name2, 1);

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project1.clone(), project2.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/projects", gallery.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let projects: Vec<GalleryProjectMetadata> =
                    test::call_and_read_body_json(&app, req).await;

                for project in projects {
                    assert!(project.id == project1.id || project.id == project2.id);
                }
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_version_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 0;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let xml = "data";

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(xml)
                    .to_request();

                let project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(project.versions.len(), 2);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_version_owner_no_s3_connection() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 0;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let xml = "data";

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .without_s3()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(xml)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_version_other_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 0;

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let xml = "data";

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .set_json(xml)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_add_gallery_project_version_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 0;

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        let xml = "data";

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .set_json(xml)
                    .to_request();

                let project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(project.versions.len(), 2);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 3;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}/xml", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, additional_versions);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_other_private_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 31;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}/xml", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_other_private_proj_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 31;
        let proj_owner = "other";
        let other: User = api::NewUser {
            username: proj_owner.into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            proj_owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}/xml", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, additional_versions);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_other_public() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 25;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}/xml", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, additional_versions);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 17;
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/project/{}/xml", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, additional_versions);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_version_xml_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 3;
        let get_version = 1;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, get_version);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_version_xml_owner_outofbounds_404() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 3;
        let get_version = 5;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_version_other_private_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 31;
        let get_version = 10;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_version_other_private_proj_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 31;
        let get_version = 10;

        let proj_owner = "other";
        let other: User = api::NewUser {
            username: proj_owner.into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            proj_owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, get_version);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_version_other_public() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 14;
        let get_version = 8;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, get_version);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_gallery_project_xml_version_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 17;
        let get_version = 2;

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}/xml",
                        gallery.id, project.id, get_version
                    ))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let xml = test::call_and_read_body(&app, req).await;
                let xml_str = String::from_utf8(xml.to_vec()).unwrap();
                let version: usize = test_utils::gallery_projects::get_version(&xml_str);
                assert_eq!(version, get_version);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_project_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let deleted_project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(deleted_project.id, project.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_project_other_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();
                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_project_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 2;
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}/project/{}", gallery.id, project.id))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let deleted_project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                assert_eq!(deleted_project.id, project.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_project_version_owner() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 8;
        let delete_index = 6;

        let user: User = api::NewUser {
            username: owner.into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}",
                        gallery.id, project.id, delete_index
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response_project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                for (index, version) in response_project.versions.iter().enumerate() {
                    if index == delete_index {
                        assert!(version.deleted);
                    } else {
                        assert!(!version.deleted);
                    }
                }
            })
            .await;
    }
    #[actix_web::test]
    async fn test_delete_gallery_project_version_other_403() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 8;
        let delete_index = 6;

        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Public);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[other.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}",
                        gallery.id, project.id, delete_index
                    ))
                    .cookie(test_utils::cookie::new(&other.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_gallery_project_version_admin() {
        let (owner, name, project_name) = ("user", "mygallery", "myproject");
        let additional_versions = 8;
        let delete_index = 6;

        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new(owner.into(), name.into(), api::PublishState::Private);

        let project = test_utils::gallery_projects::with_version_count(
            &gallery,
            owner,
            project_name,
            additional_versions,
        );

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_galleries(&[gallery.clone()])
            .with_gallery_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!(
                        "/id/{}/project/{}/version/{}",
                        gallery.id, project.id, delete_index
                    ))
                    .cookie(test_utils::cookie::new(&admin.username))
                    .to_request();

                let response_project: GalleryProjectMetadata =
                    test::call_and_read_body_json(&app, req).await;
                for (index, version) in response_project.versions.iter().enumerate() {
                    if index == delete_index {
                        assert!(version.deleted);
                    } else {
                        assert!(!version.deleted);
                    }
                }
            })
            .await;
    }
}
