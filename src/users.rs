use actix_web::{web, HttpResponse, HttpRequest, cookie::Cookie};
use actix_web::{get, post, patch};
use mongodb::Database;
use mongodb::bson::{doc,Bson};
use serde::{Serialize, Deserialize};
use jsonwebtoken::{encode,Header,EncodingKey};
use std::time::SystemTime;
use rustrict::CensorStr;
use regex::Regex;
use lazy_static::lazy_static;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all="camelCase")]
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
        User{
            username: user_data.username,
            hash: user_data.hash,
            email: user_data.email,
            group_id: user_data.group_id,
            created_at: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as u32,
            linked_accounts: std::vec::Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct LinkedAccount {
    username: String,
    strategy: String,  // TODO: migrate type -> strategy
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
#[serde(rename_all="camelCase")]
struct UserCookie<'a>{
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
            issue_date: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
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
        NewUser{username, hash, email, group_id}
    }
}

#[post("/create")]
async fn create_user(db: web::Data<Database>, user_data: web::Json<NewUser>) -> Result<HttpResponse, std::io::Error> {
    if is_valid_username(&user_data.username) {
        let user = User::from(user_data.into_inner());
        let collection = db.collection::<User>("users");
        let query = doc!{"username": &user.username};
        let update = doc!{"$setOnInsert": &user};
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
            },
            Err(_) => {
                // TODO: log the error
                Ok(HttpResponse::InternalServerError().body("User creation failed"))
            },
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

#[post("/login")]
async fn login(db: web::Data<Database>, credentials: web::Json<LoginCredentials>) -> HttpResponse {
    let host = "localhost:8080";  // TODO: configure via env variable (config crate?)
    // TODO: authenticate
    let collection = db.collection::<User>("users");
    let query = doc!{"username": &credentials.username, "hash": &credentials.password_hash};
    if let Some(user) = collection.find_one(query, None).await.expect("Unable to retrieve user from database.") {
        let cookie = UserCookie::new(&user.username, None);
        let token = encode(&Header::default(), &cookie, &EncodingKey::from_secret("test".as_ref())).unwrap();  // TODO
        let cookie = Cookie::build("netsblox", token)
            .domain(host)
            .http_only(true)
            .finish();

        HttpResponse::Ok().cookie(cookie).finish()
    } else {
        HttpResponse::Unauthorized().finish()
    }
}

// TODO: add middleware for logging in...
#[post("/logout")]
async fn logout() -> HttpResponse {
    unimplemented!();
}

#[get("/whoami")]
async fn whoami(req: HttpRequest) -> Result<HttpResponse, std::io::Error> {
    if let Some(cookie) = req.cookie("netsblox") {
        let username = cookie.value();
        Ok(HttpResponse::Ok().body(username.to_string()))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}

#[post("/delete/{username}")]
async fn delete_user() -> HttpResponse {
    unimplemented!();
}

#[post("/password/{username}")]
async fn reset_password() -> HttpResponse {
    unimplemented!();
}

#[patch("/password/{username}")]
async fn change_password() -> HttpResponse {
    unimplemented!();
}

#[get("/view/{username}")]
async fn view_user() -> HttpResponse {
    unimplemented!();
}

#[post("/link/{username}")]
async fn link_account() -> HttpResponse {
    unimplemented!();
}

#[post("/unlink/{username}")]
async fn unlink_account() -> HttpResponse {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(create_user)
        .service(login)
        .service(logout)
        .service(delete_user)
        .service(reset_password)
        .service(change_password)
        .service(view_user)
        .service(link_account)
        .service(unlink_account);
}


#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test,http,App};
    use mongodb::{Client,Database,Collection};

    async fn init_database(name: &str, users: std::vec::Vec<User>) -> Result<(Database, Collection<User>), std::io::Error>{
        let user_count = users.len();
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/").await
            .expect("Unable to connect to database");

        // Seed the database
        //let database = client.database("netsblox-tests");
        //let collection = database.collection::<User>(&format!("users-{}", name));

        let database_name = &format!("netsblox-tests-{}", name);
        let database = client.database(database_name);
        let collection = database.collection::<User>("users");
        collection.delete_many(doc!{}, None).await.expect("Unable to empty database");

        if user_count > 0 {
            collection.insert_many(users, None).await.expect("Unable to seed database");
            let count = collection.count_documents(doc!{}, None).await.expect("Unable to count docs");
            assert_eq!(count, user_count as u64, "Expected {} docs but found {}", user_count, count);
        }

        Ok((database, collection))
    }

    #[actix_web::test]
    async fn test_create_user() {
        let (database, collection) = init_database("create", vec![]).await.expect("Unable to seed database");

        // Run the test
        let mut app = test::init_service(
            App::new()
            .app_data(web::Data::new(database))
            .configure(config)
        ).await;

        let user_data = NewUser::new("test".to_string(), "pwd_hash".to_string(), "test@gmail.com".to_string(), None);
        let req = test::TestRequest::post()
            .uri("/create")
            .set_json(&user_data)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        let query = doc!{"username": user_data.username};
        let result = collection.find_one(query, None).await.expect("Could not query for user");
        assert!(result.is_some(), "User not found");
    }

    #[actix_web::test]
    async fn test_create_user_profane() {
        let (database, collection) = init_database("create_profane", vec![]).await.expect("Unable to seed database");

        // Run the test
        let mut app = test::init_service(
            App::new()
            .app_data(web::Data::new(database))
            .configure(config)
        ).await;

        let user_data = NewUser::new("hell".to_string(), "pwd_hash".to_string(), "test@gmail.com".to_string(), None);
        let req = test::TestRequest::post()
            .uri("/create")
            .set_json(&user_data)
            .to_request();

        let response = test::call_service(&mut app, req).await;
        assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    }

    //#[actix_web::test]
    //async fn test_create_user_403() {  // group member
        //let (database, collection) = init_database("create_403", vec![]).await.expect("Unable to seed database");

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
        let user = User::from(NewUser::new("brian".to_string(), "pwd_hash".to_string(), "email".to_string(), None));
        let (database, _) = init_database("login", vec![user]).await.expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
            .app_data(web::Data::new(database))
            .configure(config)
        ).await;

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
        let user = User::from(NewUser::new("brian".to_string(), "pwd_hash".to_string(), "email".to_string(), None));
        let (database, _) = init_database("login_bad_pwd", vec![user]).await.expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
            .app_data(web::Data::new(database))
            .configure(config)
        ).await;

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
        let (database, _) = init_database("login_bad_user", vec![]).await.expect("Unable to seed database");
        // Run the test
        let mut app = test::init_service(
            App::new()
            .app_data(web::Data::new(database))
            .configure(config)
        ).await;

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
    async fn test_login_with_strategy() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_login_with_strategy_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_user() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_user_403() {
        unimplemented!();
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
