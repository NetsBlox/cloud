mod app_data;
mod collaboration_invites;
mod database;
mod friends;
mod groups;
mod libraries;
mod models;
mod network;
mod projects;
mod services_hosts;
mod users;

use actix_session::{CookieSession, Session};
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer};
use app_data::AppData;
use env_logger;
use models::ServiceHost;
use mongodb::{bson::oid::ObjectId, Client};
use rusoto_s3::S3Client;
use rusoto_signature::region::Region;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
struct ClientConfig {
    client_id: String,
    services_hosts: Vec<ServiceHost>,
}

#[get("/configuration")] // TODO: add username?
async fn get_client_config(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: check if authenticated?
    // Fetch sessions
    let config = ClientConfig {
        client_id: format!("_netsblox{}", Uuid::new_v4().to_string()),
        services_hosts: vec![],
    };
    Ok(HttpResponse::Ok().json(config))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
        .await
        .expect("Could not connect to mongodb.");
    let db = client.database("netsblox-tests"); // TODO: make a custom struct that wraps the collection fns
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let region = Region::Custom {
        name: "".to_owned(),
        endpoint: "http://localhost:9000".to_owned(),
    }; // FIXME: Use this for minio but update for aws

    HttpServer::new(move || {
        App::new()
            .wrap(
                CookieSession::signed(&[1; 32])
                    .domain("localhost:7777")
                    .name("netsblox")
                    .secure(true),
            ) // FIXME: Set the key
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(AppData::new(
                db.clone(),
                S3Client::new(region.clone()),
                None,
                None,
            )))
            .service(web::scope("/libraries").configure(libraries::config))
            .service(web::scope("/services-hosts").configure(services_hosts::config))
            .service(web::scope("/users").configure(users::config))
            .service(web::scope("/projects").configure(projects::config))
            .service(web::scope("/groups").configure(groups::config))
            .service(web::scope("/friends").configure(friends::config))
            .service(web::scope("/network").configure(network::config))
            .service(web::scope("/collaboration-invites").configure(collaboration_invites::config))
            .service(get_client_config)
    })
    .bind("127.0.0.1:7777")?
    .run()
    .await
}
