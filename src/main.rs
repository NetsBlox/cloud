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
use actix_web::{middleware, web, App, HttpServer};
use app_data::AppData;
use env_logger;
use mongodb::Client;
use rusoto_s3::S3Client;
use rusoto_signature::region::Region;

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
    let s3 = S3Client::new(region);

    HttpServer::new(move || {
        App::new()
            .wrap(
                CookieSession::signed(&[1; 32])
                    .domain("localhost:8080")
                    .name("netsblox")
                    .secure(true),
            ) // FIXME: Set the key
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(AppData::new(db.clone(), s3, None, None)))
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
