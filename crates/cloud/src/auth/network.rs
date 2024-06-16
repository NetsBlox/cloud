use super::{can_edit_project, is_super_user};
use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::network::topology;
use crate::utils;
use actix_web::HttpRequest;
use netsblox_cloud_common::api::{self, ClientId};
use netsblox_cloud_common::ProjectMetadata;

pub(crate) struct ViewClient {
    pub(crate) id: ClientId,
    _private: (),
}

pub(crate) struct EvictClient {
    pub(crate) project: Option<ProjectMetadata>,
    pub(crate) id: ClientId,
    _private: (),
}

pub(crate) struct ListActiveRooms {
    _private: (),
}

pub(crate) struct ListClients {
    _private: (),
}

pub(crate) struct SendMessage {
    _private: (),
    pub(crate) msg: api::SendMessage,
}

pub(crate) async fn try_view_client(
    app: &AppData,
    req: &HttpRequest,
    client_id: &api::ClientId,
) -> Result<ViewClient, UserError> {
    let is_auth_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .is_some();

    if is_auth_host || is_super_user(app, req).await? {
        Ok(ViewClient {
            id: client_id.to_owned(),
            _private: (),
        })
    } else if utils::get_username(req).is_some() {
        Err(UserError::PermissionsError)
    } else {
        Err(UserError::LoginRequiredError)
    }
}

// helper function
pub(crate) async fn ensure_is_auth_host_or_admin(
    app: &AppData,
    req: &HttpRequest,
) -> Result<(), UserError> {
    let is_auth_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .is_some();

    if is_auth_host || is_super_user(app, req).await? {
        Ok(())
    } else if utils::get_username(req).is_some() {
        Err(UserError::PermissionsError)
    } else {
        Err(UserError::LoginRequiredError)
    }
}

