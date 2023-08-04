use crate::app_data::AppData;
use crate::auth;
use crate::common::api;
use crate::common::api::{GroupId, ServiceHost};
use crate::common::AuthorizedServiceHost;
use crate::errors::{InternalError, UserError};
use crate::groups::actions::GroupActions;
use crate::services::hosts::actions::HostActions;
use crate::users::actions::UserActions;
use crate::users::{ensure_can_edit_user, ensure_is_super_user, is_super_user};
use actix_session::Session;
use actix_web::{delete, get, post, HttpRequest};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::{ReturnDocument, UpdateOptions};

#[get("/group/{id}")]
async fn list_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(GroupId,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_vg = auth::try_view_group(&app, &req, &id).await?;

    let actions: GroupActions = app.into();
    let group = actions.view_group(&auth_vg).await?;

    Ok(HttpResponse::Ok().json(group.services_hosts.unwrap_or_default()))
}

#[post("/group/{id}")]
async fn set_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(GroupId,)>,
    hosts: web::Json<Vec<ServiceHost>>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let auth_eg = auth::try_edit_group(&app, &req, Some(&id)).await?;

    let actions: GroupActions = app.into();
    let group = actions.set_group_hosts(&auth_eg, &hosts).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[get("/user/{username}")]
async fn list_user_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.into();
    let user = actions.get_user(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(user.services_hosts.unwrap_or_default()))
}

#[post("/user/{username}")]
async fn set_user_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    hosts: web::Json<Vec<ServiceHost>>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.into();
    let user = actions.set_hosts(&auth_eu, &hosts).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[get("/all/{username}")]
async fn list_all_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    // FIXME: Update this
    // let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    // let actions: UserActions = app.into();
    // let user = actions.get_all_hosts(&auth_vu).await?;

    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": &username};
    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let mut groups = app
        .groups
        .find(doc! {"owner": &username}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if let Some(group_id) = user.group_id {
        if let Some(in_group) = app
            .groups
            .find_one(doc! {"id": group_id}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            groups.push(in_group);
        }
    };

    let services_hosts = user.services_hosts.unwrap_or_default().into_iter().chain(
        groups
            .into_iter()
            .flat_map(|g| g.services_hosts.unwrap_or_default()),
    );
    Ok(HttpResponse::Ok().json(services_hosts.collect::<Vec<_>>()))
}

#[get("/authorized/")]
async fn get_authorized_hosts(
    app: web::Data<AppData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_vah = auth::try_view_auth_hosts(&app, &req).await?;

    let actions: HostActions = app.into();
    let hosts = actions.get_hosts(&auth_vah).await?;

    Ok(HttpResponse::Ok().json(hosts))
}

#[post("/authorized/")]
async fn authorize_host(
    app: web::Data<AppData>,
    host_data: web::Json<api::AuthorizedServiceHost>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_ah = auth::try_auth_host(&app, &req).await?;

    let actions: HostActions = app.into();
    let secret = actions.authorize(&auth_ah, host_data.into_inner()).await?;

    Ok(HttpResponse::Ok().json(secret))
}

#[delete("/authorized/{id}")]
async fn unauthorize_host(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (host_id,) = path.into_inner();
    let auth_ah = auth::try_auth_host(&app, &req).await?;

    let actions: HostActions = app.into();
    let host = actions.unauthorize(&auth_ah, &host_id).await?;

    Ok(HttpResponse::Ok().json(host))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_group_hosts)
        .service(set_group_hosts)
        .service(list_user_hosts)
        .service(set_user_hosts)
        .service(list_all_hosts)
        .service(authorize_host)
        .service(get_authorized_hosts)
        .service(unauthorize_host);
}

#[cfg(test)]
mod test {
    use actix_web::{body::MessageBody, http, test, App};
    use netsblox_cloud_common::{Group, User};

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    async fn test_set_user_hosts() {
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
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let cats = vec!["custom".into()];
                let hosts = vec![
                    ServiceHost {
                        url: "http://service1.com".into(),
                        categories: cats.clone(),
                    },
                    ServiceHost {
                        url: "http://service2.com".into(),
                        categories: cats.clone(),
                    },
                ];
                let req = test::TestRequest::post()
                    .uri(&format!("/user/{}", &user.username))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&hosts)
                    .to_request();

                let response = test::call_service(&app, req).await;

                // Check that the hosts have been set in the database
                let query = doc! {"username": &user.username};
                let user = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user")
                    .ok_or(UserError::UserNotFoundError)
                    .expect("User not found in db.");

                assert_eq!(user.services_hosts.map(|hosts| hosts.len()).unwrap_or(0), 2);

                assert_eq!(response.status(), http::StatusCode::OK);
                let bytes = response.into_body().try_into_bytes().unwrap();
                let user: api::User = serde_json::from_slice(&bytes).unwrap();

                assert_eq!(user.services_hosts.map(|hosts| hosts.len()).unwrap_or(0), 2);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_set_group_hosts() {
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

                let cats = vec!["custom".into()];
                let hosts = vec![
                    ServiceHost {
                        url: "http://service1.com".into(),
                        categories: cats.clone(),
                    },
                    ServiceHost {
                        url: "http://service2.com".into(),
                        categories: cats.clone(),
                    },
                ];
                let req = test::TestRequest::post()
                    .uri(&format!("/group/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&hosts)
                    .to_request();

                let response = test::call_service(&app, req).await;

                // Check that the hosts have been set in the database
                let query = doc! {"id": &group.id};
                let group = app_data
                    .groups
                    .find_one(query, None)
                    .await
                    .expect("Could not query for group")
                    .ok_or(UserError::GroupNotFoundError)
                    .expect("Group not found in db.");

                assert_eq!(
                    group.services_hosts.map(|hosts| hosts.len()).unwrap_or(0),
                    2
                );

                // Check the hosts are set in the returned group
                assert_eq!(response.status(), http::StatusCode::OK);
                let bytes = response.into_body().try_into_bytes().unwrap();
                let group: api::Group = serde_json::from_slice(&bytes).unwrap();

                assert_eq!(
                    group.services_hosts.map(|hosts| hosts.len()).unwrap_or(0),
                    2
                );
            })
            .await;
    }
}
