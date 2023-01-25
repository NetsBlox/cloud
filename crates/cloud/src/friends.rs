use crate::app_data::AppData;
use crate::common::api::{FriendLinkState, UserRole};
use crate::errors::UserError;
use crate::network::topology;
use crate::users::{ensure_can_edit_user, get_user_role};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};

#[get("/{owner}/")]
async fn list_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    // Admins are considered a friend to everyone (at least one-way)
    let friend_names: Vec<_> = app.get_friends(&owner).await?;

    Ok(HttpResponse::Ok().json(friend_names))
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
        Some(app.get_friends(&owner).await?)
    };

    let online_friends = topology::get_online_users(filter_usernames).await;

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

    app.unfriend(&owner, &friend).await?;

    Ok(HttpResponse::Ok().body("User has been unfriended!"))
}

#[post("/{owner}/block/{friend}")]
async fn block_user(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;
    app.block_user(&owner, &friend).await?;

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
    app.unblock_user(&owner, &friend).await?;

    Ok(HttpResponse::Ok().body("User has been unblocked."))
}

#[get("/{owner}/invites/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &owner).await?;

    let invites = app.list_invites(&owner).await?;

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
    let state = app.send_invite(&owner, &recipient).await?;

    match state {
        FriendLinkState::PENDING => Ok(HttpResponse::Ok().body("Invitation sent.")),
        FriendLinkState::APPROVED => Ok(HttpResponse::Ok().body("Accepted friend request.")),
        FriendLinkState::BLOCKED => {
            Ok(HttpResponse::Conflict().body("Cannot send request when blocked."))
        }
        _ => unreachable!(),
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
    app.response_to_invite(&recipient, &sender, new_state)
        .await?;

    Ok(HttpResponse::Ok().body("Responded to invitation."))
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
