use actix_web::{web, HttpResponse, HttpRequest, cookie::Cookie};
use actix_web::{get, post, patch};
use mongodb::Database;
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{encode,Header,EncodingKey};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct User {
    username: String,
    email: String,
    hash: String,
    group_id: Option<u32>,
    created_at: u32,
    linked_accounts: Vec<LinkedAccount>,
}

#[derive(Serialize, Deserialize)]
struct LinkedAccount {
    username: String,
    strategy: String,  // TODO: migrate type -> strategy
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

// TODO: Check that the name doesn't contain profanity
#[post("/create")]
async fn create_user(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
    // TODO
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
}
