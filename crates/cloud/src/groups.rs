use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::users::{ensure_can_edit_user, is_super_user};
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;

use crate::common::{self, api};

#[get("/user/{owner}")]
async fn list_groups(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    let query = doc! {"owner": owner};
    let cursor = app
        .groups
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    let groups: Vec<api::Group> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|g| g.into())
        .collect();

    Ok(HttpResponse::Ok().json(groups))
}

#[get("/id/{id}")]
async fn view_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let username = session
        .get::<String>("username")
        .ok()
        .flatten()
        .ok_or(UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await.unwrap_or(false) {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let group: api::Group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?
        .into();

    Ok(HttpResponse::Ok().json(group))
}

#[get("/id/{id}/members")]
async fn list_members(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    ensure_can_edit_group(&app, &session, &id).await?;
    let query = doc! {"groupId": id};
    let cursor = app.find_users_where(query).await?;
    let members: Vec<api::User> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|u| u.into())
        .collect();

    Ok(HttpResponse::Ok().json(members))
}

pub async fn ensure_can_edit_group(
    app: &AppData,
    session: &Session,
    group_id: &api::GroupId,
) -> Result<(), UserError> {
    let query = doc! {"id": group_id};
    match app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        Some(group) => ensure_can_edit_user(app, session, &group.owner).await,
        None => Err(UserError::GroupNotFoundError),
    }
}

// TODO: Should this send the data, too?
#[post("/user/{owner}")]
async fn create_group(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
    body: web::Json<api::CreateGroupData>,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    let group = common::Group::new(owner.to_owned(), body.name.to_owned());
    let query = doc! {"name": &group.name, "owner": &group.owner};
    let update = doc! {"$setOnInsert": group};
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();
    let result = app
        .groups
        .update_one(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count == 1 {
        Err(UserError::GroupExistsError)
    } else {
        Ok(HttpResponse::Ok().body("Group created."))
    }
}

#[patch("/id/{id}")]
async fn update_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    data: web::Json<api::UpdateGroupData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let username = session
        .get::<String>("username")
        .ok()
        .flatten()
        .ok_or(UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await.unwrap_or(false) {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };
    let update = doc! {"$set": {"name": &data.name}};
    let result = app
        .groups
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.matched_count > 0 {
        Ok(HttpResponse::Ok().body("Group deleted."))
    } else {
        Err(UserError::GroupNotFoundError)
    }
}

#[delete("/id/{id}")]
async fn delete_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let username = session
        .get::<String>("username")
        .ok()
        .flatten()
        .ok_or(UserError::PermissionsError)?;

    let query = if is_super_user(&app, &session).await.unwrap_or(false) {
        doc! {"id": id}
    } else {
        doc! {"id": id, "owner": username}
    };

    let result = app
        .groups
        .delete_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if result.deleted_count > 0 {
        Ok(HttpResponse::Ok().body("Group deleted."))
    } else {
        Err(UserError::GroupNotFoundError)
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_groups)
        .service(view_group)
        .service(list_members)
        .service(update_group)
        .service(delete_group)
        .service(create_group);
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_list_groups() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_groups_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group_404() {
        unimplemented!();
    }
    // TODO: How does it handle malformed IDs?
}
