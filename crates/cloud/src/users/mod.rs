mod email_template;
mod html_template;
mod strategies;

use crate::app_data::AppData;
use crate::common::api;
use crate::common::api::UserRole;
use crate::common::{BannedAccount, SetPasswordToken, User};
use crate::errors::{InternalError, UserError};
use crate::groups::ensure_can_edit_group;
use crate::network::topology;
use crate::services::ensure_is_authorized_host;
use actix_session::Session;
use actix_web::http::header;
use actix_web::{get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lettre::Address;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use regex::Regex;
use rustrict::CensorStr;
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::collections::HashSet;

pub async fn is_super_user(app: &AppData, session: &Session) -> Result<bool, UserError> {
    match get_session_role(app, session).await? {
        UserRole::Admin => Ok(true),
        _ => Ok(false),
    }
}

async fn get_session_role(app: &AppData, session: &Session) -> Result<UserRole, UserError> {
    if let Some(username) = session.get::<String>("username").unwrap_or(None) {
        get_user_role(app, &username).await
    } else {
        session.purge();
        Err(UserError::LoginRequiredError)
    }
}

pub(crate) async fn get_user_role(app: &AppData, username: &str) -> Result<UserRole, UserError> {
    let query = doc! {"username": username};
    Ok(app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .map(|user| user.role)
        .unwrap_or(UserRole::User))
}

pub async fn is_moderator(app: &AppData, session: &Session) -> Result<bool, UserError> {
    let role = get_session_role(app, session).await?;
    Ok(role >= UserRole::Moderator)
}

pub async fn ensure_is_moderator(app: &AppData, session: &Session) -> Result<(), UserError> {
    if !is_moderator(app, session).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

pub async fn ensure_is_super_user(app: &AppData, session: &Session) -> Result<(), UserError> {
    if !is_super_user(app, session).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

pub async fn ensure_can_edit_user(
    app: &AppData,
    session: &Session,
    username: &str,
) -> Result<(), UserError> {
    if !can_edit_user(app, session, username).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

pub async fn can_edit_user(
    app: &AppData,
    session: &Session,
    username: &str,
) -> Result<bool, UserError> {
    if let Some(requestor) = session.get::<String>("username").unwrap_or(None) {
        let can_edit = requestor == username
            || is_super_user(app, session).await?
            || has_group_containing(app, &requestor, username).await?;
        Ok(can_edit)
    } else {
        Err(UserError::LoginRequiredError)
    }
}

async fn has_group_containing(app: &AppData, owner: &str, member: &str) -> Result<bool, UserError> {
    let query = doc! {"username": member};
    match app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        Some(user) => match user.group_id {
            Some(group_id) => {
                let query = doc! {"owner": owner};
                let cursor = app
                    .groups
                    .find(query, None)
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;
                let groups = cursor
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;
                let group_ids = groups
                    .into_iter()
                    .map(|group| group.id)
                    .collect::<HashSet<_>>();
                Ok(group_ids.contains(&group_id))
            }
            None => Ok(false),
        },
        None => Ok(false),
    }
}

#[get("/")]
async fn list_users(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;
    let query = doc! {};
    let cursor = app
        .users
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    let users: Vec<api::User> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|user| user.into())
        .collect();
    Ok(HttpResponse::Ok().json(users))
}

#[post("/create")]
async fn create_user(
    app: web::Data<AppData>,
    req: HttpRequest,
    user_data: web::Json<api::NewUser>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    app.ensure_not_tor_ip(req).await?;
    ensure_valid_email(&user_data.email)?;
    // TODO: record IP? Definitely
    // TODO: add more security features. Maybe activate accounts?

    let role = user_data.role.as_ref().unwrap_or(&UserRole::User);
    match role {
        UserRole::User => {
            if let Some(group_id) = &user_data.group_id {
                ensure_can_edit_group(&app, &session, group_id).await?;
            }
        }
        _ => ensure_is_super_user(&app, &session).await?,
    };

    let user: User = user_data.into_inner().into();
    ensure_valid_username(&user.username)?;

    let query = doc! {"email": &user.email};
    if let Some(_account) = app
        .banned_accounts
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        return Err(UserError::InvalidEmailAddress);
    }

    let query = doc! {"username": &user.username};
    let update = doc! {"$setOnInsert": &user};
    let options = mongodb::options::FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::Before)
        .upsert(true)
        .build();
    let existing_user = app
        .users
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if existing_user.is_some() {
        Err(UserError::UserExistsError)
    } else {
        if let Some(group_id) = user.group_id {
            app.group_members_updated(&group_id).await;
        }
        Ok(HttpResponse::Ok().body("User created"))
    }
}

