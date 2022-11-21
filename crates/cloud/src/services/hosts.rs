use crate::app_data::AppData;
use crate::common::api;
use crate::common::api::{GroupId, ServiceHost};
use crate::common::AuthorizedServiceHost;
use crate::errors::{InternalError, UserError};
use crate::users::{ensure_can_edit_user, ensure_is_super_user, is_super_user};
use actix_session::Session;
use actix_web::{delete, get, post, HttpRequest};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::options::UpdateOptions;
use regex::Regex;

#[get("/group/{id}")]
async fn list_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let username = session
        .get::<String>("username")
        .ok()
        .flatten()
        .ok_or(UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await? {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;

    Ok(HttpResponse::Ok().json(group.services_hosts.unwrap_or_default()))
}

#[post("/group/{id}")]
async fn set_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(GroupId,)>,
    hosts: web::Json<Vec<ServiceHost>>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let username = session
        .get::<String>("username")
        .ok()
        .flatten()
        .ok_or(UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await? {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let update = doc! {"$set": {"servicesHosts": &hosts.into_inner()}};
    let result = app
        .groups
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    if result.matched_count > 0 {
        Ok(HttpResponse::Ok().body("Group updated"))
    } else {
        Err(UserError::GroupNotFoundError)
    }
}

#[get("/user/{username}")]
async fn list_user_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let username = path.into_inner().0;

    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": username};
    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    Ok(HttpResponse::Ok().json(user.services_hosts.unwrap_or_default()))
}

#[post("/user/{username}")]
async fn set_user_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    hosts: web::Json<Vec<ServiceHost>>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let username = path.into_inner().0;
    ensure_can_edit_user(&app, &session, &username).await?;

    let service_hosts: Vec<ServiceHost> = hosts.into_inner();
    let update = doc! {"$set": {"servicesHosts": &service_hosts }};
    let filter = doc! {"username": username};
    let result = app
        .users
        .update_one(filter, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.modified_count == 1 {
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/all/{username}")]
async fn list_all_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": &username};
    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let mut groups = app
        .groups
        .find(doc! {"owner": &username}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if let Some(group_id) = user.group_id {
        if let Some(in_group) = app
            .groups
            .find_one(doc! {"id": group_id}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            groups.push(in_group);
        }
    };

    let services_hosts = user.services_hosts.unwrap_or_default().into_iter().chain(
        groups
            .into_iter()
            .flat_map(|g| g.services_hosts.unwrap_or_default()),
    );
    Ok(HttpResponse::Ok().json(services_hosts.collect::<Vec<_>>()))
}

#[get("/authorized/")]
async fn get_authorized_hosts(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let query = doc! {};
    let cursor = app
        .authorized_services
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let hosts: Vec<api::AuthorizedServiceHost> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|invite| invite.into())
        .collect();

    Ok(HttpResponse::Ok().json(hosts))
}

#[post("/authorized/")]
async fn authorize_host(
    app: web::Data<AppData>,
    host_data: web::Json<api::AuthorizedServiceHost>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_valid_service_id(&host_data.id)?;
    ensure_is_super_user(&app, &session).await?;

    let query = doc! {"id": &host_data.id};
    let host: AuthorizedServiceHost = host_data.into_inner().into();
    let update = doc! {"$setOnInsert": &host};
    let options = UpdateOptions::builder().upsert(true).build();
    let result = app
        .authorized_services
        .update_one(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count > 0 {
        Err(UserError::ServiceHostAlreadyAuthorizedError)
    } else {
        Ok(HttpResponse::Ok().json(host.secret))
    }
}

#[delete("/authorized/{id}")]
async fn unauthorize_host(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let (client_id,) = path.into_inner();
    let query = doc! {"id": &client_id};
    let result = app
        .authorized_services
        .delete_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.deleted_count == 0 {
        Err(UserError::ServiceHostNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

pub async fn ensure_is_authorized_host(app: &AppData, req: &HttpRequest) -> Result<(), UserError> {
    let query = req
        .headers()
        .get("X-Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value_str| {
            let mut chunks = value_str.split(':');
            let id = chunks.next();
            let secret = chunks.next();
            id.and_then(|id| secret.map(|s| (id, s)))
        })
        .map(|(id, secret)| doc! {"id": id, "secret": secret})
        .ok_or(UserError::PermissionsError)?; // permissions error since there are no credentials

    app.authorized_services
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::PermissionsError)?;

    Ok(())
}

pub fn ensure_valid_service_id(id: &str) -> Result<(), UserError> {
    let max_len = 25;
    let min_len = 3;
    let char_count = id.chars().count();
    lazy_static! {
        // This is safe to unwrap since it is a constant
        static ref SERVICE_ID_REGEX: Regex = Regex::new(r"^[A-Za-z][A-Za-z0-9_\-]+$").unwrap();
    }

    let is_valid = char_count > min_len && char_count < max_len && SERVICE_ID_REGEX.is_match(id);
    if is_valid {
        Ok(())
    } else {
        Err(UserError::InvalidServiceHostIDError)
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_group_hosts)
        .service(set_group_hosts)
        .service(list_user_hosts)
        .service(set_user_hosts)
        .service(list_all_hosts)
        .service(authorize_host)
        .service(get_authorized_hosts)
        .service(unauthorize_host);
}

mod test {}
