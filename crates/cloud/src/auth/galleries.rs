use crate::{app_data::AppData, errors::InternalError};
use actix_web::HttpRequest;
use mongodb::bson::doc;

use crate::errors::UserError;
use netsblox_cloud_common::api::{self, PublishState};
use netsblox_cloud_common::{Gallery, GalleryProjectMetadata};

pub(crate) struct ViewGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}
#[cfg(test)]
impl ViewGallery {
    pub(crate) fn test(gallery: &Gallery) -> ViewGallery {
        ViewGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

pub(crate) struct EditGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}
#[cfg(test)]
impl EditGallery {
    pub(crate) fn test(gallery: &Gallery) -> EditGallery {
        EditGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

pub(crate) struct DeleteGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}
#[cfg(test)]
impl DeleteGallery {
    pub(crate) fn test(gallery: &Gallery) -> DeleteGallery {
        DeleteGallery {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

/// witness for authorization for adding projects to a gallery
pub(crate) struct AddGalleryProject {
    pub(crate) metadata: Gallery,
    _private: (),
}
#[cfg(test)]
impl AddGalleryProject {
    pub(crate) fn test(gallery: &Gallery) -> AddGalleryProject {
        AddGalleryProject {
            metadata: gallery.clone(),
            _private: (),
        }
    }
}

/// witness for authorization for deleting a specific project in a gallery
pub(crate) struct DeleteGalleryProject {
    pub(crate) metadata: Gallery,
    pub(crate) project: GalleryProjectMetadata,
    _private: (),
}
#[cfg(test)]
impl DeleteGalleryProject {
    pub(crate) fn test(
        gallery: &Gallery,
        project: &GalleryProjectMetadata,
    ) -> DeleteGalleryProject {
        DeleteGalleryProject {
            metadata: gallery.clone(),
            project: project.clone(),
            _private: (),
        }
    }
}

/// functions to try to obtain the given permissions
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
        metadata: result,
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

    super::try_edit_user(app, req, None, &result.owner).await?;

    // is there a presedent for a valid name?
    Ok(EditGallery {
        metadata: result,
        _private: (),
    })
}

/// Try to obtain permissions to delete a gallery.
pub(crate) async fn try_delete_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<DeleteGallery, UserError> {
    try_edit_gallery(app, req, id)
        .await
        .map(|eg| DeleteGallery {
            metadata: eg.metadata.clone(),
            _private: (),
        })
}

// TODO: Add checking for group memebership.
// Must add group id to gallery definition
// this is so a collection of people can "control a gallery"
pub(crate) async fn try_add_gallery_project(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<AddGalleryProject, UserError> {
    try_edit_gallery(app, req, id)
        .await
        .map(|eg| AddGalleryProject {
            metadata: eg.metadata.clone(),
            _private: (),
        })
}

pub(crate) async fn try_delete_gallery_project(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
    prid: &api::ProjectId,
) -> Result<DeleteGalleryProject, UserError> {
    let gallery = try_edit_gallery(app, req, id).await?.metadata;

    let is_gal_owner = super::try_edit_user(app, req, None, &gallery.owner)
        .await
        .is_ok();

    let query = doc! {"galleryId": prid};
    let proj = app
        .gallery_projects
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GalleryNotFoundError)?;

    if is_gal_owner {
        // if power over owner
        Ok(DeleteGalleryProject {
            metadata: gallery,
            project: proj,
            _private: (),
        })
    } else {
        // or project owner
        super::try_edit_user(app, req, None, &proj.owner).await?;
        Ok(DeleteGalleryProject {
            metadata: gallery,
            project: proj,
            _private: (),
        })
    }
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
    #[ignore]
    async fn test_try_view_gallery_group() {
        todo!();
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
        let change = api::ChangeGalleryData {
            name: Some("gallerytwo".into()),
            state: Some(PublishState::Private),
        };
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
                    .set_json((gallery.id, change))
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

        let change = api::ChangeGalleryData {
            name: Some("gallerytwo".into()),
            state: Some(PublishState::Private),
        };

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
                    .set_json((gallery.id, change))
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
        let change = api::ChangeGalleryData {
            name: Some("gallerytwo".into()),
            state: Some(PublishState::Private),
        };
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
                    .set_json((gallery.id, change))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_edit_gallery_group() {
        todo!();
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

    #[actix_web::test]
    #[ignore]
    async fn test_try_delete_gallery_group() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_add_gallery_project_owner() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_add_gallery_project_other() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_add_gallery_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_add_gallery_project_group() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_delete_gallery_project_owner() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_delete_gallery_project_other() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_delete_gallery_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_delete_gallery_project_group() {
        todo!();
    }

    #[get("/test")]
    // Possibility for macro to replace the view test definition
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
        gallery: web::Json<(api::GalleryId, api::ChangeGalleryData)>,
    ) -> Result<HttpResponse, UserError> {
        let (gallery_id, _change) = gallery.into_inner();
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
