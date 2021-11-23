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
        HttpResponse::NotFound().finish()
    }
}

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
    #[actix_web::test]
    async fn test_create_user() {
        // TODO: check that it sets the cookie
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_user_profane() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_user_403() {  // group member
        // TODO: check that it sets the cookie
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_login() {
        // TODO: check that it sets the cookie
        unimplemented!();
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
    async fn test_login_403() {
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
    fn is_valid_username() {
        assert!(super::is_valid_username("hello"));
    }

    #[test]
    fn is_valid_username_leading_underscore() {
        assert_eq!(super::is_valid_username("_hello"), false);
    }

    #[test]
    fn is_valid_username_leading_dash() {
        assert_eq!(super::is_valid_username("-hello"), false);
    }

    #[test]
    fn is_valid_username_at_symbol() {
        assert_eq!(super::is_valid_username("hello@gmail.com"), false);
    }

    #[test]
    fn is_valid_username_vulgar() {
        assert_eq!(super::is_valid_username("hell"), false);
    }
}
