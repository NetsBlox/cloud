mod app_data;
mod collaboration_invites;
mod config;
mod database;
mod friends;
mod groups;
mod libraries;
mod models;
mod network;
mod projects;
mod services_hosts;
mod users;

use crate::app_data::AppData;
use crate::config::Settings;
use crate::models::ServiceHost;
use actix_cors::Cors;
use actix_session::{CookieSession, Session};
use actix_web::{cookie::SameSite, get, middleware, web, App, HttpResponse, HttpServer};
use mongodb::Client;
use rusoto_core::credential::StaticProvider;
use rusoto_s3::{CreateBucketRequest, S3Client, S3};
use rusoto_signature::region::Region;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig<'a> {
    client_id: String,
    username: Option<String>,
    services_hosts: Vec<ServiceHost>,
    cloud_url: &'a str,
}

#[get("/configuration")] // TODO: add username?
async fn get_client_config(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: if authenticated,
    //  - [ ] retrieve services hosts
    let default_hosts = app.settings.services_hosts.clone();
    let config = ClientConfig {
        client_id: format!("_netsblox{}", Uuid::new_v4()),
        username: session.get::<String>("username").unwrap_or(None),
        services_hosts: default_hosts,
        cloud_url: &app.settings.public_url,
    };
    Ok(HttpResponse::Ok().json(config))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Settings::new().unwrap();
    // TODO: move the logic below into app_data?
    let client = Client::with_uri_str(&config.database.url)
        .await
        .expect("Could not connect to mongodb.");

    let db = client.database(&config.database.name);
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let region = Region::Custom {
        name: config.s3.region_name,
        endpoint: config.s3.endpoint,
    };

    let s3 = S3Client::new_with(
        rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
        StaticProvider::new(
            config.s3.credentials.access_key,
            config.s3.credentials.secret_key,
            None,
            None,
        ),
        //StaticProvider::from(AwsCredentials::default()),
        region,
    );

    // Create the s3 bucket
    let bucket = config.s3.bucket;
    let request = CreateBucketRequest {
        bucket: bucket.clone(),
        ..Default::default()
    };
    s3.create_bucket(request).await.unwrap();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_header()
            .allow_any_method()
            .supports_credentials();

        App::new()
            .wrap(cors)
            .wrap(
                CookieSession::signed(&[1; 32])
                    //.domain(&config.cookie.domain)  // FIXME: Enable this again
                    .expires_in(2 * 7 * 24 * 60 * 60)
                    .same_site(SameSite::None)
                    .lazy(true)
                    .name(&config.cookie.name)
                    .secure(true),
            )
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(AppData::new(
                Settings::new().unwrap(),
                db.clone(),
                s3.clone(),
                bucket.clone(),
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
    .bind(&config.address)?
    .run()
    .await
}
