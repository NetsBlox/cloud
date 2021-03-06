mod strategies;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::groups::ensure_can_edit_group;
use crate::models::{BannedAccount, SetPasswordToken, User};
use crate::services::ensure_is_authorized_host;
use actix_session::Session;
use actix_web::{get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use netsblox_core::{LinkedAccount, LoginRequest, NewUser, UserRole};
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
        let query = doc! {"username": username};
        Ok(app
            .users
            .find_one(query, None)
            .await
            .unwrap()
            .map(|user| user.role)
            .unwrap_or(UserRole::User))
    } else {
        session.purge();
        Err(UserError::LoginRequiredError)
    }
}

pub async fn is_moderator(app: &AppData, session: &Session) -> Result<bool, UserError> {
    match get_session_role(app, session).await? {
        UserRole::Admin | UserRole::Moderator => Ok(true),
        _ => Ok(false),
    }
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
        println!("Can {} edit {}?", requestor, username);
        let can_edit = requestor == username
            || is_super_user(app, session).await?
            || has_group_containing(app, &requestor, username).await;
        Ok(can_edit)
    } else {
        println!("Could not get username from cookie!");
        Err(UserError::LoginRequiredError)
    }
}

async fn has_group_containing(app: &AppData, owner: &str, member: &str) -> bool {
    let query = doc! {"username": member};
    match app.users.find_one(query, None).await.unwrap() {
        Some(user) => match user.group_id {
            Some(group_id) => {
                let query = doc! {"owner": owner};
                let cursor = app.groups.find(query, None).await.unwrap();
                let groups = cursor.try_collect::<Vec<_>>().await.unwrap();
                let group_ids = groups
                    .into_iter()
                    .map(|group| group.id)
                    .collect::<HashSet<_>>();
                group_ids.contains(&group_id)
            }
            None => false,
        },
        None => false,
    }
}

#[get("/")]
async fn list_users(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;
    let query = doc! {};
    let cursor = app.users.find(query, None).await.unwrap();
    let users: Vec<netsblox_core::User> = cursor
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .into_iter()
        .map(|user| user.into())
        .collect();
    Ok(HttpResponse::Ok().json(users))
}

#[post("/create")]
async fn create_user(
    app: web::Data<AppData>,
    req: HttpRequest,
    user_data: web::Json<NewUser>,
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
                ensure_can_edit_group(&app, &session, group_id).await?
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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
    {
        return Err(UserError::InvalidEmailAddress);
    }

    let case_insensitive_email = mongodb::bson::Regex {
        pattern: format!("^{}$", &user.email),
        options: String::from("i"),
    };
    let query = doc! {"$or": [
        {"username": &user.username},
        {"email": {"$regex": case_insensitive_email}}
    ]};
    let update = doc! {"$setOnInsert": &user};
    let options = mongodb::options::FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::Before)
        .upsert(true)
        .build();
    let existing_user = app
        .users
        .find_one_and_update(query, update, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    if let Some(existing_user) = existing_user {
        if existing_user.username == user.username {
            Err(UserError::UserExistsError)
        } else {
            Err(UserError::EmailExistsError)
        }
    } else {
        Ok(HttpResponse::Ok().body("User created"))
    }
}

fn ensure_valid_email(email: &str) -> Result<(), UserError> {
    lazy_static! {
        static ref EMAIL_REGEX: Regex = Regex::new(r"^.+@.+\..+$").unwrap();
    }
    if EMAIL_REGEX.is_match(email) {
        Ok(())
    } else {
        Err(UserError::InvalidEmailAddress)
    }
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
    request: web::Json<LoginRequest>,
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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
    {
        return Err(UserError::BannedUserError);
    }

    update_ownership(&app, &client_id, &user.username).await?;
    // TODO: should we record more here? like user roles?
    session.insert("username", &user.username).unwrap();
    Ok(HttpResponse::Ok().body(user.username))
}

async fn update_ownership(
    app: &AppData,
    client_id: &Option<String>,
    username: &str,
) -> Result<bool, UserError> {
    // Update ownership of current project
    if let Some(client_id) = &client_id {
        if !client_id.starts_with('_') {
            return Err(UserError::InvalidClientIdError);
        }

        let query = doc! {"owner": client_id};
        let update = doc! {"$set": {"owner": username}};
        let result = app
            .project_metadata
            .update_one(query, update, None)
            .await
            .map_err(|err| InternalError::DatabaseConnectionError(err))?;

        // TODO: Update the room
        Ok(result.modified_count > 0)
    } else {
        Ok(false)
    }
}

