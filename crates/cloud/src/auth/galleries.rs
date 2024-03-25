use crate::{app_data::AppData, errors::InternalError};
use actix_web::HttpRequest;
use mongodb::bson::doc;

use netsblox_cloud_common::api::{self, PublishState};

use crate::errors::UserError;

// Permissions on galleries
pub(crate) struct ViewGallery {
    pub(crate) metadata: api::Gallery,
    _private: (),
}
impl ViewGallery {
    pub(crate) fn test(gallery: &api::Gallery) -> ViewGallery {
        ViewGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

pub(crate) struct EditGallery {
    pub(crate) metadata: api::Gallery,
    _private: (),
}
impl EditGallery {
    pub(crate) fn test(gallery: &api::Gallery) -> EditGallery {
        EditGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}
pub(crate) struct DeleteGallery {
    pub(crate) metadata: api::Gallery,
    _private: (),
}
impl DeleteGallery {
    pub(crate) fn test(gallery: &api::Gallery) -> DeleteGallery {
        DeleteGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

// functions to try to obtain the given permissions
pub(crate) async fn try_view_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<ViewGallery, UserError> {
    let query = doc! {"id": id};
    let result = app
        .galleries
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GalleryNotFoundError)?;

    if result.state != PublishState::Public {
        super::try_edit_user(app, req, None, &result.owner).await?;
    }

    Ok(ViewGallery {
        metadata: result.into(),
        _private: (),
    })
}

pub(crate) async fn try_edit_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<EditGallery, UserError> {
    let query = doc! {"id": id};
    let result = app
        .galleries
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GalleryNotFoundError)?;

    // return early with error if user is not owner
    super::try_edit_user(app, req, None, &result.owner).await?;

    // else, return permissions EditGallery
    Ok(EditGallery {
        metadata: result.into(),
        _private: (),
    })
}

/// Try to obtain permissions to delete a gallery. Only gallery owners
/// are allowed to delete the gallery.
pub(crate) async fn try_delete_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<DeleteGallery, UserError> {
    // for now you can only delete the gallery if you are allowed to edit it
    try_edit_gallery(app, req, id)
        .await
        .map(|eg| DeleteGallery {
            metadata: eg.metadata.clone(),
            _private: (),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, http, test, web, App, HttpResponse};
    use netsblox_cloud_common::{Gallery, User};

    use crate::test_utils;

    #[actix_web::test]
    async fn test_try_view_private_gallery_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Public);

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_public_gallery_other() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Public);

        test_utils::setup()
            .with_users(&[owner.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_private_gallery_other() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_private_gallery_admin() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
            role: Some(api::UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), admin.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_gallery_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);
        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_gallery_other() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_gallery_admin() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
            role: Some(api::UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), admin.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_delete_gallery_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(delete_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_delete_gallery_other() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), other.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(delete_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_delete_gallery_admin() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
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
            role: Some(api::UserRole::Admin),
        }
        .into();
        let gallery = Gallery::new("owner".into(), "gallery".into(), api::PublishState::Private);

        test_utils::setup()
            .with_users(&[owner.clone(), admin.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(delete_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/test")
                    .set_json(gallery.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[get("/test")]
    async fn view_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        gallery: web::Json<api::GalleryId>,
    ) -> Result<HttpResponse, UserError> {
        let gallery_id = gallery.into_inner();
        try_view_gallery(&app, &req, &gallery_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn edit_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        gallery: web::Json<api::GalleryId>,
    ) -> Result<HttpResponse, UserError> {
        let gallery_id = gallery.into_inner();
        try_edit_gallery(&app, &req, &gallery_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn delete_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        gallery: web::Json<api::GalleryId>,
    ) -> Result<HttpResponse, UserError> {
        let gallery_id = gallery.into_inner();
        try_delete_gallery(&app, &req, &gallery_id).await?;
        Ok(HttpResponse::Ok().finish())
    }
}
