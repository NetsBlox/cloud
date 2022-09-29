mod app_data;
mod collaboration_invites;
mod config;
mod errors;
mod friends;
mod groups;
mod libraries;
mod models;
mod network;
mod projects;
mod services;
mod users;

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
use netsblox_core::ClientConfig;
use uuid::Uuid;

#[get("/configuration")]
async fn get_client_config(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    println!(
        "Getting configuration for {:?}",
        // session.get::<String>("username")
        session.entries(),
    );
    let query = doc! {"public": true};
    let default_hosts: Vec<netsblox_core::ServiceHost> = app
        .authorized_services
        .find(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .into_iter()
        .map(|host| host.into())
        .collect();

    let config = ClientConfig {
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

    let app_data = AppData::new(client, Settings::new().unwrap(), None, None);
    app_data
        .initialize()
        .await
        .map_err(|err| {
            error!("Error during initialization: {:?}", err);
            err
        })
        .unwrap();

    let secret_key = Key::from(&[1; 32]); // FIXME: specify this in the config

    let secs_in_week: i64 = 60 * 60 * 24 * 7;
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_header()
            .allow_any_method()
            .supports_credentials();

        App::new()
            .wrap(cors)
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_name(config.cookie.name.clone())
                    .cookie_same_site(SameSite::None)
                    .cookie_secure(true)
                    .cookie_http_only(true)
                    .cookie_domain(Some(config.cookie.domain.clone()))
                    .cookie_content_security(CookieContentSecurity::Private)
                    .session_lifecycle(
                        PersistentSession::default().session_ttl(Duration::seconds(secs_in_week)),
                    )
                    .build(),
            )
            .wrap(middleware::Logger::default())
            .wrap_fn(|req, srv| {
                let source = req
                    .headers()
                    .get("x-source")
                    .map(|v| v.to_str().ok())
                    .flatten()
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
            .app_data(web::Data::new(app_data.clone()))
            .service(web::scope("/libraries").configure(libraries::config))
            .service(web::scope("/users").configure(users::config))
            .service(web::scope("/projects").configure(projects::config))
            .service(web::scope("/groups").configure(groups::config))
            .service(web::scope("/friends").configure(friends::config))
            .service(web::scope("/network").configure(network::config))
            .service(web::scope("/collaboration-invites").configure(collaboration_invites::config))
            .service(web::scope("/services").configure(services::config))
            .service(get_client_config)
    })
    .bind(&config.address)?
    .run()
    .await
}
