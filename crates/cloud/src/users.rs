use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::groups::ensure_can_edit_group;
use crate::models::User;
use actix_session::Session;
use actix_web::{get, patch, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::{doc, DateTime};
use netsblox_core::LinkedAccount;
use regex::Regex;
use rustrict::CensorStr;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashSet;
use std::time::SystemTime;

impl From<NewUser> for User {
    fn from(user_data: NewUser) -> Self {
        let salt = passwords::PasswordGenerator::new()
            .length(8)
            .exclude_similar_characters(true)
            .numbers(true)
            .spaces(false)
            .generate_one()
            .unwrap_or("salt".to_owned());

        let hash: String = if let Some(pwd) = user_data.password {
            sha512(&(pwd + &salt))
        } else {
            "None".to_owned()
        };

        User {
            username: user_data.username,
            hash,
            salt,
            email: user_data.email,
            group_id: user_data.group_id,
            created_at: DateTime::from_system_time(SystemTime::now()),
            linked_accounts: std::vec::Vec::new(),
            admin: user_data.admin,
            services_hosts: None,
        }
    }
}

pub async fn is_super_user(app: &AppData, session: &Session) -> bool {
    if let Some(username) = session.get::<String>("username").unwrap_or(None) {
        println!("checking if {} is a super user.", &username);
        let query = doc! {"username": username};
        match app.users.find_one(query, None).await.unwrap() {
            Some(user) => user.admin.unwrap_or(false),
            None => false,
        }
    } else {
        println!("no username in the cookie");
        false
    }
}

pub async fn ensure_is_super_user(app: &AppData, session: &Session) -> Result<(), UserError> {
    if !is_super_user(app, session).await {
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
    if !can_edit_user(app, session, username).await {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

pub async fn can_edit_user(app: &AppData, session: &Session, username: &str) -> bool {
    if let Some(requestor) = session.get::<String>("username").unwrap_or(None) {
        println!("Can {} edit {}?", requestor, username);
        requestor == username
            || is_super_user(app, session).await
            || has_group_containing(app, &requestor, username).await
    } else {
        println!("Could not get username from cookie!");
        false
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

#[derive(Serialize, Deserialize)]
struct NewUser {
    username: String,
    email: String, // TODO: validate the email address
    password: Option<String>,
    group_id: Option<String>,
    admin: Option<bool>,
}

#[get("/")]
async fn list_users(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;
    let query = doc! {};
    let cursor = app.users.find(query, None).await.unwrap();
    let usernames: Vec<String> = cursor
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .into_iter()
        .map(|user| user.username)
        .collect();
    Ok(HttpResponse::Ok().json(usernames))
}

#[post("/create")]
async fn create_user(
    app: web::Data<AppData>,
    user_data: web::Json<NewUser>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_valid_username(&user_data.username)?;
    ensure_valid_email(&user_data.email)?;
    println!("{:?}", user_data.password);

    if user_data.admin.unwrap_or(false) {
        ensure_is_super_user(&app, &session).await?;
    } else if let Some(group_id) = &user_data.group_id {
        ensure_can_edit_group(&app, &session, group_id).await?;
    }

    let user = User::from(user_data.into_inner());

    println!("create user: {}, {}", &user.username, &user.hash);
    let query = doc! {"username": &user.username};
    let update = doc! {"$setOnInsert": &user};
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();
    let result = app.users.update_one(query, update, options).await;

    match result {
        Ok(update_result) => {
            if update_result.matched_count == 0 {
                Ok(HttpResponse::Ok().body("User created"))
            } else {
                Ok(HttpResponse::BadRequest().body("User already exists"))
            }
        }
        Err(_err) => {
            // TODO: log the error
            Ok(HttpResponse::InternalServerError().body("User creation failed"))
        }
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
    lazy_static! {
        static ref USERNAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_\-]+$").unwrap();
    }
    USERNAME_REGEX.is_match(name) && !name.is_inappropriate()
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginCredentials {
    username: String,
    password: String,
    strategy: Option<String>,
    client_id: Option<String>, // TODO: add a secret token for the client?
}

// TODO: should we change the endpoints to /users/{id}
// (post -> create; get -> view; patch -> update, delete -> delete)
#[post("/login")]
async fn login(
    app: web::Data<AppData>,
    credentials: web::Json<LoginCredentials>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: check if tor IP
    println!("login attempt: {}", &credentials.username);
    let query = doc! {"username": &credentials.username};

    let banned_accounts = app.collection::<BannedAccount>("bannedAccounts");
    if let Some(_account) = banned_accounts
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?
    {
        return Err(UserError::BannedUserError);
    }

    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?
        .ok_or_else(|| UserError::UserNotFoundError)?;

    println!("credentials: {:?}", &credentials);
    let LoginCredentials {
        client_id,
        password,
        ..
    } = credentials.into_inner();
    let hash = sha512(&(password + &user.salt));
    if hash != user.hash {
        return Err(UserError::IncorrectPasswordError);
    }

    session.insert("username", &user.username).unwrap();
    match update_ownership(&app, &client_id, &user.username).await {
        Err(msg) => Ok(HttpResponse::BadRequest().body(msg)),
        _ => Ok(HttpResponse::Ok().body(user.username)),
    }
}

async fn update_ownership(
    app: &AppData,
    client_id: &Option<String>,
    username: &str,
) -> Result<bool, &'static str> {
    // Update ownership of current project
    if let Some(client_id) = &client_id {
        if !client_id.starts_with('_') {
            return Err("Invalid client ID.");
        }

        let query = doc! {"owner": client_id};
        let update = doc! {"$set": {"owner": username}};
        let result = app.project_metadata.update_one(query, update, None).await;
        // TODO: Update the room
        Ok(result.unwrap().modified_count > 0)
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
async fn whoami(session: Session) -> Result<HttpResponse, std::io::Error> {
    if let Some(username) = session.get::<String>("username").unwrap() {
        Ok(HttpResponse::Ok().body(username))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BannedAccount {
    username: String,
    email: String,
    banned_at: DateTime,
}

impl BannedAccount {
    pub fn new(username: String, email: String) -> BannedAccount {
        let banned_at = DateTime::now();
        BannedAccount {
            username,
            email,
            banned_at,
        }
    }
}

#[post("/{username}/ban")]
async fn ban_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let collection = app.collection::<User>("users");
    let query = doc! {"username": username};
    match collection.find_one(query, None).await.unwrap() {
        Some(user) => {
            let banned_accounts = app.collection::<BannedAccount>("bannedAccounts");
            let account = BannedAccount::new(user.username, user.email);
            banned_accounts.insert_one(account, None).await.unwrap();
            Ok(HttpResponse::Ok().body("Account has been banned"))
        }
        None => Ok(HttpResponse::NotFound().body("Account not found")),
    }

    // TODO: authenticate!
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
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let pg = passwords::PasswordGenerator::new()
        .length(8)
        .exclude_similar_characters(true)
        .numbers(true)
        .spaces(false);
    let new_password = pg.generate_one().unwrap();

    // TODO: This will need to send an email...

    set_password(&app, &username, new_password).await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/{username}/password")]
async fn change_password(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    data: web::Json<String>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    ensure_can_edit_user(&app, &session, &username).await?;

    set_password(&app, &username, data.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn set_password(app: &AppData, username: &str, password: String) -> Result<(), UserError> {
    let query = doc! {"username": username};
    let user = app
        .users
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?
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
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

    if result.modified_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(())
    }
}

fn sha512(text: &str) -> String {
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
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();

    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": username};
    let user: netsblox_core::User = app
        .users
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::UserNotFoundError)?
        .into();

    Ok(HttpResponse::Ok().json(user))
}

#[post("/{username}/link/")]
async fn link_account(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    account: web::Json<LinkedAccount>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;
    // TODO: check if already used
    // TODO: ensure valid account
    let query = doc! {"username": &username};
    let update = doc! {"$push": {"linkedAccounts": &account.into_inner()}};
    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?; // TODO: wrap the error?

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
        .service(reset_password)
        .service(change_password)
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
