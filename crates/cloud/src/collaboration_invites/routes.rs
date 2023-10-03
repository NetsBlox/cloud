use actix_web::{get, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::app_data::AppData;
use crate::auth;
use crate::collaboration_invites::actions::CollaborationInviteActions;
use crate::common::{api::InvitationState, api::ProjectId};
use crate::errors::UserError;

#[get("/user/{receiver}/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (receiver,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &receiver).await?;

    let actions: CollaborationInviteActions = app.to_collab_invite_actions();
    let invites = actions.list_invites(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(invites))
}

#[post("/{project_id}/invite/{receiver}")]
async fn send_invite(
    app: web::Data<AppData>,
    req: HttpRequest,
    path: web::Path<(ProjectId, String)>,
) -> Result<HttpResponse, UserError> {
    let (project_id, receiver) = path.into_inner();
    let auth_ic = auth::collaboration::try_invite(&app, &req, &project_id).await?;

    let actions: CollaborationInviteActions = app.to_collab_invite_actions();
    let invitation = actions.send_invite(&auth_ic, &receiver).await?;

    Ok(HttpResponse::Ok().json(invitation))
}

#[post("/id/{id}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    state: web::Json<InvitationState>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_ri = auth::collaboration::try_respond_to_invite(&app, &req, &id).await?;

    let actions: CollaborationInviteActions = app.to_collab_invite_actions();
    // TODO: what should the arguments be?
    let state = actions.respond(&auth_ri, state.into_inner()).await?;

    Ok(HttpResponse::Ok().json(state))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http, test, web, App};
    use netsblox_cloud_common::{api, CollaborationInvite, User};

    use crate::test_utils;

    #[actix_web::test]
    async fn test_list_invites() {
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let invites: Vec<_> = (1..=10)
            .map(|i| {
                CollaborationInvite::new(
                    format!("sender_{}", i),
                    rcvr.username.clone(),
                    ProjectId::new(format!("project_{}", i)),
                )
            })
            .collect();

        test_utils::setup()
            .with_users(&[rcvr.clone()])
            .with_collab_invites(&invites)
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&rcvr.username))
                    .uri(&format!("/user/{}/", &rcvr.username))
                    .to_request();

                // Ensure that the collaboration invite is returned.
                // This will panic if the response is incorrect so no assert needed.
                let invites: Vec<api::CollaborationInvite> =
                    test::call_and_read_body_json(&app, req).await;

                assert_eq!(invites.len(), 10);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_invites_403() {
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let other_user: User = api::NewUser {
            username: "other_user".to_string(),
            email: "other_user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[rcvr.clone(), other_user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other_user.username))
                    .uri(&format!("/user/{}/", &rcvr.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_invites_admin() {
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(api::UserRole::Admin),
        }
        .into();

        let invites: Vec<_> = (1..=10)
            .map(|i| {
                CollaborationInvite::new(
                    format!("sender_{}", i),
                    rcvr.username.clone(),
                    ProjectId::new(format!("project_{}", i)),
                )
            })
            .collect();

        test_utils::setup()
            .with_users(&[rcvr.clone(), admin.clone()])
            .with_collab_invites(&invites)
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/user/{}/", &rcvr.username))
                    .to_request();

                // Ensure that the collaboration invite is returned.
                // This will panic if the response is incorrect so no assert needed.
                let invites: Vec<api::CollaborationInvite> =
                    test::call_and_read_body_json(&app, req).await;

                assert_eq!(invites.len(), 10);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_send_invite() {
        let sender: User = api::NewUser {
            username: "sender".to_string(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner("sender".to_string())
            .build();

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&sender.username))
                    .uri(&format!("/{}/invite/{}", &project.id, &rcvr.username))
                    .to_request();

                // Ensure that the collaboration invite is returned.
                // This will panic if the response is incorrect so no assert needed.
                let _invite: api::CollaborationInvite =
                    test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_send_invite_403() {
        let other_user: User = api::NewUser {
            username: "other_user".to_string(),
            email: "other_user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let sender: User = api::NewUser {
            username: "sender".to_string(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner("sender".to_string())
            .build();

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone(), other_user.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&other_user.username))
                    .uri(&format!("/{}/invite/{}", &project.id, &rcvr.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_send_invite_exists() {
        let sender: User = api::NewUser {
            username: "sender".to_string(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner("sender".to_string())
            .build();
        let invite = CollaborationInvite::new(
            "anySender".into(),
            rcvr.username.clone(),
            project.id.clone(),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_collab_invites(&[invite])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&sender.username))
                    .uri(&format!("/{}/invite/{}", &project.id, &rcvr.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::CONFLICT);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_send_invite_admin() {
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

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_respond_to_invite_project_deleted() {
        todo!();
    }
}
