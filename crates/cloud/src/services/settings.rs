use actix_session::Session;
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;

use crate::common::api;
use crate::{
    app_data::AppData,
    errors::{InternalError, UserError},
    groups::ensure_can_edit_group,
    services::ensure_is_authorized_host,
    users::ensure_can_edit_user,
};

#[get("/user/{username}/")]
async fn list_user_hosts_with_settings(
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

    let hosts: Vec<_> = user.service_settings.keys().collect();
    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/user/{username}/{host}")]
async fn get_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": &username};
    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let default_settings = String::from("");
    let settings = user
        .service_settings
        .get(&host)
        .unwrap_or(&default_settings);

    Ok(HttpResponse::Ok().body(settings.to_owned()))
}

#[get("/user/{username}/{host}/all")]
async fn get_all_settings(
    req: HttpRequest,
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    if ensure_is_authorized_host(&app, &req).await.is_err() {
        ensure_can_edit_user(&app, &session, &username).await?;
    }

    let query = doc! {"username": &username};
    let user = app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let query = match user.group_id {
        Some(ref group_id) => doc! {"$or": [
            {"owner": &username},
            {"id": group_id}
        ]},
        None => doc! {"owner": username},
    };
    let cursor = app
        .groups
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let mut groups: Vec<_> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let member_settings = user
        .group_id
        .and_then(|group_id| groups.iter().position(|group| group.id == group_id))
        .map(|pos| groups.swap_remove(pos))
        .and_then(|group| group.service_settings.get(&host).map(|s| s.to_owned()));

    let all_settings = api::ServiceSettings {
        user: user.service_settings.get(&host).cloned(),
        member: member_settings,
        groups: groups
            .into_iter()
            .filter_map(|group| group.service_settings.get(&host).map(|s| s.to_owned()))
            .collect(),
    };

    Ok(HttpResponse::Ok().json(all_settings))
}

#[post("/user/{username}/{host}")]
async fn set_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    body: web::Bytes,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let settings = std::str::from_utf8(&body).map_err(|_err| UserError::InternalError)?;

    let query = doc! {"username": &username};
    let update = doc! {"$set": {format!("serviceSettings.{}", &host): settings}};

    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[delete("/user/{username}/{host}")]
async fn delete_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"username": &username};
    let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};

    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[get("/group/{group_id}/")]
async fn list_group_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (group_id,) = path.into_inner();
    ensure_can_edit_group(&app, &session, &group_id).await?;

    let query = doc! {"id": &group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let hosts: Vec<_> = group.service_settings.keys().collect();
    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/group/{group_id}/{host}")]
async fn get_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    ensure_can_edit_group(&app, &session, &group_id).await?;

    let query = doc! {"id": &group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;

    let default_settings = String::from("");
    let settings = group
        .service_settings
        .get(&host)
        .unwrap_or(&default_settings);

    Ok(HttpResponse::Ok().body(settings.to_owned()))
}

#[post("/group/{group_id}/{host}")]
async fn set_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    ensure_can_edit_group(&app, &session, &group_id).await?;

    let query = doc! {"id": &group_id};
    let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};

    let result = app
        .groups
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Err(UserError::GroupNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[delete("/group/{group_id}/{host}")]
async fn delete_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    ensure_can_edit_group(&app, &session, &group_id).await?;

    let query = doc! {"id": &group_id};
    let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};

    let result = app
        .users
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 0 {
        Err(UserError::UserNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_user_settings)
        .service(set_user_settings)
        .service(list_user_hosts_with_settings)
        .service(get_all_settings)
        .service(delete_user_settings)
        .service(get_group_settings)
        .service(set_group_settings)
        .service(list_group_hosts_with_settings)
        .service(delete_group_settings);
}
