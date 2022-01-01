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

use actix_cors::Cors;
use actix_session::{CookieSession, Session};
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer};
use app_data::AppData;
use env_logger;
use models::ServiceHost;
use mongodb::Client;
use rusoto_core::credential::{AwsCredentials, StaticProvider};
use rusoto_s3::{CreateBucketRequest, S3Client, S3};
use rusoto_signature::region::Region;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    client_id: String,
    username: Option<String>,
    services_hosts: Vec<ServiceHost>,
    cloud_url: &'static str,
}

#[get("/configuration")] // TODO: add username?
async fn get_client_config(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: if authenticated,
    //  - [ ] retrieve services hosts
    let default_host = ServiceHost {
        url: "http://localhost:5000/services".to_owned(),
        categories: vec![],
    };
    let config = ClientConfig {
        client_id: format!("_netsblox{}", Uuid::new_v4().to_string()),
        username: session.get::<String>("username").unwrap_or(None),
        services_hosts: vec![default_host],
        cloud_url: "http://localhost:7777",
    };
    Ok(HttpResponse::Ok().json(config))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
        .await
        .expect("Could not connect to mongodb.");
    let db = client.database("netsblox-rs");
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    // TODO: Ensure the bucket exists

    let region = Region::Custom {
        name: "".to_owned(),
        endpoint: "http://127.0.0.1:9000".to_owned(),
    }; // FIXME: Use this for minio but update for aws

    let s3 = S3Client::new_with(
        rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
        StaticProvider::new("KEY".to_owned(), "MYSECRET".to_owned(), None, None),
        //StaticProvider::from(AwsCredentials::default()),
        region,
    );

    // Create the s3 bucket
    let bucket = "netsbloxrs".to_owned();
    let request = CreateBucketRequest {
        bucket: bucket.clone(),
        ..Default::default()
    };
    s3.create_bucket(request).await;

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_header()
            .allow_any_method();

        App::new()
            .wrap(cors)
            .wrap(
                CookieSession::signed(&[1; 32])
                    .domain("localhost:7777")
                    .name("netsblox")
                    .secure(true),
            ) // FIXME: Set the key
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(AppData::new(
                db.clone(),
                s3.clone(),
                bucket.clone(),
                //S3Client::new(region.clone()),
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
