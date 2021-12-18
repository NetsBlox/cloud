use crate::app_data::AppData;
use actix_session::Session;
use actix_web::{get, patch, post};
use actix_web::{web, HttpResponse};
use lazy_static::lazy_static;
use mongodb::bson::{doc, Bson, DateTime};
use regex::Regex;
use rustrict::CensorStr;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// TODO: Add banning support
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct User {
    username: String,
    email: String,
    hash: String,
    group_id: Option<u32>,
    created_at: u32,
    linked_accounts: Vec<LinkedAccount>,
}

impl Into<Bson> for User {
    fn into(self) -> Bson {
        Bson::Document(doc! {
            "username": self.username,
            "email": self.email,
            "hash": self.hash,
            "groupId": self.group_id,
            "createdAt": self.created_at,
            "linkedAccounts": Into::<Bson>::into(self.linked_accounts)
        })
    }
}

impl From<NewUser> for User {
    fn from(user_data: NewUser) -> Self {
        User {
            username: user_data.username,
            hash: user_data.hash,
            email: user_data.email,
            group_id: user_data.group_id,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32,
            linked_accounts: std::vec::Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct LinkedAccount {
    username: String,
    strategy: String, // TODO: migrate type -> strategy
}

impl Into<Bson> for LinkedAccount {
    fn into(self) -> Bson {
        Bson::Document(doc! {
            "username": self.username,
            "strategy": self.strategy,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserCookie<'a> {
    username: &'a str,
    //group_id: String,
    issue_date: u64,
    remember: bool,
}

impl UserCookie<'_> {
    pub fn new<'a>(username: &'a str, remember: Option<bool>) -> UserCookie {
        UserCookie {
            username,
            remember: remember.unwrap_or(false),
            issue_date: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct NewUser {
    username: String,
    hash: String,
    email: String,
    group_id: Option<u32>,
}

impl NewUser {
    pub fn new(username: String, hash: String, email: String, group_id: Option<u32>) -> NewUser {
        NewUser {
            username,
            hash,
            email,
            group_id,
        }
    }
}

#[post("/create")]
async fn create_user(
    app: web::Data<AppData>,
    user_data: web::Json<NewUser>,
) -> Result<HttpResponse, std::io::Error> {
    if is_valid_username(&user_data.username) {
        let user = User::from(user_data.into_inner());
        let collection = app.collection::<User>("users");
        let query = doc! {"username": &user.username};
        let update = doc! {"$setOnInsert": &user};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();
        let result = collection.update_one(query, update, options).await;

        match result {
            Ok(update_result) => {
                if update_result.matched_count == 0 {
                    Ok(HttpResponse::Ok().body("User created"))
                } else {
                    Ok(HttpResponse::BadRequest().body("User already exists"))
                }
            }
            Err(_) => {
                // TODO: log the error
                Ok(HttpResponse::InternalServerError().body("User creation failed"))
            }
        }
    } else {
        Ok(HttpResponse::BadRequest().body("Invalid username"))
    }
}

fn is_valid_username(name: &str) -> bool {
    lazy_static! {
        static ref USERNAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_\-]+$").unwrap();
    }
    USERNAME_REGEX.is_match(name) && !name.is_inappropriate()
}

#[derive(Serialize, Deserialize)]
struct LoginCredentials {
    username: String,
    password_hash: String,
}

// TODO: should we change the endpoints to /users/{id}
// (post -> create; get -> view; patch -> update, delete -> delete)
#[post("/login")]
async fn login(
    app: web::Data<AppData>,
    credentials: web::Json<LoginCredentials>,
    session: Session,
) -> HttpResponse {
    // TODO: authenticate
    let collection = app.collection::<User>("users");
    let query = doc! {"username": &credentials.username, "hash": &credentials.password_hash};

    let banned_accounts = app.collection::<BannedAccount>("bannedAccounts");
    if let Some(_account) = banned_accounts
        .find_one(doc! {"username": &credentials.username}, None)
        .await
        .expect("Unable to verify account isn't banned.")
    {
        return HttpResponse::Unauthorized().body("Account has been banned");
    }

    match collection
        .find_one(query, None)
        .await
        .expect("Unable to retrieve user from database.")
    {
        Some(user) => {
            session.insert("username", &user.username).unwrap();
            HttpResponse::Ok().finish()
        }
        None => HttpResponse::Unauthorized().finish(),
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
        Ok(HttpResponse::Ok().body(username.to_string()))
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
            Ok(HttpResponse::Ok().finish())
        }
        None => Ok(HttpResponse::NotFound().finish()),
    }

    // TODO: authenticate!
    // TODO: record the email and username as banned
}

#[post("/{username}/delete")]
async fn delete_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: check auth
    let (username,) = path.into_inner();
    let collection = app.collection::<User>("users");
    let query = doc! {"username": username};
    let result = collection.delete_one(query, None).await.unwrap();
    if result.deleted_count > 0 {
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[post("/{username}/password")]
async fn reset_password() -> HttpResponse {
    unimplemented!();
    // TODO: This will need to send an email...
}

#[derive(Deserialize)]
struct PasswordChangeData {
    password_hash: String,
}

#[patch("/{username}/password")]
async fn change_password(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    data: web::Json<PasswordChangeData>,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let collection = app.collection::<User>("users");
    let query = doc! {"username": username};
    let update = doc! {"hash": &data.password_hash};
    let result = collection.update_one(query, update, None).await.unwrap();
    if result.modified_count > 0 {
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/{username}")]
async fn view_user(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let collection = app.collection::<User>("users");
    let query = doc! {"username": username};
    if let Some(user) = collection.find_one(query, None).await.unwrap() {
        // TODO: check auth
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[derive(Deserialize)]
struct StrategyCredentials {
    // TODO: combine this with the basic login?
    username: String,
    password: String,
}

#[post("/{username}/link/{strategy}")]
async fn link_account(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    credentials: web::Json<StrategyCredentials>,
) -> Result<HttpResponse, std::io::Error> {
    let (username, strategy) = path.into_inner();
    let collection = app.collection::<User>("users");
    // TODO: add auth
    // TODO: check if already used
    let query = doc! {"username": &username};
    let account = LinkedAccount { username, strategy };
    let update = doc! {"$pull": {"linkedAccounts": &account}};
    let result = collection.update_one(query, update, None).await.unwrap();
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
) -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    let (username,) = path.into_inner();
    let collection = app.collection::<User>("users");
    let query = doc! {"username": username};
    let update = doc! {"$pull": {"linkedAccounts": &account.into_inner()}};
    let result = collection.update_one(query, update, None).await.unwrap();
    if result.matched_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[get("/{owner}/projects")]
async fn list_user_projects() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{owner}/projects/shared")]
async fn list_shared_projects() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_user)
        .service(login)
        .service(logout)
        .service(delete_user)
        .service(reset_password)
        .service(change_password)
        .service(view_user)
        .service(link_account)
        .service(unlink_account)
        .service(list_user_projects)
        .service(list_shared_projects);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_session::CookieSession;
    use actix_web::{http, test, App};
    use mongodb::{Client, Collection};

    async fn init_app_data(
        prefix: &'static str,
        users: std::vec::Vec<User>,
    ) -> Result<(AppData, Collection<User>), std::io::Error> {
        let user_count = users.len();
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        let database = client.database("netsblox-tests");
        let app = AppData::new(database, None, Some(prefix));
        let collection = app.collection::<User>("users");
        collection
            .delete_many(doc! {}, None)
            .await
            .expect("Unable to empty database");

        if user_count > 0 {
            collection
                .insert_many(users, None)
                .await
                .expect("Unable to seed database");
            let count = collection
                .count_documents(doc! {}, None)
                .await
                .expect("Unable to count docs");
            assert_eq!(
                count, user_count as u64,
                "Expected {} docs but found {}",
                user_count, count
            );
        }

        Ok((app, collection))
    }

    #[actix_web::test]
    async fn test_create_user() {
        let (database, collection) = init_app_data("create", vec![])
            .await
            .expect("Unable to seed database");

        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let user_data = NewUser::new(
            "test".to_string(),
            "pwd_hash".to_string(),
            "test@gmail.com".to_string(),
            None,
        );
        let req = test::TestRequest::post()
            .uri("/create")
            .set_json(&user_data)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        let query = doc! {"username": user_data.username};
        let result = collection
            .find_one(query, None)
            .await
            .expect("Could not query for user");
        assert!(result.is_some(), "User not found");
    }

    #[actix_web::test]
    async fn test_create_user_profane() {
        let (database, collection) = init_app_data("create_profane", vec![])
            .await
            .expect("Unable to seed database");

        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let user_data = NewUser::new(
            "hell".to_string(),
            "pwd_hash".to_string(),
            "test@gmail.com".to_string(),
            None,
        );
        let req = test::TestRequest::post()
            .uri("/create")
            .set_json(&user_data)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    }

    //#[actix_web::test]
    //async fn test_create_user_403() {  // group member
    //let (database, collection) = init_app_data("create_403", vec![]).await.expect("Unable to seed database");

    //// Run the test
    //let mut app = test::init_service(
    //App::new()
    //.app_data(web::Data::new(database))
    //.configure(config)
    //).await;

    //let user_data = NewUser::new(
    //"hell".to_string(),
    //"pwd_hash".to_string(),
    //"test@gmail.com".to_string(),
    //None  // TODO: set the group
    //);
    //let req = test::TestRequest::post()
    //.uri("/create")
    //.set_json(&user_data)
    //.to_request();

    //let response = test::call_service(&mut app, req).await;
    //assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    //}

    #[actix_web::test]
    async fn test_login() {
        let user = User::from(NewUser::new(
            "brian".to_string(),
            "pwd_hash".to_string(),
            "email".to_string(),
            None,
        ));
        let (database, _) = init_app_data("login", vec![user])
            .await
            .expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
                .wrap(
                    CookieSession::signed(&[1; 32])
                        .domain("localhost:8080")
                        .name("netsblox")
                        .secure(true),
                )
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let credentials = LoginCredentials {
            username: "brian".to_string(),
            password_hash: "pwd_hash".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&credentials)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        let cookie = response.headers().get(http::header::SET_COOKIE);
        assert!(cookie.is_some());
        let cookie_data = cookie.unwrap().to_str().unwrap();
        assert!(cookie_data.starts_with("netsblox="));
    }

    #[actix_web::test]
    async fn test_login_bad_pwd() {
        let user = User::from(NewUser::new(
            "brian".to_string(),
            "pwd_hash".to_string(),
            "email".to_string(),
            None,
        ));
        let (database, _) = init_app_data("login_bad_pwd", vec![user])
            .await
            .expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let credentials = LoginCredentials {
            username: "brian".to_string(),
            password_hash: "wrong_hash".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&credentials)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_login_403() {
        let (database, _) = init_app_data("login_bad_user", vec![])
            .await
            .expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let credentials = LoginCredentials {
            username: "nonExistentUser".to_string(),
            password_hash: "pwd_hash".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&credentials)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_login_banned() {
        let user = User::from(NewUser::new(
            "brian".to_string(),
            "pwd_hash".to_string(),
            "email".to_string(),
            None,
        ));
        let (app_data, _) = init_app_data("login_bad_pwd", vec![user])
            .await
            .expect("Unable to seed database");

        let collection = app_data.collection::<BannedAccount>("bannedAccounts");
        let banned_account = BannedAccount::new("brian".to_string(), "email".to_string());
        collection
            .insert_one(banned_account, None)
            .await
            .expect("Could not insert banned account");
        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_data))
                .configure(config),
        )
        .await;

        // TODO: Ban the account (manually)
        let credentials = LoginCredentials {
            username: "brian".to_string(),
            password_hash: "pwd_hash".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&credentials)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_login_with_strategy() {
        todo!();
    }

    #[actix_web::test]
    async fn test_login_with_strategy_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_logout() {
        let user = User::from(NewUser::new(
            "brian".to_string(),
            "pwd_hash".to_string(),
            "email".to_string(),
            None,
        ));
        let (database, _) = init_app_data("login", vec![user])
            .await
            .expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
                .wrap(
                    CookieSession::signed(&[0; 32])
                        .domain("localhost:8080")
                        .name("netsblox")
                        .secure(true),
                )
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;

        let req = test::TestRequest::post().uri("/logout").to_request();

        let response = test::call_service(&mut app, req).await;
        let cookie = response.headers().get(http::header::SET_COOKIE);
        assert!(cookie.is_some());
        let cookie_data = cookie.unwrap().to_str().unwrap();
        assert!(cookie_data.starts_with("netsblox="));
    }

    #[actix_web::test]
    async fn test_delete_user() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_user_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_link_account() {
        todo!();
    }

    #[actix_web::test]
    async fn test_link_account_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_link_account_duplicate() {
        todo!();
    }

    #[test]
    async fn test_is_valid_username() {
        assert!(super::is_valid_username("hello"));
    }

    #[test]
    async fn test_is_valid_username_leading_underscore() {
        assert_eq!(super::is_valid_username("_hello"), false);
    }

    #[test]
    async fn test_is_valid_username_leading_dash() {
        assert_eq!(super::is_valid_username("-hello"), false);
    }

    #[test]
    async fn test_is_valid_username_at_symbol() {
        assert_eq!(super::is_valid_username("hello@gmail.com"), false);
    }

    #[test]
    async fn test_is_valid_username_vulgar() {
        assert_eq!(super::is_valid_username("hell"), false);
    }
}
