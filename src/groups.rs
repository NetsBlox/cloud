use crate::app_data::AppData;
use crate::models::{Group, User};
use crate::users::{can_edit_user, is_super_user};
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[get("/user/{owner}")]
async fn list_groups(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner,) = path.into_inner();
    if !is_allowed(&app, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }
    let query = doc! {"owner": owner};
    let cursor = app.groups.find(query, None).await.unwrap();
    let groups = cursor.try_collect::<Vec<Group>>().await.unwrap();
    Ok(HttpResponse::Ok().json(groups))
}

#[get("/id/{id}")]
async fn view_group(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (id,) = path.into_inner();
    if let Some(username) = session.get::<String>("username").unwrap() {
        match ObjectId::parse_str(id) {
            Ok(id) => {
                let query = if is_super_user(&app, &session).await {
                    doc! {"_id": id}
                } else {
                    doc! {"_id": id, "owner": username}
                };
                if let Some(group) = app.groups.find_one(query, None).await.unwrap() {
                    Ok(HttpResponse::Ok().json(group))
                } else {
                    Ok(HttpResponse::NotFound().body("Not found."))
                }
            }
            Err(_err) => Ok(HttpResponse::NotFound().body("Not found.")),
        }
    } else {
        Ok(HttpResponse::Unauthorized().body("Not allowed."))
    }
}

#[get("/id/{id}/members")]
async fn list_members(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (id,) = path.into_inner();
    match ObjectId::parse_str(id) {
        Ok(id) => {
            let query = doc! {"_id": id};
            match app.groups.find_one(query, None).await.unwrap() {
                Some(group) => {
                    if !is_allowed(&app, &session, &group.owner).await {
                        return Ok(HttpResponse::Unauthorized().body("Not allowed"));
                    }
                    let query = doc! {"groupId": group._id};
                    let cursor = app.users.find(query, None).await.unwrap();
                    let members = cursor.try_collect::<Vec<User>>().await.unwrap();
                    Ok(HttpResponse::Ok().json(members))
                }
                None => Ok(HttpResponse::NotFound().body("Not found.")),
            }
        }
        Err(_err) => Ok(HttpResponse::NotFound().body("Not found.")),
    }
}

#[post("/user/{owner}/{name}")]
async fn create_group(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }
    let group = doc! {"owner": &owner, "name": &name};
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();
    let result = app
        .groups
        .update_one(group.clone(), group, options)
        .await
        .unwrap();

    if result.matched_count == 1 {
        Ok(HttpResponse::Conflict().body("Group with name already exists."))
    } else {
        Ok(HttpResponse::Ok().body("Group created."))
    }
}

#[derive(Deserialize)]
struct UpdateGroupData {
    name: String,
}

#[patch("/id/{id}")]
async fn update_group(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    data: web::Json<UpdateGroupData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (id,) = path.into_inner();
    if let Some(username) = session.get::<String>("username").unwrap() {
        let query = if is_super_user(&app, &session).await {
            doc! {"_id": id}
        } else {
            doc! {"_id": id, "owner": username}
        };
        let update = doc! {"$set": {"name": &data.name}};
        let result = app.groups.update_one(query, update, None).await.unwrap();
        if result.matched_count > 0 {
            Ok(HttpResponse::Ok().body("Group deleted."))
        } else {
            Ok(HttpResponse::NotFound().body("Not found."))
        }
    } else {
        Ok(HttpResponse::Unauthorized().body("Not allowed."))
    }
}

#[delete("/id/{id}")]
async fn delete_group(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (id,) = path.into_inner();
    if let Some(username) = session.get::<String>("username").unwrap() {
        let query = if is_super_user(&app, &session).await {
            doc! {"_id": id}
        } else {
            doc! {"_id": id, "owner": username}
        };
        let result = app.groups.delete_one(query, None).await.unwrap();
        if result.deleted_count > 0 {
            Ok(HttpResponse::Ok().body("Group deleted."))
        } else {
            Ok(HttpResponse::NotFound().body("Not found."))
        }
    } else {
        Ok(HttpResponse::Unauthorized().body("Not allowed."))
    }
}

async fn is_allowed(app: &AppData, session: &Session, owner: &str) -> bool {
    can_edit_user(&app, &session, owner).await
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
    use super::*;
    use actix_web::{http, test, App};
    use mongodb::{Client, Collection, Database};

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