pub(crate) async fn try_evict_client(
    app: &AppData,
    req: &HttpRequest,
    client_id: &api::ClientId,
) -> Result<EvictClient, UserError> {
    // client can be evicted by anyone who can edit the browser project
    let project = get_project_for_client(app, client_id).await?;
    if let Some(metadata) = project.clone() {
        if can_edit_project(app, req, Some(client_id), &metadata)
            .await
            .is_ok()
        {
            return Ok(EvictClient {
                project,
                id: client_id.to_owned(),
                _private: (),
            });
        }
    }

    // or by anyone who can edit the corresponding user
    let task = app
        .network
        .send(topology::GetClientUsername(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;

    let username = task.run().await.ok_or(UserError::PermissionsError)?;

    let _auth_eu = super::try_edit_user(app, req, None, &username).await?;

    Ok(EvictClient {
        project,
        id: client_id.to_owned(),
        _private: (),
    })
}

pub(crate) async fn try_list_rooms(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListActiveRooms, UserError> {
    if is_super_user(app, req).await? {
        Ok(ListActiveRooms { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_list_clients(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListClients, UserError> {
    if is_super_user(app, req).await? {
        Ok(ListClients { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_send_message(
    app: &AppData,
    req: &HttpRequest,
    msg: api::SendMessage,
) -> Result<SendMessage, UserError> {
    // Allow extension messages where the inner msg type is prefixed with "unauth".
    // Check out the tests for an example.
    let is_unauth_ok = msg
        .content
        .as_object()
        .and_then(|msg| {
            msg.get("type").and_then(|name| {
                if name == "extension" {
                    msg.get("data")
                } else {
                    None
                }
            })
        })
        .and_then(|inner_msg| inner_msg.as_object())
        .and_then(|inner_msg| inner_msg.get("type"))
        .and_then(|inner_type| inner_type.as_str())
        .map(|inner_type| inner_type.starts_with("unauth:"))
        .unwrap_or(false);

    if is_unauth_ok {
        return Ok(SendMessage { _private: (), msg });
    }

    let host = utils::get_authorized_host(&app.authorized_services, req).await?;

    // Sending messages is allowed if you:
    // - are an authorized host
    if let Some(_host) = host {
        Ok(SendMessage { _private: (), msg })
    // - or can edit (ie, operate on behalf of) the sender
    } else if let Some(sender) = msg.sender.as_ref() {
        // check if we have permissions to edit sender
        let username = match sender {
            api::SendMessageSender::Username(username) => Some(username.clone()),
            api::SendMessageSender::Client(client_id) => {
                let task = app
                    .network
                    .send(topology::GetClientUsername(client_id.clone()))
                    .await
                    .map_err(InternalError::ActixMessageError)?;
                task.run().await
            }
        }
        .ok_or(UserError::PermissionsError)?; // must be an authorized host

        super::try_edit_user(app, req, None, &username)
            .await
            .map(|_eu| SendMessage { _private: (), msg })
    } else {
        Err(UserError::PermissionsError)
    }
}

async fn get_project_for_client(
    app: &AppData,
    client_id: &ClientId,
) -> Result<Option<ProjectMetadata>, UserError> {
    let task = app
        .network
        .send(topology::GetClientState(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;

    let client_state = task.run().await;
    let project_id = client_state.and_then(|state| match state {
        api::ClientState::Browser(api::BrowserClientState { project_id, .. }) => Some(project_id),
        _ => None,
    });

    let metadata = if let Some(id) = project_id {
        Some(app.get_project_metadatum(&id).await?)
    } else {
        None
    };

    Ok(metadata)
}

#[cfg(test)]
impl ViewClient {
    pub(crate) fn test(id: ClientId) -> Self {
        Self { id, _private: () }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{http, post, test, web, App, HttpResponse};
    use netsblox_cloud_common::{AuthorizedServiceHost, User};
    use serde_json::json;

    use super::*;
    use crate::{errors::UserError, test_utils};

    #[actix_web::test]
    async fn test_try_send_msg_auth_host() {
        let msg = api::SendMessage {
            sender: None,
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };
        let visibility = api::ServiceHostScope::Public(Vec::new());
        let host = AuthorizedServiceHost::new(
            "http://localhost:5656".into(),
            api::ServiceID::new("TestServices"),
            visibility,
        );

        test_utils::setup()
            .with_authorized_services(&[host.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .append_header(host.auth_header())
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_self() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let msg = api::SendMessage {
            sender: Some(api::SendMessageSender::Username(user.username.clone())),
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_self_client_id() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let sender_client = test_utils::network::Client::new(Some(user.username.clone()), None);
        let msg = api::SendMessage {
            sender: Some(api::SendMessageSender::Client(sender_client.id.clone())),
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_clients(&[sender_client])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_admin() {
        let msg = api::SendMessage {
            sender: Some(api::SendMessageSender::Username("user".into())),
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };
        let admin: User = api::NewUser {
            username: api::Username::new("admin"),
            email: api::Email::new("admin@netsblox.org"),
            password: None,
            group_id: None,
            role: Some(api::UserRole::Admin),
        }
        .into();
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[admin.clone(), user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_moderator() {
        let msg = api::SendMessage {
            sender: Some(api::SendMessageSender::Username("user".into())),
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };
        let moderator: User = api::NewUser {
            username: api::Username::new("moderator"),
            email: api::Email::new("moderator@netsblox.org"),
            password: None,
            group_id: None,
            role: Some(api::UserRole::Moderator),
        }
        .into();
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[moderator.clone(), user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&moderator.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_other_user() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other_user: User = api::NewUser {
            username: api::Username::new("other_user"),
            email: api::Email::new("other_user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let msg = api::SendMessage {
            sender: Some(api::SendMessageSender::Username(user.username.clone())),
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };

        test_utils::setup()
            .with_users(&[other_user.clone(), user])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&other_user.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_other_user_no_sender() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let msg = api::SendMessage {
            sender: None,
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({"test": "hello!"}),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_send_msg_unauth() {
        let msg = api::SendMessage {
            sender: None,
            target: api::SendMessageTarget::Client {
                client_id: ClientId::new("_test_client_id".into()),
                state: None,
            },
            content: json!({
                "type": "extension",
                "data": {
                    "type": "unauth:test",
                }
            }),
        };

        test_utils::setup()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(send_msg_test),
                )
                .await;

                let req = test::TestRequest::post()
                    .uri("/send")
                    .set_json(msg)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[post("/send")]
    async fn send_msg_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        data: web::Json<api::SendMessage>,
    ) -> Result<HttpResponse, UserError> {
        try_send_message(&app, &req, data.into_inner()).await?;
        Ok(HttpResponse::Ok().finish())
    }
}
