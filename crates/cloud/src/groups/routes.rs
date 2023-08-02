use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use crate::groups::actions::GroupActions;
use actix_session::Session;
use actix_web::{delete, get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

#[get("/user/{owner}")]
async fn list_groups(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &session, None, &owner).await?;

    let actions: GroupActions = app.into();
    let groups = actions.list_groups(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(groups))
}

#[get("/id/{id}")]
async fn view_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_vg = auth::try_view_group(&app, &session, &id).await?;

    let actions: GroupActions = app.into();
    let group = actions.view_group(&auth_vg).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[get("/id/{id}/members")]
async fn list_members(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let auth_vg = auth::try_view_group(&app, &session, &id).await?;

    let actions: GroupActions = app.into();
    let members = actions.list_members(&auth_vg).await?;

    Ok(HttpResponse::Ok().json(members))
}

// TODO: Should this send the data, too?
#[post("/user/{owner}")]
async fn create_group(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
    body: web::Json<api::CreateGroupData>,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &session, None, &owner).await?;

    let actions: GroupActions = app.into();
    let group = actions.create_group(&auth_eu, &body.name).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[patch("/id/{id}")]
async fn update_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    data: web::Json<api::UpdateGroupData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_eg = auth::try_edit_group(&app, &session, &id).await?;

    let actions: GroupActions = app.into();
    let group = actions.rename_group(&auth_eg, &data.name).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[delete("/id/{id}")]
async fn delete_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let auth_dg = auth::try_delete_group(&app, &session, &id).await?;

    let actions: GroupActions = app.into();
    let group = actions.delete_group(&auth_dg).await?;

    Ok(HttpResponse::Ok().json(group))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_groups)
        .service(view_group)
        .service(list_members)
        .service(update_group)
        .service(delete_group)
        .service(create_group);
}

#[cfg(test)]
mod tests {
    use actix_web::{body::MessageBody, http, test, App};
    use netsblox_cloud_common::{Group, User};

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    #[ignore]
    async fn test_list_groups() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_groups_403() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_group() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_members() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_members_403() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_members_404() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_group() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let data = api::UpdateGroupData {
                    name: "new_name".into(),
                };
                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;

                // Check that the group is updated in the db
                let query = doc! {"id": &group.id};
                let group = app_data
                    .groups
                    .find_one(query, None)
                    .await
                    .expect("Could not query DB")
                    .ok_or(UserError::GroupNotFoundError)
                    .expect("Group not found in db.");

                assert_eq!(group.name, "new_name".to_string());

                // Check response
                assert_eq!(response.status(), http::StatusCode::OK);
                let bytes = response.into_body().try_into_bytes().unwrap();
                let group: api::Group = serde_json::from_slice(&bytes).unwrap();

                assert_eq!(group.name, "new_name".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_update_group_no_perms() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let data = api::UpdateGroupData {
                    name: "new_name".into(),
                };
                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;

                // Not found is fine since it is technically more secure
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_update_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _group: api::Group = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_group_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_ne!(response.status(), http::StatusCode::OK);

                let query = doc! {"id": group.id};
                let group = app_data.groups.find_one(query, None).await.unwrap();
                assert!(group.is_some());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_group_404() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }
    // TODO: How does it handle malformed IDs?
}
