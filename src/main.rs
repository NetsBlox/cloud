use actix_web::{web, App, HttpResponse, HttpRequest, HttpServer, middleware};
use actix_web::get;
use serde::Serialize;
use mongodb::Client;
use env_logger;
mod libraries;
mod services_hosts;
mod users;
mod projects;
mod database;

////////////// Users //////////////
#[derive(Serialize)]
struct User {
    username: String,
}

#[get("/users/{username}")]
async fn view_user(path: web::Path<(String,)>, req: HttpRequest) -> Result<HttpResponse, std::io::Error> {
    let username = path.into_inner().0;

    if let Some(cookie) = req.cookie("netsblox") {
        let requestor = cookie.value();
        if requestor == username {  // FIXME: use actual auth
            Ok(HttpResponse::Ok().json(User{username: username.to_string()}))
        } else {
            Ok(HttpResponse::Unauthorized().finish())
        }
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let client = Client::with_uri_str("mongodb://127.0.0.1:27017/").await.expect("Could not connect to mongodb.");
    let db = client.database("netsblox-tests");
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(db.clone()))
            .service(web::scope("/libraries").configure(libraries::config))
            .service(web::scope("/services-hosts").configure(services_hosts::config))
            .service(web::scope("/users").configure(users::config))
            .service(web::scope("/projects").configure(projects::config))

    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
