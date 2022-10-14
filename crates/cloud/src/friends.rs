use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::{FriendLink, User};
use crate::network::topology;
use crate::users::{ensure_can_edit_user, get_user_role, is_super_user};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::UpdateOptions;
use netsblox_core::{FriendInvite, FriendLinkState, UserRole};

#[get("/{owner}/")]
async fn list_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    // Admins are considered a friend to everyone (at least one-way)
    let is_universal_friend = matches!(get_user_role(&app, &owner).await?, UserRole::Admin);

    let friend_names = if is_universal_friend {
        app.users
            .find(doc! {}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<User>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|user| user.username)
            .filter(|username| username != &owner)
            .collect()
    } else {
        get_friends(&app, &owner).await
    };

    Ok(HttpResponse::Ok().json(friend_names))
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
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();

    ensure_can_edit_user(&app, &session, &owner).await?;

    let is_universal_friend = matches!(get_user_role(&app, &owner).await?, UserRole::Admin);
    let filter_usernames = if is_universal_friend {
        None
    } else {
        Some(get_friends(&app, &owner).await)
    };

    let online_friends = app
        .network
        .send(topology::GetOnlineUsers {
            usernames: filter_usernames,
        })
        .await
        .map_err(|_err| UserError::InternalError)?;

    Ok(HttpResponse::Ok().json(online_friends))
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
        .map_err(InternalError::DatabaseConnectionError)?;

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
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;
    let query = doc! {
        "$or": [
            {"sender": &owner, "recipient": &friend},
            {"sender": &friend, "recipient": &owner}
        ]
    };
    app.friends.delete_one(query, None).await.unwrap();

    let link = FriendLink::new(owner, friend, Some(FriendLinkState::BLOCKED));
    app.friends.insert_one(link, None).await.unwrap();
    Ok(HttpResponse::Ok().body("User has been blocked."))
}

#[post("/{owner}/unblock/{friend}")]
async fn unblock_user(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;
    let query = doc! {
        "sender": &owner,
        "recipient": &friend,
        "state": FriendLinkState::BLOCKED,
    };
    let result = app.friends.delete_one(query, None).await.unwrap();
    if result.deleted_count == 1 {
        Ok(HttpResponse::Ok().body("User has been unblocked."))
    } else {
        Ok(HttpResponse::Conflict().body("Could not unblock user."))
    }
}

#[get("/{owner}/invites/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    let query = doc! {"recipient": &owner, "state": FriendLinkState::PENDING}; // TODO: ensure they are still pending
    let cursor = app.friends.find(query, None).await.unwrap();
    let invites: Vec<FriendInvite> = cursor
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
        .into_iter()
        .map(|link| link.into())
        .collect();

    Ok(HttpResponse::Ok().json(invites))
}

#[post("/{owner}/invite/")]
async fn send_invite(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    recipient: web::Json<String>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let recipient = recipient.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    // TODO: ensure usernames are valid and not the same
    let query = doc! {
        "sender": &recipient,
        "recipient": &owner,
        "state": FriendLinkState::PENDING
    };

    let update = doc! {"$set": {"state": FriendLinkState::APPROVED}};
    let approved_existing = app
        .friends
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .modified_count
        > 0;

    if approved_existing {
        return Ok(HttpResponse::Ok().body("Approved existing friend request"));
    }

    let query = doc! {
        "$or": [
            {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::BLOCKED},
            {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::BLOCKED},
            {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::APPROVED},
            {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::APPROVED},
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
        Ok(HttpResponse::Ok().body("Invitation sent."))
    } else {
        // TODO: Should we allow users to send multiple invitations (ie, after rejection)?
        Ok(HttpResponse::Conflict().body("Invitation already exists."))
    }
}

#[post("/{recipient}/invites/{sender}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    body: web::Json<FriendLinkState>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (recipient, sender) = path.into_inner();
    ensure_can_edit_user(&app, &session, &recipient).await?;
    let new_state = body.into_inner();
    let query = doc! {"recipient": &recipient, "sender": &sender};
    let update = doc! {"$set": {"state": new_state}};
    let result = app.friends.update_one(query, update, None).await.unwrap();
    if result.matched_count > 0 {
        Ok(HttpResponse::Ok().body("Responded to invitation."))
    } else {
        Ok(HttpResponse::NotFound().body("Invitation not found."))
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_friends)
        .service(list_online_friends)
        .service(block_user)
        .service(unblock_user)
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