#[post("/logout")]
async fn logout(session: Session) -> HttpResponse {
    session.purge();
    HttpResponse::Ok().finish()
}

#[get("/whoami")]
async fn whoami(session: Session) -> Result<HttpResponse, UserError> {
    if let Some(username) = session.get::<String>("username").unwrap() {
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
    // TODO: disallow self-ban?
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": username};
    match app.users.find_one(query, None).await.unwrap() {
        Some(user) => {
            let account = BannedAccount::new(user.username, user.email);
            app.banned_accounts.insert_one(account, None).await.unwrap();
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
    let result = app.users.delete_one(query, None).await.unwrap();
    if result.deleted_count > 0 {
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::UserNotFoundError)?;

    let token = SetPasswordToken::new(username.clone());
    let url = String::from("TODO"); // TODO: set the url;

    let update = doc! {"$setOnInsert": token};
    let query = doc! {"username": &username};
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();

    let result = app
        .password_tokens
        .update_one(query, update, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    if result.upserted_id.is_none() {
        return Err(UserError::PasswordResetLinkSentError);
    }

    let subject = "Password Reset Request";
    //let message = format!("<h1>Password Reset Request<h1>Click the link below to reset the password for {}");
    let message = String::from("TODO:");
    app.send_email(&user.email, subject, message).await?;
    // TODO: This will need to send an email with the token link...
    //user.email

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct SetPasswordQueryParams {
    pub token: Option<String>,
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
                .map_err(|err| InternalError::DatabaseConnectionError(err))?
                .ok_or_else(|| UserError::PermissionsError)?;

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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::UserNotFoundError)?;

    let update = doc! {
        "$set": {
            "hash": sha512(&(password + &user.salt))
        }
    };
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    if result.matched_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(())
    }
}

pub fn sha512(text: &str) -> String {
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

    match ensure_is_authorized_host(&app, &req).await {
        Err(_) => ensure_can_edit_user(&app, &session, &username).await?,
        _ => {}
    };

    let query = doc! {"username": username};
    let user: netsblox_core::User = app
        .users
        .find_one(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::UserNotFoundError)?
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

    match creds {
        strategies::Credentials::NetsBlox { .. } => return Err(UserError::InvalidAccountTypeError),
        _ => {}
    };

    // TODO: should we wrap this error?
    strategies::authenticate(&creds).await?;

    let account: LinkedAccount = creds.into();
    let query = doc! {"linkedAccounts": {"$elemMatch": &account}};
    let existing = app
        .users
        .find_one(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?; // TODO: wrap the error?
                                                                      // .and_then(|_user| UserError::AccountAlreadyLinkedErrorli
    if existing.is_some() {
        return Err(UserError::AccountAlreadyLinkedError);
    }

    let query = doc! {"username": &username};
    let update = doc! {"$push": {"linkedAccounts": &account}};
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?; // TODO: wrap the error?

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
    account: web::Json<LinkedAccount>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;
    let query = doc! {"username": username};
    let update = doc! {"$pull": {"linkedAccounts": &account.into_inner()}};
    let result = app.users.update_one(query, update, None).await.unwrap();
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
        .service(change_password)
        .service(whoami)
        .service(view_user)
        .service(link_account)
        .service(unlink_account);
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use actix_session::CookieSession;
//     use actix_web::{http, test, App};
//     use mongodb::{Client, Collection};

//     impl NewUser {
//         fn new(
//             username: String,
//             password: String,
//             email: String,
//             group_id: Option<ObjectId>,
//         ) -> NewUser {
//             NewUser {
//                 username,
//                 password: Some(password),
//                 email,
//                 group_id,
//             }
//         }
//     }

//     async fn init_app_data(
//         prefix: &'static str,
//         users: std::vec::Vec<User>,
//     ) -> Result<(AppData, Collection<User>), std::io::Error> {
//         let user_count = users.len();
//         let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
//             .await
//             .expect("Unable to connect to database");

//         let database = client.database("netsblox-tests");
//         // TODO: update
//         let app = AppData::new(database, None, Some(prefix));

//         // settings: Settings,
//         // db: Database,
//         // s3: S3Client,
//         // bucket: String,
//         // network: Option<Addr<TopologyActor>>,
//         // prefix: Option<&'static str>,

//         let collection = app.collection::<User>("users");
//         collection
//             .delete_many(doc! {}, None)
//             .await
//             .expect("Unable to empty database");

//         if user_count > 0 {
//             collection
//                 .insert_many(users, None)
//                 .await
//                 .expect("Unable to seed database");
//             let count = collection
//                 .count_documents(doc! {}, None)
//                 .await
//                 .expect("Unable to count docs");
//             assert_eq!(
//                 count, user_count as u64,
//                 "Expected {} docs but found {}",
//                 user_count, count
//             );
//         }

//         Ok((app, collection))
//     }

//     #[actix_web::test]
//     async fn test_create_user() {
//         let (database, collection) = init_app_data("create", vec![])
//             .await
//             .expect("Unable to seed database");

//         // Run the test
//         let mut app = test::init_service(
//             App::new()
//                 .app_data(web::Data::new(database))
//                 .configure(config),
//         )
//         .await;

//         let user_data = NewUser::new(
//             "test".into(),
//             "pwd_hash".into(),
//             "test@gmail.com".into(),
//             None,
//         );
//         let req = test::TestRequest::post()
//             .uri("/create")
//             .set_json(&user_data)
//             .to_request();

//         let response = test::call_service(&mut app, req).await;
//         let query = doc! {"username": user_data.username};
//         let result = collection
//             .find_one(query, None)
//             .await
//             .expect("Could not query for user");
//         assert!(result.is_some(), "User not found");
//     }

//     #[actix_web::test]
//     async fn test_create_user_profane() {
//         let (database, collection) = init_app_data("create_profane", vec![])
//             .await
//             .expect("Unable to seed database");

//         // Run the test
//         let mut app = test::init_service(
//             App::new()
//                 .app_data(web::Data::new(database))
//                 .configure(config),
//         )
//         .await;

//         let user_data = NewUser::new(
//             "hell".into(),
//             "pwd_hash".into(),
//             "test@gmail.com".into(),
//             None,
//         );
//         let req = test::TestRequest::post()
//             .uri("/create")
//             .set_json(&user_data)
//             .to_request();

//         let response = test::call_service(&mut app, req).await;
//         assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
//     }

//     //#[actix_web::test]
//     //async fn test_create_user_403() {  // group member
//     //let (database, collection) = init_app_data("create_403", vec![]).await.expect("Unable to seed database");

//     //// Run the test
//     //let mut app = test::init_service(
//     //App::new()
//     //.app_data(web::Data::new(database))
//     //.configure(config)
//     //).await;

//     //let user_data = NewUser::new(
//     //"hell".into(),
//     //"pwd_hash".into(),
//     //"test@gmail.com".into(),
//     //None  // TODO: set the group
//     //);
//     //let req = test::TestRequest::post()
//     //.uri("/create")
//     //.set_json(&user_data)
//     //.to_request();

//     //let response = test::call_service(&mut app, req).await;
//     //assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
//     //}

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

//     #[actix_web::test]
//     async fn test_delete_user() {
//         todo!();
//     }

//     #[actix_web::test]
//     async fn test_delete_user_403() {
//         todo!();
//     }

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

//     #[test]
//     async fn test_is_valid_username() {
//         assert!(super::is_valid_username("hello"));
//     }

//     #[test]
//     async fn test_is_valid_username_leading_underscore() {
//         assert_eq!(super::is_valid_username("_hello"), false);
//     }

//     #[test]
//     async fn test_is_valid_username_leading_dash() {
//         assert_eq!(super::is_valid_username("-hello"), false);
//     }

//     #[test]
//     async fn test_is_valid_username_at_symbol() {
//         assert_eq!(super::is_valid_username("hello@gmail.com"), false);
//     }

//     #[test]
//     async fn test_is_valid_username_vulgar() {
//         assert_eq!(super::is_valid_username("hell"), false);
//     }
// }