fn ensure_valid_email(email: &str) -> Result<(), UserError> {
    email
        .parse::<Address>()
        .map_err(|_err| UserError::InvalidEmailAddress)?;

    Ok(())
}

fn ensure_valid_username(name: &str) -> Result<(), UserError> {
    if !is_valid_username(name) {
        Err(UserError::InvalidUsername)
    } else {
        Ok(())
    }
}

fn is_valid_username(name: &str) -> bool {
    let max_len = 25;
    let min_len = 3;
    let char_count = name.chars().count();
    lazy_static! {
        static ref USERNAME_REGEX: Regex = Regex::new(r"^[a-z][a-z0-9_\-]+$").unwrap();
    }

    char_count > min_len
        && char_count < max_len
        && USERNAME_REGEX.is_match(name)
        && !name.is_inappropriate()
}

#[post("/login")]
async fn login(
    req: HttpRequest,
    app: web::Data<AppData>,
    request: web::Json<api::LoginRequest>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: record login IPs?
    app.ensure_not_tor_ip(req).await?;

    let request = request.into_inner();
    let client_id = request.client_id.clone();
    let user = strategies::login(&app, request.credentials).await?;

    let query = doc! {"$or": [
        {"username": &user.username},
        {"email": &user.email},
    ]};

    if let Some(_account) = app
        .banned_accounts
        .find_one(query.clone(), None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        return Err(UserError::BannedUserError);
    }

    if let Some(client_id) = client_id {
        update_ownership(&app, &client_id, &user.username).await?;
        app.network.do_send(topology::SetClientUsername {
            id: client_id,
            username: Some(user.username.clone()),
        });
    }
    session.insert("username", &user.username).unwrap();
    Ok(HttpResponse::Ok().body(user.username))
}

async fn update_ownership(
    app: &AppData,
    client_id: &api::ClientId,
    username: &str,
) -> Result<(), UserError> {
    // Update ownership of current project
    if !client_id.as_str().starts_with('_') {
        return Err(UserError::InvalidClientIdError);
    }

    let query = doc! {"owner": client_id.as_str()};
    if let Some(metadata) = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        // No project will be found for non-NetsBlox clients such as PyBlox
        let name = app.get_valid_project_name(username, &metadata.name).await?;
        let update = doc! {"$set": {"owner": username, "name": name}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();
        let new_metadata = app
            .project_metadata
            .find_one_and_update(query, update, Some(options))
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        app.on_room_changed(new_metadata);
    }
    Ok(())
}

#[derive(Deserialize)]
struct LogoutQueryParams {
    pub client_id: Option<api::ClientId>,
}

// TODO: add a client ID to update the client ID immediately
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
        app.network.do_send(topology::SetClientUsername {
            id: client_id.clone(),
            username: None,
        });
    }

    HttpResponse::Ok().finish()
}

#[get("/whoami")]
async fn whoami(session: Session) -> Result<HttpResponse, UserError> {
    if let Some(username) = session.get::<String>("username").ok().flatten() {
        Ok(HttpResponse::Ok().body(username))
    } else {
        Err(UserError::PermissionsError)
    }
}

#[post("/{username}/ban")]
async fn ban_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": username};
    match app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        Some(user) => {
            let account = BannedAccount::new(user.username, user.email);
            app.banned_accounts
                .insert_one(account, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            Ok(HttpResponse::Ok().body("User has been banned"))
        }
        None => Err(UserError::UserNotFoundError),
    }
}

#[post("/{username}/delete")]
async fn delete_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": username};
    let user = app
        .users
        .find_one_and_delete(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    if let Some(group_id) = user.group_id {
        app.group_members_updated(&group_id).await;
    }

    Ok(HttpResponse::Ok().finish())
}

