use super::html_template;
use super::strategies;
use crate::app_data::AppData;
use crate::auth;
use crate::common::api;
use crate::errors::UserError;
use crate::users::actions::UserActions;
use crate::utils;
use actix_session::Session;
use actix_web::http::header;
use actix_web::{get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};
use mongodb::bson::doc;
use serde::Deserialize;

#[get("/")]
async fn list_users(app: web::Data<AppData>, req: HttpRequest) -> Result<HttpResponse, UserError> {
    let auth_lu = auth::try_list_users(&app, &req).await?;

    let actions: UserActions = app.as_user_actions();
    let users = actions.list_users(&auth_lu).await?;

    Ok(HttpResponse::Ok().json(users))
}

#[post("/create")]
async fn create_user(
    app: web::Data<AppData>,
    req: HttpRequest,
    user_data: web::Json<api::NewUser>,
) -> Result<HttpResponse, UserError> {
    let req_addr = req.peer_addr().map(|addr| addr.ip());
    if let Some(addr) = req_addr {
        app.ensure_not_tor_ip(&addr).await?;
    }

    // TODO: record IP? Definitely
    // TODO: add more security features. Maybe activate accounts?

    let auth_cu = auth::try_create_user(&app, &req, user_data.into_inner()).await?;
    let actions: UserActions = app.as_user_actions();
    let user = actions.create_user(auth_cu).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("/login")]
