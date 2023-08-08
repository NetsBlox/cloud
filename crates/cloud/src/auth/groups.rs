use crate::app_data::AppData;
use actix_web::HttpRequest;
use netsblox_cloud_common::api;

use crate::errors::UserError;

pub(crate) struct ViewGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_view_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<ViewGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct EditGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_edit_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: Option<&api::GroupId>,
) -> Result<EditGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct DeleteGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_delete_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, http, test, web, App, HttpResponse};
    use netsblox_cloud_common::{api, Group, User};

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

    #[get("/test")]
    async fn edit_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_edit_group(&app, &req, Some(&group_id)).await?;
        Ok(HttpResponse::Ok().finish())
    }
}
