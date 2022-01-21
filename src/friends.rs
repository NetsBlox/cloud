use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::{FriendLink, FriendLinkState};
use crate::users::{can_edit_user, ensure_can_edit_user};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::options::UpdateOptions;
use serde::Deserialize;

#[get("/{owner}/")]
async fn list_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> HttpResponse {
    let (owner,) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }

    let friend_names = get_friends(&app, &owner).await;
    HttpResponse::Ok().json(friend_names)
}

async fn get_friends(app: &AppData, owner: &str) -> Vec<String> {
    let query = doc! {"$or": [{"sender": &owner, "state": FriendLinkState::APPROVED}, {"recipient": &owner, "state": FriendLinkState::APPROVED}]};
    let cursor = app.friends.find(query, None).await.unwrap();
    let links = cursor.try_collect::<Vec<_>>().await.unwrap();
    let friend_names: Vec<_> = links
        .into_iter()
        .map(|l| {
            if l.sender == owner {
                l.recipient
            } else {
                l.sender
            }
        })
        .collect();
    friend_names
}

#[get("/{owner}/online")]
async fn list_online_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> HttpResponse {
    let (owner,) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }

    let friend_names = get_friends(&app, &owner).await;
    // TODO: Find the client IDs for these (and filter using them)
    // let online_friends = friend_names.iter().filter_map(|username| app.network.).collect();
    // TODO...
    HttpResponse::Ok().json(friend_names)
}

#[post("/{owner}/unfriend/{friend}")]
async fn unfriend(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    let query = doc! {
        "$or": [
            {"sender": &owner, "recipient": &friend, "state": FriendLinkState::APPROVED},
            {"sender": &friend, "recipient": &owner, "state": FriendLinkState::APPROVED}
        ]
    };
    let result = app
        .friends
        .delete_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

    if result.deleted_count > 0 {
        Ok(HttpResponse::Ok().body("User has been unfriended!"))
    } else {
        Ok(HttpResponse::NotFound().body("Not found."))
    }
}

#[post("/{owner}/block/{friend}")]
async fn block_user(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> HttpResponse {
    let (owner, friend) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }
    let query = doc! {
        "$or": [
            {"sender": &owner, "recipient": &friend},
            {"sender": &friend, "recipient": &owner}
        ]
    };
    app.friends.delete_one(query, None).await.unwrap();

    let link = FriendLink::new(owner, friend, Some(FriendLinkState::BLOCKED));
    app.friends.insert_one(link, None).await.unwrap();
    HttpResponse::Ok().body("User has been blocked.")
}

#[get("/{owner}/invites/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> HttpResponse {
    let (owner,) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }

    let query = doc! {"recipient": &owner};
    let cursor = app.friends.find(query, None).await.unwrap();
    let invites = cursor.try_collect::<Vec<_>>().await.unwrap();
    HttpResponse::Ok().json(invites)
}

#[post("/{owner}/invite/{recipient}")]
async fn send_invite(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> HttpResponse {
    let (owner, recipient) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }

    let query = doc! {
        "$or": [
            {"sender": &owner, "recipient": &recipient},
            {"sender": &recipient, "recipient": &owner}
        ]
    };
    let link = FriendLink::new(owner, recipient, None);
    let update = doc! {"$setOnInsert": link};
    let options = UpdateOptions::builder().upsert(true).build();
    let result = app
        .friends
        .update_one(query, update, options)
        .await
        .unwrap();

    if result.upserted_id.is_some() {
        HttpResponse::Ok().body("Invitation sent.")
    } else {
        // TODO: Should we allow users to send multiple invitations (ie, after rejection)?
        HttpResponse::Conflict().body("Invitation already exists.")
    }
}

#[derive(Deserialize)]
struct InvitationResponse {
    response: FriendLinkState,
}

#[post("/{owner}/invite/{id}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    body: web::Json<InvitationResponse>,
    session: Session,
) -> HttpResponse {
    let (owner, id) = path.into_inner();
    if !can_edit_user(&app, &session, &owner).await {
        return HttpResponse::Unauthorized().body("Not allowed.");
    }
    let new_state = body.into_inner().response;
    // TODO: parse ID as ObjectId
    let query = doc! {"id": id, "recipient": &owner};
    let update = doc! {"$set": {"state": new_state}};
    let result = app.friends.update_one(query, update, None).await.unwrap();
    if result.matched_count > 0 {
        HttpResponse::Ok().body("Responded to invitation.")
    } else {
        HttpResponse::NotFound().body("Invitation not found.")
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_friends)
        .service(list_online_friends)
        .service(block_user)
        .service(unfriend)
        .service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_list_friends() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_friends_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_online_friends() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_online_friends_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_unfriend() {
        todo!();
    }

    #[actix_web::test]
    async fn test_unfriend_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_block_user() {
        todo!();
    }

    #[actix_web::test]
    async fn test_block_user_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_invite_user() {
        todo!();
    }

    #[actix_web::test]
    async fn test_invite_user_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_invite_user_no_duplicates() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite_403() {
        todo!();
    }
}
