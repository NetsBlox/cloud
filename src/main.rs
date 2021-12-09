mod app_data;
mod collaboration_invites;
mod database;
mod friends;
mod groups;
mod libraries;
mod network;
mod projects;
mod services_hosts;
mod users;

use actix_session::CookieSession;
use actix_web::get;
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use app_data::AppData;
use env_logger;
use mongodb::Client;
use serde::Serialize;

////////////// Users //////////////
#[derive(Serialize)]
struct User {
    username: String,
}

#[get("/users/{username}")]
async fn view_user(
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, std::io::Error> {
    let username = path.into_inner().0;

    if let Some(cookie) = req.cookie("netsblox") {
        let requestor = cookie.value();
        if requestor == username {
            // FIXME: use actual auth
            Ok(HttpResponse::Ok().json(User {
                username: username.to_string(),
            }))
        } else {
            Ok(HttpResponse::Unauthorized().finish())
        }
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
        .await
        .expect("Could not connect to mongodb.");
    let db = client.database("netsblox-tests"); // TODO: make a custom struct that wraps the collection fns
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(
                CookieSession::signed(&[1; 32])
                    .domain("localhost:8080")
                    .name("netsblox")
                    .secure(true),
            ) // FIXME: Set the key
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(AppData::new(db.clone(), None, None)))
            .service(web::scope("/libraries").configure(libraries::config))
            .service(web::scope("/services-hosts").configure(services_hosts::config))
            .service(web::scope("/users").configure(users::config))
            .service(web::scope("/projects").configure(projects::config))
            .service(web::scope("/groups").configure(groups::config))
            .service(web::scope("/friends").configure(friends::config))
            .service(web::scope("/network").configure(network::config))
            .service(web::scope("/collaboration-invites").configure(collaboration_invites::config))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
