use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::users::{can_edit_user, ensure_can_edit_user, is_super_user};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;
use netsblox_core::{GroupId, ServiceHost};

#[get("/group/{id}")]
async fn list_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let username = session
        .get::<String>("username")
        .unwrap()
        .ok_or_else(|| UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await? {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?
        .ok_or_else(|| UserError::GroupNotFoundError)?;

    Ok(HttpResponse::Ok().json(group.services_hosts.unwrap_or_else(Vec::new)))
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
        .unwrap()
        .ok_or_else(|| UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await? {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let update = doc! {"$set": {"servicesHosts": &hosts.into_inner()}};
    let result = app.groups.update_one(query, update, None).await.unwrap();
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
        .map_err(|_err| InternalError::DatabaseConnectionError)?
        .ok_or_else(|| UserError::UserNotFoundError)?;

    Ok(HttpResponse::Ok().json(user.services_hosts.unwrap_or_else(Vec::new)))
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
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

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
        .map_err(|_err| InternalError::DatabaseConnectionError)?
        .ok_or_else(|| UserError::UserNotFoundError)?;

    let mut groups = app
        .groups
        .find(doc! {"owner": &username}, None)
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    if let Some(group_id) = user.group_id {
        if let Some(in_group) = app
            .groups
            .find_one(doc! {"id": group_id}, None)
            .await
            .map_err(|_err| InternalError::DatabaseConnectionError)?
        {
            groups.push(in_group);
        }
    };

    let services_hosts = user
        .services_hosts
        .unwrap_or_else(Vec::new)
        .into_iter()
        .chain(
            groups
                .into_iter()
                .flat_map(|g| g.services_hosts.unwrap_or_else(Vec::new)),
        );
    Ok(HttpResponse::Ok().json(services_hosts.collect::<Vec<_>>()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_group_hosts)
        .service(set_group_hosts)
        .service(list_user_hosts)
        .service(set_user_hosts)
        .service(list_all_hosts);
}

mod test {}
