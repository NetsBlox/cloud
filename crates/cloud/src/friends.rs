use crate::app_data::AppData;
use crate::common::api::{FriendLinkState, UserRole};
use crate::errors::{InternalError, UserError};
use crate::network::topology::GetOnlineUsers;
use crate::users::{ensure_can_edit_user, get_user_role};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use mongodb::bson::doc;
use mongodb::options::CountOptions;

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

    let task = app
        .network
        .send(GetOnlineUsers(filter_usernames))
        .await
        .map_err(InternalError::ActixMessageError)?;
    let online_friends = task.run().await;

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

    // ensure users are valid
    let options = CountOptions::builder().limit(Some(2)).build();
    let query = doc! {
        "$or": [
            {"username": &owner},
            {"username": &recipient},
        ]
    };
    let user_count = app
        .users
        .count_documents(query, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    if user_count != 2 {
        return Err(UserError::UserNotFoundError);
    }

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
    use super::*;
    use actix_web::test;
    use actix_web::{http, web, App};
    use netsblox_cloud_common::FriendLink;
    use netsblox_cloud_common::{
        api::{self, UserRole},
        User,
    };

    use crate::test_utils;

    #[actix_web::test]
    async fn test_list_friends() {
        // Define users
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let f1: User = api::NewUser {
            username: "f1".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let f2: User = api::NewUser {
            username: "f2".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let nonfriend: User = api::NewUser {
            username: "nonfriend".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        // Define the friend relationships
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::APPROVED),
        );
        let l2 = FriendLink::new(
            f2.username.clone(),
            user.username.clone(),
            Some(FriendLinkState::APPROVED),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), f2.clone(), nonfriend])
            .with_friend_links(&[l1, l2])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::get()
                    .uri(&format!("/{}/", &user.username))
                    .cookie(cookie)
                    .to_request();

                let friends: Vec<String> = test::call_and_read_body_json(&app, req).await;
                assert_eq!(friends.len(), 2);
                dbg!(&friends);
                assert!(friends.contains(&f1.username));
                assert!(friends.contains(&f2.username));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_friends_401() {
        // Define users
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/{}/", &user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_online_friends() {
        // Define users
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let f1: User = api::NewUser {
            username: "f1".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let f2: User = api::NewUser {
            username: "f2".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let nonfriend: User = api::NewUser {
            username: "nonfriend".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        // Define the friend relationships
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::APPROVED),
        );
        let l2 = FriendLink::new(
            user.username.clone(),
            f2.username.clone(),
            Some(FriendLinkState::APPROVED),
        );

        // Connect f1, nonfriend
        let c1 = test_utils::network::Client::new(Some(f1.username.clone()), None);
        let c2 = test_utils::network::Client::new(Some(nonfriend.username.clone()), None);
        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), f2, nonfriend])
            .with_friend_links(&[l1, l2])
            .with_clients(&[c1, c2])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::get()
                    .uri(&format!("/{}/online", &user.username))
                    .cookie(cookie)
                    .to_request();

                let friends: Vec<String> = test::call_and_read_body_json(&app, req).await;
                assert_eq!(friends.len(), 1);
                assert_eq!(friends[0], f1.username);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_online_friends_401() {
        // Define users
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/{}/online", &user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unfriend() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unfriend_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_block_user() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_block_user_401() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_invite_user() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_invite_user_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_invite_user_no_duplicates() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite_403() {
        todo!();
    }
}
