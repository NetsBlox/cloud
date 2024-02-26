mod app_data;
mod auth;
mod collaboration_invites;
mod common;
mod config;
mod errors;
mod friends;
mod galleries;
mod groups;
mod libraries;
mod login_helper;
mod magic_links;
mod network;
mod oauth;
mod projects;
mod services;
#[cfg(test)]
mod test_utils;
mod users;
mod utils;

use crate::common::api;
use crate::config::Settings;
use crate::errors::UserError;
use crate::{app_data::AppData, errors::InternalError};
use actix_cors::Cors;
use actix_session::{
    config::CookieContentSecurity, config::PersistentSession, storage::CookieSessionStore, Session,
    SessionMiddleware,
};
use actix_web::cookie::time::Duration;
use actix_web::{
    cookie::Key, cookie::SameSite, dev::Service, error::ErrorForbidden, get, http::Method,
    middleware, web, App, HttpResponse, HttpServer,
};
use futures::TryStreamExt;
use log::error;
use mongodb::bson::doc;
use mongodb::Client;
use tokio::sync::oneshot;
use uuid::Uuid;

#[get("/configuration")]
async fn get_client_config(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let query = doc! {"visibility": {"$ne": "private"}};
    let default_hosts: Vec<api::ServiceHost> = app
        .authorized_services
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|host| host.into())
        .collect();

    let config = api::ClientConfig {
        client_id: format!("_netsblox{}", Uuid::new_v4()),
        username: session.get::<String>("username").unwrap_or(None),
        services_hosts: default_hosts,
        cloud_url: app.settings.public_url.to_owned(),
    };

    Ok(HttpResponse::Ok().json(config))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = Settings::new().unwrap();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let client = Client::with_uri_str(&config.database.url)
        .await
        .expect("Could not connect to mongodb.");

    let (tx, rx) = oneshot::channel();
    let app_data = AppData::new(client, Settings::new().unwrap(), None, None, Some(tx));
    app_data
        .initialize()
        .await
        .map_err(|err| {
            error!("Error during initialization: {:?}", err);
            err
        })
        .unwrap();

    let address = config.address.clone();
    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_header()
            .allow_any_method()
            .supports_credentials();

        let size_32_mb = 1 << 25;
        App::new()
            .wrap(cors)
            .wrap(app_data.metrics.handler())
            .wrap(session_middleware(&config))
            .wrap(middleware::Logger::default())
            .wrap_fn(|req, srv| {
                let source = req
                    .headers()
                    .get("x-source")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown");
                let allow = *req.method() == Method::GET || source != "NetsBlox";

                let fut = if allow { Some(srv.call(req)) } else { None };
                async {
                    match fut {
                        Some(x) => x.await,
                        None => Err(ErrorForbidden("Operation is not allowed")),
                    }
                }
            })
            .app_data(web::PayloadConfig::new(size_32_mb))
            .app_data(web::JsonConfig::default().limit(size_32_mb))
            .app_data(web::Data::new(app_data.clone()))
            .service(web::scope("/libraries").configure(libraries::routes::config))
            .service(web::scope("/galleries").configure(galleries::routes::config))
            .service(web::scope("/users").configure(users::routes::config))
            .service(web::scope("/projects").configure(projects::routes::config))
            .service(web::scope("/groups").configure(groups::routes::config))
            .service(web::scope("/friends").configure(friends::routes::config))
            .service(web::scope("/magic-links").configure(magic_links::routes::config))
            .service(web::scope("/network").configure(network::routes::config))
            .service(web::scope("/oauth").configure(oauth::routes::config))
            .service(
                web::scope("/collaboration-invites")
                    .configure(collaboration_invites::routes::config),
            )
            .service(web::scope("/services").configure(services::config))
            .service(get_client_config)
    })
    .client_request_timeout(std::time::Duration::from_secs(60))
    .bind(&address)?
    .run();

    // If the network topology is dropped, the server should be stopped as it is unusable
    let handle = server.handle();
    tokio::spawn(async move {
        let _ = rx.await;
        handle.stop(false).await;
    });

    server.await
}

fn session_middleware(config: &Settings) -> SessionMiddleware<CookieSessionStore> {
    let secret_key = Key::from(config.cookie.key.as_bytes());
    let secs_in_week: i64 = 60 * 60 * 24 * 7;

    let mut builder = SessionMiddleware::builder(CookieSessionStore::default(), secret_key)
        .cookie_name(config.cookie.name.clone())
        .cookie_same_site(SameSite::None)
        .cookie_secure(true)
        .cookie_http_only(true)
        .cookie_domain(Some(config.cookie.domain.clone()))
        .cookie_content_security(CookieContentSecurity::Private)
        .session_lifecycle(
            PersistentSession::default().session_ttl(Duration::seconds(secs_in_week)),
        );

    let domain = config.cookie.domain.clone();
    if domain.starts_with("localhost") {
        builder = builder.cookie_domain(Some(domain));
    }

    builder.build()
}
