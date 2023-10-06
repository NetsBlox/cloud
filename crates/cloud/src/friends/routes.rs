use crate::app_data::AppData;
use crate::auth;
use crate::common::api::FriendLinkState;
use crate::errors::UserError;
use crate::friends::actions::FriendActions;
use actix_web::{get, post, HttpRequest};
use actix_web::{web, HttpResponse};

#[get("/{owner}/")]
async fn list_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    let friend_names = actions.list_friends(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(friend_names))
}

#[get("/{owner}/online")]
async fn list_online_friends(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    let online_friends = actions.list_online_friends(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(online_friends))
}

#[post("/{owner}/unfriend/{friend}")]
async fn unfriend(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    actions.unfriend(&auth_eu, &friend).await?;

    // Send "true" since it was successful but there isn't anything to send
    // (w/o adding extra overhead by making assumptions like that they want
    // to see all their friends)
    Ok(HttpResponse::Ok().json(true))
}

#[post("/{owner}/block/{friend}")]
async fn block_user(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    let link = actions.block(&auth_eu, &friend).await?;

    Ok(HttpResponse::Ok().json(link))
}

#[post("/{owner}/unblock/{friend}")]
async fn unblock_user(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, friend) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    actions.unblock(&auth_eu, &friend).await?;

    // Send "true" since it was successful but there isn't anything to send
    // (w/o adding extra overhead by making assumptions like that they want
    // to see all their friends)
    Ok(HttpResponse::Ok().json(true))
}

#[get("/{owner}/invites/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    let invites = actions.list_invites(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(invites))
}

#[post("/{owner}/invite/")]
async fn send_invite(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    recipient: web::Json<String>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let recipient = recipient.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions: FriendActions = app.as_friend_actions();
    let state = actions.send_invite(&auth_eu, &recipient).await?;

    match state {
        FriendLinkState::Blocked => Ok(HttpResponse::Conflict().json(state)),
        _ => Ok(HttpResponse::Ok().json(state)),
    }
}

#[post("/{recipient}/invites/{sender}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    body: web::Json<FriendLinkState>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (recipient, sender) = path.into_inner();
    let state = body.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &recipient).await?;

    let actions: FriendActions = app.as_friend_actions();
    let request = actions.respond_to_invite(&auth_eu, &sender, state).await?;

    Ok(HttpResponse::Ok().json(request))
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
    use std::time::{Duration, SystemTime};

    use super::*;
    use actix_web::{http, test};
    use actix_web::{web, App};
    use mongodb::bson::DateTime;
    use netsblox_cloud_common::{
        api::{self, UserRole},
        User,
    };
    use netsblox_cloud_common::{FriendLink, Group};

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
            Some(FriendLinkState::Approved),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), nonfriend])
            .with_friend_links(&[l1])
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
                assert_eq!(friends.len(), 1);
                assert_eq!(friends[0], f1.username);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_friends_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
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
                    .uri(&format!("/{}/", &other.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
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
            Some(FriendLinkState::Approved),
        );
        let l2 = FriendLink::new(
            user.username.clone(),
            f2.username.clone(),
            Some(FriendLinkState::Approved),
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
    async fn test_list_online_friends_403() {
        // Define users
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        // Connect f1, nonfriend
        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
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
                    .uri(&format!("/{}/online", &other.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_unfriend() {
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
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Approved),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone()])
            .with_friend_links(&[l1])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/unfriend/{}", &user.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let friends = app_data.get_friends(&user.username).await.unwrap();
                assert_eq!(friends.len(), 0);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_unfriend_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other_user: User = api::NewUser {
            username: "other_user".into(),
            email: "other_user@netsblox.org".into(),
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
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Approved),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), other_user.clone()])
            .with_friend_links(&[l1])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&other_user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/unfriend/{}", &user.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);

                let friends = app_data.get_friends(&user.username).await.unwrap();
                assert_eq!(friends.len(), 1);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_unfriend_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
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
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Approved),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), admin.clone()])
            .with_friend_links(&[l1])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&admin.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/unfriend/{}", &user.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let friends = app_data.get_friends(&user.username).await.unwrap();
                assert_eq!(friends.len(), 0);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_block_user() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other_user: User = api::NewUser {
            username: "other_user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone(), other_user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!(
                        "/{}/block/{}",
                        &user.username, &other_user.username
                    ))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_block_user_existing() {
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
        let mut l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Approved),
        );

        // Roll back the creation date to make it obvious that the update time is different
        l1.created_at = DateTime::from_system_time(SystemTime::now() - Duration::from_secs(100));

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone()])
            .with_friend_links(&[l1.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/block/{}", &user.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let link: api::FriendLink = test::call_and_read_body_json(&app, req).await;
                assert!(matches!(link.state, FriendLinkState::Blocked));
                assert_ne!(link.created_at, link.updated_at);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_block_user_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
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
        let mut l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Approved),
        );

        // Roll back the creation date to make it obvious that the update time is different
        l1.created_at = DateTime::from_system_time(SystemTime::now() - Duration::from_secs(100));

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone(), other.clone()])
            .with_friend_links(&[l1.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/block/{}", &other.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_unblock_user() {
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
        let l1 = FriendLink::new(
            user.username.clone(),
            f1.username.clone(),
            Some(FriendLinkState::Blocked),
        );

        test_utils::setup()
            .with_users(&[user.clone(), f1.clone()])
            .with_friend_links(&[l1.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/{}/unblock/{}", &user.username, &f1.username))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_invite_user() {
        let user: User = api::NewUser {
            username: "someUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other_user: User = api::NewUser {
            username: "otherUser".into(),
            email: "otherUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), other_user])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri("/someUser/invite/")
                    .set_json(String::from("otherUser"))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                //let query = doc!{"username": }
                //app_data.users.find_one(query, None).await.unwrap();
                // TODO: check that the invite was sent
            })
            .await;
    }

    #[actix_web::test]
    async fn test_invite_nonexistent_user() {
        let user: User = api::NewUser {
            username: "someUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri("/someUser/invite/")
                    .set_json(String::from("notAUser"))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_invite_user_member() {
        let user: User = api::NewUser {
            username: "someUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());
        let member: User = api::NewUser {
            username: "someMember".into(),
            email: "someMember@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), member])
            .with_groups(&[group])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri("/someUser/invite/")
                    .set_json(String::from("someMember"))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_invite_user_unauth() {
        let user: User = api::NewUser {
            username: "someUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other_user: User = api::NewUser {
            username: "otherUser".into(),
            email: "otherUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user, other_user])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let req = test::TestRequest::post()
                    .uri("/someUser/invite/")
                    .set_json(String::from("target"))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_invite_user_403() {
        let user: User = api::NewUser {
            username: "someUser".into(),
            email: "someUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other_user: User = api::NewUser {
            username: "otherUser".into(),
            email: "otherUser@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let target: User = api::NewUser {
            username: "target".into(),
            email: "target@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user, other_user.clone(), target])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;
                let cookie = test_utils::cookie::new(&other_user.username);
                let req = test::TestRequest::post()
                    .uri("/someUser/invite/")
                    .set_json(String::from("target"))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite() {
        // #[post("/{recipient}/invites/{sender}")]
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite_403() {
        todo!();
    }
}