#[post("/{username}/password")]
async fn reset_password(
    app: web::Data<AppData>,
    req: HttpRequest,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, UserError> {
    app.ensure_not_tor_ip(req).await?;
    let (username,) = path.into_inner();
    let user = app
        .users
        .find_one(doc! {"username": &username}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let token = SetPasswordToken::new(username.clone());

    let update = doc! {"$setOnInsert": &token};
    let query = doc! {"username": &username};
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();

    let result = app
        .password_tokens
        .update_one(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.upserted_id.is_none() {
        return Err(UserError::PasswordResetLinkSentError);
    }

    let subject = "Password Reset Request";
    let url = format!(
        "{}/users/{}/password?token={}",
        app.settings.public_url, &username, &token.secret
    );
    let message = email_template::set_password_email(&username, &url);
    app.send_email(&user.email, subject, message).await?;
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
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    match params.into_inner().token {
        Some(token) => {
            let query = doc! {"secret": token};
            let token = app
                .password_tokens
                .find_one_and_delete(query, None) // If the username is incorrect, the token is compromised (so delete either way)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::PermissionsError)?;

            if token.username != username {
                return Err(UserError::PermissionsError);
            }
        }
        None => ensure_can_edit_user(&app, &session, &username).await?,
    }

    set_password(&app, &username, data.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn set_password(app: &AppData, username: &str, password: String) -> Result<(), UserError> {
    let query = doc! {"username": username};
    let user = app
        .users
        .find_one(query.clone(), None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let update = doc! {
        "$set": {
            "hash": sha512(&(password + &user.salt))
        }
    };
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(())
    }
}

pub(crate) fn sha512(text: &str) -> String {
    let mut hasher = Sha512::new();
    hasher.update(text);
    let hash = hasher.finalize();
    hex::encode(hash)
}

#[get("/{username}")]
async fn view_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    if ensure_is_authorized_host(&app, &req, None).await.is_err() {
        ensure_can_edit_user(&app, &session, &username).await?;
    }

    let query = doc! {"username": username};
    let user: api::User = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?
        .into();

    Ok(HttpResponse::Ok().json(user))
}

#[post("/{username}/link/")]
async fn link_account(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    creds: web::Json<strategies::Credentials>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;
    let creds = creds.into_inner();

    if let strategies::Credentials::NetsBlox { .. } = creds {
        return Err(UserError::InvalidAccountTypeError);
    };

    strategies::authenticate(&creds).await?;

    let account: api::LinkedAccount = creds.into();
    let query = doc! {"linkedAccounts": {"$elemMatch": &account}};
    let existing = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if existing.is_some() {
        return Err(UserError::AccountAlreadyLinkedError);
    }

    let query = doc! {"username": &username};
    let update = doc! {"$push": {"linkedAccounts": &account}};
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[post("/{username}/unlink")]
async fn unlink_account(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    account: web::Json<api::LinkedAccount>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;
    let query = doc! {"username": username};
    let update = doc! {"$pull": {"linkedAccounts": &account.into_inner()}};
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    if result.matched_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_user)
        .service(list_users)
        .service(login)
        .service(logout)
        .service(delete_user)
        .service(ban_user)
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
    use crate::test_utils;

    use super::*;
    use actix_web::{http, test, App};
    use netsblox_cloud_common::Group;

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
        test_utils::setup()
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
                    group_id: None,
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .set_json(&user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_member_nonowner() {
        test_utils::setup()
            .run(|app_data| async {
                let user_data = api::NewUser {
                    username: "someMember".into(),
                    email: "test@gmail.com".into(),
                    password: Some("pwd".into()),
                    group_id: None,
                    role: Some(UserRole::User),
                };
                let req = test::TestRequest::post()
                    .uri("/create")
                    .set_json(&user_data)
                    .to_request();

                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
            })
            .await;
    }

    //     #[actix_web::test]
    //     async fn test_login() {
    //         let user = User::from(NewUser::new(
    //             "brian".into(),
    //             "pwd_hash".into(),
    //             "email".into(),
    //             None,
    //         ));
    //         let (database, _) = init_app_data("login", vec![user])
    //             .await
    //             .expect("Unable to seed database");
    //         // Run the test
    //         let mut app = test::init_service(
    //             App::new()
    //                 .wrap(
    //                     CookieSession::signed(&[1; 32])
    //                         .domain("localhost:8080")
    //                         .name("netsblox")
    //                         .secure(true),
    //                 )
    //                 .app_data(web::Data::new(database))
    //                 .configure(config),
    //         )
    //         .await;

    //         let credentials = LoginCredentials {
    //             username: "brian".into(),
    //             password: "pwd_hash".into(),
    //             client_id: None,
    //             strategy: None,
    //         };
    //         let req = test::TestRequest::post()
    //             .uri("/login")
    //             .set_json(&credentials)
    //             .to_request();

    //         let response = test::call_service(&mut app, req).await;
    //         let cookie = response.headers().get(http::header::SET_COOKIE);
    //         assert!(cookie.is_some());
    //         let cookie_data = cookie.unwrap().to_str().unwrap();
    //         assert!(cookie_data.starts_with("netsblox="));
    //     }

    //     #[actix_web::test]
    //     async fn test_login_bad_pwd() {
    //         let user = User::from(NewUser::new(
    //             "brian".into(),
    //             "pwd_hash".into(),
    //             "email".into(),
    //             None,
    //         ));
    //         let (database, _) = init_app_data("login_bad_pwd", vec![user])
    //             .await
    //             .expect("Unable to seed database");
    //         // Run the test
    //         let mut app = test::init_service(
    //             App::new()
    //                 .app_data(web::Data::new(database))
    //                 .configure(config),
    //         )
    //         .await;

    //         let credentials = LoginCredentials {
    //             username: "brian".into(),
    //             password: "wrong_hash".into(),
    //             client_id: None,
    //             strategy: None,
    //         };
    //         let req = test::TestRequest::post()
    //             .uri("/login")
    //             .set_json(&credentials)
    //             .to_request();

    //         let response = test::call_service(&mut app, req).await;
    //         assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    //     }

    //     #[actix_web::test]
    //     async fn test_login_403() {
    //         let (database, _) = init_app_data("login_bad_user", vec![])
    //             .await
    //             .expect("Unable to seed database");
    //         // Run the test
    //         let mut app = test::init_service(
    //             App::new()
    //                 .app_data(web::Data::new(database))
    //                 .configure(config),
    //         )
    //         .await;

    //         let credentials = LoginCredentials {
    //             username: "nonExistentUser".into(),
    //             password: "pwd_hash".into(),
    //             client_id: None,
    //             strategy: None,
    //         };
    //         let req = test::TestRequest::post()
    //             .uri("/login")
    //             .set_json(&credentials)
    //             .to_request();

    //         let response = test::call_service(&mut app, req).await;
    //         assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    //     }

    //     #[actix_web::test]
    //     async fn test_login_banned() {
    //         let user = User::from(NewUser::new(
    //             "brian".into(),
    //             "pwd_hash".into(),
    //             "email".into(),
    //             None,
    //         ));
    //         let (app_data, _) = init_app_data("login_bad_pwd", vec![user])
    //             .await
    //             .expect("Unable to seed database");

    //         // Ban the account (manually)
    //         let collection = app_data.collection::<BannedAccount>("bannedAccounts");
    //         let banned_account = BannedAccount::new("brian".into(), "email".into());
    //         collection
    //             .insert_one(banned_account, None)
    //             .await
    //             .expect("Could not insert banned account");

    //         // Run the test
    //         let mut app = test::init_service(
    //             App::new()
    //                 .app_data(web::Data::new(app_data))
    //                 .configure(config),
    //         )
    //         .await;

    //         let credentials = LoginCredentials {
    //             username: "brian".into(),
    //             password: "pwd_hash".into(),
    //             client_id: None,
    //             strategy: None,
    //         };

    //         let req = test::TestRequest::post()
    //             .uri("/login")
    //             .set_json(&credentials)
    //             .to_request();

    //         let response = test::call_service(&mut app, req).await;
    //         assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    //     }

    //     #[actix_web::test]
    //     async fn test_login_with_strategy() {
    //         todo!();
    //     }

    //     #[actix_web::test]
    //     async fn test_login_with_strategy_403() {
    //         todo!();
    //     }

    //     #[actix_web::test]
    //     async fn test_logout() {
    //         let user = User::from(NewUser::new(
    //             "brian".into(),
    //             "pwd_hash".into(),
    //             "email".into(),
    //             None,
    //         ));
    //         let (database, _) = init_app_data("login", vec![user])
    //             .await
    //             .expect("Unable to seed database");
    //         // Run the test
    //         let mut app = test::init_service(
    //             App::new()
    //                 .wrap(
    //                     CookieSession::signed(&[0; 32])
    //                         .domain("localhost:8080")
    //                         .name("netsblox")
    //                         .secure(true),
    //                 )
    //                 .app_data(web::Data::new(database))
    //                 .configure(config),
    //         )
    //         .await;

    //         let req = test::TestRequest::post().uri("/logout").to_request();

    //         let response = test::call_service(&mut app, req).await;
    //         let cookie = response.headers().get(http::header::SET_COOKIE);
    //         assert!(cookie.is_some());
    //         let cookie_data = cookie.unwrap().to_str().unwrap();
    //         assert!(cookie_data.starts_with("netsblox="));
    //     }

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

    #[actix_web::test]
    async fn test_is_valid_username() {
        assert!(super::is_valid_username("hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_leading_underscore() {
        assert!(!super::is_valid_username("_hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_leading_dash() {
        assert!(!super::is_valid_username("-hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_at_symbol() {
        assert!(!super::is_valid_username("hello@gmail.com"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_vulgar() {
        assert!(!super::is_valid_username("shit"));
    }
}
