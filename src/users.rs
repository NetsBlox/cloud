use actix_web::{web, App, HttpResponse, HttpRequest, HttpServer, middleware, cookie::Cookie};
use actix_web::{get, post, delete, patch};
use mongodb::Database;
use futures::stream::{TryStreamExt};
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use mongodb::options::FindOptions;

#[derive(Serialize, Deserialize)]
struct User {
    username: String,
    email: String,
    hash: String,
    groupId: Option<u32>,
    createdAt: u32,
    linkedAccounts: Vec<LinkedAccount>,
}

#[derive(Serialize, Deserialize)]
struct LinkedAccount {
    username: String,
    strategy: String,  // TODO: migrate type -> strategy
}

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
async fn login(credentials: web::Json<LoginCredentials>) -> HttpResponse {
    let host = "localhost:8080";  // TODO: configure via env variable
    // TODO: sign this and stuff
    let cookie = Cookie::build("netsblox", credentials.username.clone())
        .domain(host)
        .http_only(true)
        .finish();

    HttpResponse::Ok().cookie(cookie).finish()
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


