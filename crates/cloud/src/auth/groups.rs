use crate::{app_data::AppData, errors::InternalError, utils};
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::api;
use netsblox_macros::Witness;

use crate::errors::UserError;

// Permissions on groups
#[derive(Witness)]
pub(crate) struct ViewGroup {
    pub(crate) id: api::GroupId,
    // TODO: can the Witness macro add the following field?
    _private: (),
}

// TODO: the macro, Witness, should generate this
impl ViewGroup {
    fn new(id: api::GroupId) -> Self {
        Self { id, _private: () }
    }

    #[cfg(test)]
    pub(crate) fn mock(id: api::GroupId) -> Self {
        Self::new(id)
    }
}

pub(crate) struct EditGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) struct DeleteGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

// functions to try to obtain the given permissions
pub(crate) async fn try_view_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<ViewGroup, UserError> {
    // for now you can only view the group if you are allowed to edit it
    try_edit_group(app, req, group_id)
        .await
        .map(|eg| ViewGroup::new(eg.id))
}

pub(crate) async fn try_edit_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<EditGroup, UserError> {
    let query = doc! {"id": group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;

    let authorized = utils::get_authorized_host(&app.authorized_services, req)
        .await
        .ok()
        .flatten()
        .is_some();

    if !authorized {
        let _auth = super::try_edit_user(app, req, None, &group.owner).await?;
    }

    Ok(EditGroup {
        id: group_id.to_owned(),
        _private: (),
    })
}

/// Try to obtain permissions to delete the given group. Only group owners
/// (or those who can edit group owners) are allowed to delete the group.
pub(crate) async fn try_delete_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    let query = doc! {"id": group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;
    let _auth = super::try_edit_user(app, req, None, &group.owner).await?;

    Ok(DeleteGroup {
        id: group_id.to_owned(),
        _private: (),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, http, test, web, App, HttpResponse};
    use netsblox_cloud_common::{
        api::{self, UserRole},
        Group, User,
    };

    use crate::test_utils;

    #[actix_web::test]
    async fn test_try_edit_group_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_groups(&[group.clone()])
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
                    .set_json(group.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_group_other() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner, other.clone()])
            .with_groups(&[group.clone()])
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
                    .set_json(group.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_group_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner, admin.clone()])
            .with_groups(&[group.clone()])
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
                    .set_json(group.id)
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
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_view_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn edit_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_edit_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn delete_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_delete_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }
}