async fn login(
    req: HttpRequest,
    app: web::Data<AppData>,
    request: web::Json<api::LoginRequest>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let req_addr = req.peer_addr().map(|addr| addr.ip());
    if let Some(addr) = req_addr {
        app.ensure_not_tor_ip(&addr).await?;
    }

    let request = request.into_inner();

    let actions: UserActions = app.as_user_actions();
    let user = actions.login(request).await?;

    session.insert("username", &user.username).unwrap();
    Ok(HttpResponse::Ok().json(user))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LogoutQueryParams {
    pub client_id: Option<api::ClientId>,
}

// TODO: make sure the username and client ID are already associated
// TODO: ideally this would make sure there is a client token/secret
#[post("/logout")]
async fn logout(
    app: web::Data<AppData>,
    params: web::Query<LogoutQueryParams>,
    session: Session,
) -> HttpResponse {
    session.purge();

    if let Some(client_id) = &params.client_id {
        // FIXME: this method should be updated as it currently could be used to half logout other users...
        let actions: UserActions = app.as_user_actions();
        actions.logout(client_id);
    }

    HttpResponse::Ok().finish()
}

#[get("/whoami")]
async fn whoami(req: HttpRequest) -> Result<HttpResponse, UserError> {
    if let Some(username) = utils::get_username(&req) {
        Ok(HttpResponse::Ok().body(username))
    } else {
        Err(UserError::PermissionsError)
    }
}

#[post("/{username}/ban")]
async fn ban_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_bu = auth::try_ban_user(&app, &req, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let account = actions.ban_user(&auth_bu).await?;

    Ok(HttpResponse::Ok().json(account))
}

#[post("/{username}/unban")]
async fn unban_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_bu = auth::try_ban_user(&app, &req, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let account = actions.unban_user(&auth_bu).await?;

    Ok(HttpResponse::Ok().json(account))
}

#[post("/{username}/delete")]
async fn delete_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let user = actions.delete_user(&auth_eu).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("/{username}/password")]
async fn reset_password(
    app: web::Data<AppData>,
    req: HttpRequest,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, UserError> {
    let req_addr = req.peer_addr().map(|addr| addr.ip());
    if let Some(addr) = req_addr {
        app.ensure_not_tor_ip(&addr).await?;
    }

    let (username,) = path.into_inner();
    let actions: UserActions = app.as_user_actions();
    actions.reset_password(&username).await?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct SetPasswordQueryParams {
    pub token: Option<String>,
}

#[get("/{username}/password")]
async fn change_password_page(
    path: web::Path<(String,)>,
    _params: web::Query<SetPasswordQueryParams>,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let html = html_template::set_password_page(&username);
    Ok(HttpResponse::Ok()
        .content_type(header::ContentType::html())
        .body(html))
}

#[patch("/{username}/password")]
async fn change_password(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    data: web::Json<String>,
    params: web::Query<SetPasswordQueryParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    let auth_sp = auth::try_set_password(&app, &req, &username, params.into_inner().token).await?;
    let actions: UserActions = app.as_user_actions();
    let user = actions.set_password(&auth_sp, data.into_inner()).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[get("/{username}")]
async fn view_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let user = actions.get_user(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("/{username}/link/")]
async fn link_account(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    creds: web::Json<strategies::Credentials>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    let creds = creds.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let user = actions.link_account(&auth_eu, creds).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("/{username}/unlink")]
async fn unlink_account(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    account: web::Json<api::LinkedAccount>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let user = actions
        .unlink_account(&auth_eu, account.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(user))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_user)
        .service(list_users)
        .service(login)
        .service(logout)
        .service(delete_user)
        .service(ban_user)
        .service(unban_user)
        .service(reset_password)
        .service(change_password_page)
        .service(change_password)
        .service(whoami)
        .service(view_user)
        .service(link_account)
        .service(unlink_account);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{errors::InternalError, network::topology, test_utils};

    use super::*;
    use actix_web::{http, test, App};
    use netsblox_cloud_common::{
        api::{BannedAccount, Credentials, UserRole},
        Group, User,
    };

    #[actix_web::test]
    async fn test_create_user() {
        test_utils::setup()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let user_data = api::NewUser {
                    username: "test".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: None,
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let query = doc! {"username": user_data.username};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_some(), "User not found");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_user_profane() {
        test_utils::setup()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let user_data = api::NewUser {
                    username: "damn".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: None,
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

                let query = doc! {"username": user_data.username};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_none(), "User created");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_member_unauth() {
        let owner_name = String::from("admin");
        let owner: User = api::NewUser {
            username: owner_name.clone(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner_name, "some_group".into());
        test_utils::setup()
            .with_users(&[owner])
            .with_groups(&[group.clone()])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let user_data = api::NewUser {
                    username: "someMember".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: Some(group.id),
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_member_nonowner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other_user: User = api::NewUser {
            username: "otherUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "some_group".into());
        test_utils::setup()
            .with_users(&[owner, other_user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let user_data = api::NewUser {
                    username: "someMember".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: Some(group.id),
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .cookie(test_utils::cookie::new(&other_user.username))
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_member_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "some_group".into());
        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let user_data = api::NewUser {
                    username: "someMember".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: Some(group.id),
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .cookie(test_utils::cookie::new(&owner.username))
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox { username, password },
                    client_id: None,
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let cookie = response.headers().get(http::header::SET_COOKIE);
                assert!(cookie.is_some());

                let cookie_data = cookie.unwrap().to_str().unwrap();
                assert!(cookie_data.starts_with("test_netsblox="));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_user_json() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox {
                        username: username.clone(),
                        password,
                    },
                    client_id: None,
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let user: api::User = test::call_and_read_body_json(&app, req).await;
                assert_eq!(user.username, username);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_bad_pwd() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox {
                        username,
                        password: "badpwd".into(),
                    },
                    client_id: None,
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
                let cookie = response.headers().get(http::header::SET_COOKIE);
                assert!(cookie.is_none());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_invalid_user() {
        let username: String = "user".into();
        let password: String = "password".into();

        test_utils::setup()
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox { username, password },
                    client_id: None,
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
                let cookie = response.headers().get(http::header::SET_COOKIE);
                assert!(cookie.is_none());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_set_client_username() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();
        let client = test_utils::network::Client::new(None, None);

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_clients(&[client.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox {
                        username: username.clone(),
                        password,
                    },
                    client_id: Some(client.id),
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                // Give it a little time for the actix message to be received
                // (message passing is async)
                tokio::time::sleep(Duration::from_millis(10)).await;

                let task = app_data
                    .network
                    .send(topology::GetOnlineUsers(None))
                    .await
                    .map_err(InternalError::ActixMessageError)
                    .unwrap();
                let online_friends = task.run().await;
                assert_eq!(online_friends.len(), 1);
                assert!(online_friends.contains(&username));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_banned() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user])
            .with_banned_users(&[username.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let credentials = api::LoginRequest {
                    credentials: Credentials::NetsBlox { username, password },
                    client_id: None,
                };
                let req = test::TestRequest::post()
                    .uri("/login")
                    .set_json(&credentials)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
                let cookie = response.headers().get(http::header::SET_COOKIE);
                assert!(cookie.is_none());
            })
            .await;
    }

    //     #[actix_web::test]
    //     async fn test_login_with_strategy() {
    //         todo!();
    //     }

    //     #[actix_web::test]
    //     async fn test_login_with_strategy_403() {
    //         todo!();
    //     }

    #[actix_web::test]
    async fn test_logout() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;
                let req = test::TestRequest::post()
                    .uri("/logout")
                    .cookie(test_utils::cookie::new(&username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let cookie = response.headers().get(http::header::SET_COOKIE);
                assert!(cookie.is_some());
                let cookie_data = cookie.unwrap().to_str().unwrap();
                assert!(cookie_data.starts_with("test_netsblox=;"));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_logout_set_client_username() {
        let username: String = "user".into();
        let password: String = "password".into();
        let user: User = api::NewUser {
            username: username.clone(),
            email: "user@netsblox.org".into(),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();
        let client = test_utils::network::Client::new(Some(username.clone()), None);

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_clients(&[client.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;
                let req = test::TestRequest::post()
                    .uri(&format!("/logout?clientId={}", client.id.as_str()))
                    .cookie(test_utils::cookie::new(&username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                // Give it a little time for the actix message to be received
                // (message passing is async)
                tokio::time::sleep(Duration::from_millis(10)).await;

                let task = app_data
                    .network
                    .send(topology::GetOnlineUsers(None))
                    .await
                    .map_err(InternalError::ActixMessageError)
                    .unwrap();
                let online_friends = task.run().await;

                assert_eq!(online_friends.len(), 0);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_user_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let other_username = "other_user";
        let other_user: User = api::NewUser {
            username: other_username.to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[admin, other_user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new("admin");
                let req = test::TestRequest::post()
                    .uri("/other_user/delete")
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let query = doc! {"username": other_username};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_none(), "User not deleted");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_user_unauth() {
        let other_username = "other_user";
        let other_user: User = api::NewUser {
            username: other_username.to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[other_user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/other_user/delete")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);

                let query = doc! {"username": other_username};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_some(), "User deleted");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_user_group_owner() {
        let owner_name = "owner".to_string();
        let owner: User = api::NewUser {
            username: owner_name.clone(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner_name.clone(), "some_group".into());
        let other_username = "other_user";
        let other_user: User = api::NewUser {
            username: other_username.to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[owner, other_user])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/other_user/delete")
                    .cookie(test_utils::cookie::new(&owner_name))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let query = doc! {"username": other_username};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_none(), "User not deleted");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_user_other_user() {
        let user1: User = api::NewUser {
            username: "user1".to_string(),
            email: "user1@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let user1_name = user1.username.clone();
        let user2: User = api::NewUser {
            username: "user2".to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user1, user2])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/user2/delete")
                    .cookie(test_utils::cookie::new(&user1_name))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);

                let query = doc! {"username": "user2"};
                let result = app_data
                    .users
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_some(), "User deleted");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_ban_user() {
        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let admin_name = admin.username.clone();
        let some_user: User = api::NewUser {
            username: "some_user".to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[admin, some_user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/some_user/ban")
                    .cookie(test_utils::cookie::new(&admin_name))
                    .to_request();

                // This will panic if the response isn't a banned account; no assert needed
                let _account: BannedAccount = test::call_and_read_body_json(&app, req).await;

                let query = doc! {"username": "some_user"};
                let result = app_data
                    .banned_accounts
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_some(), "User not banned");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_ban_user_403() {
        let user: User = api::NewUser {
            username: "user".to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let user_name = user.username.clone();
        let some_user: User = api::NewUser {
            username: "some_user".to_string(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user, some_user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/some_user/ban")
                    .cookie(test_utils::cookie::new(&user_name))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);

                let query = doc! {"username": "some_user"};
                let result = app_data
                    .banned_accounts
                    .find_one(query, None)
                    .await
                    .expect("Could not query for user");

                assert!(result.is_none(), "User banned");
            })
            .await;
    }

    #[actix_web::test]
    #[ignore] // ignore until we can test fns using the mailer
    async fn test_reset_password() {
        let user: User = api::NewUser {
            username: "user".to_string(),
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
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri(&format!("/{}/password", &user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    //     #[actix_web::test]
    //     async fn test_link_account() {
    //         todo!();
    //     }

    //     #[actix_web::test]
    //     async fn test_link_account_403() {
    //         todo!();
    //     }

    //     #[actix_web::test]
    //     async fn test_link_account_duplicate() {
    //         todo!();
    //     }
}
