use std::collections::HashSet;

use actix_session::{Session, SessionExt};
use actix_web::HttpRequest;
use futures::TryStreamExt;
use mongodb::bson::doc;
use netsblox_cloud_common::api::{self, ClientId, UserRole};

use crate::{
    app_data::AppData,
    auth,
    errors::{InternalError, UserError},
    utils,
};

#[derive(Debug)]
pub(crate) struct CreateUser {
    pub(crate) data: api::NewUser,
    _private: (),
}

#[derive(Debug)]
pub(crate) struct ViewUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) struct ListUsers {
    _private: (),
}

pub(crate) struct EditUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) struct SetPassword {
    pub(crate) username: String,
    _private: (),
}

pub(crate) struct BanUser {
    pub(crate) username: String,
    _private: (),
}

// TODO: make a macro for making it when testing?
#[cfg(test)]
impl EditUser {
    pub(crate) fn test(username: String) -> Self {
        Self {
            username,
            _private: (),
        }
    }
}

#[cfg(test)]
impl ViewUser {
    pub(crate) fn test(username: String) -> Self {
        Self {
            username,
            _private: (),
        }
    }
}

/// Try to get privileges to create the given user. Must be able
/// to edit the target group (if user is in a group). Moderators
/// or admins can only be created by others with their role (or
/// higher).
pub(crate) async fn try_create_user(
    app: &AppData,
    req: &HttpRequest,
    data: api::NewUser,
) -> Result<CreateUser, UserError> {
    // make sure we can:
    // - edit the target group
    // - make the target user role
    if let Some(group_id) = data.group_id.clone() {
        auth::try_edit_group(app, req, &group_id).await?;
    }

    let new_user_role = data.role.unwrap_or(UserRole::User);
    let is_privileged = !matches!(new_user_role, UserRole::User);

    let is_authorized = if is_privileged {
        // only moderators, admins can make privileged users (up to their role)
        let username = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;
        let req_role = get_user_role(app, &username).await?;
        dbg!(&req_role, &new_user_role);
        req_role >= UserRole::Moderator && req_role >= new_user_role
    } else {
        true
    };

    if is_authorized {
        Ok(CreateUser { data, _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_view_user(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&ClientId>,
    username: &str,
) -> Result<ViewUser, UserError> {
    // can view user if:
    // - self
    // - moderator/admin
    // - group owner
    let is_guest = client_id.map(|id| id.as_str() == username).unwrap_or(false);
    if is_guest {
        return Ok(ViewUser {
            username: username.to_owned(),
            _private: (),
        });
    }

    let viewer = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;

    let authorized = viewer == username
        || utils::get_authorized_host(&app.authorized_services, req)
            .await?
            .is_some()
        || get_user_role(app, &viewer).await? >= UserRole::Moderator
        || has_group_containing(app, &viewer, username).await?;

    if authorized {
        Ok(ViewUser {
            username: username.to_owned(),
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_list_users(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListUsers, UserError> {
    if is_super_user(app, req).await? {
        Ok(ListUsers { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_edit_user(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    username: &str,
) -> Result<EditUser, UserError> {
    if let Some(requestor) = utils::get_username(req) {
        let can_edit = requestor == username
            || get_user_role(app, &requestor).await? >= UserRole::Moderator
            || has_group_containing(app, &requestor, username).await?;

        if can_edit {
            Ok(EditUser {
                username: username.to_owned(),
                _private: (),
            })
        } else {
            Err(UserError::PermissionsError)
        }
    } else {
        // unauthenticated
        client_id
            .and_then(|id| {
                if username == id.as_str() {
                    Some(EditUser {
                        username: username.to_owned(),
                        _private: (),
                    })
                } else {
                    None
                }
            })
            .ok_or(UserError::LoginRequiredError)
    }
}

pub(crate) async fn try_set_password(
    app: &AppData,
    req: &HttpRequest,
    username: &str,
    token: Option<String>,
) -> Result<SetPassword, UserError> {
    let authorized = match token {
        Some(token) => {
            let query = doc! {"secret": token};
            let token = app
                .password_tokens
                .find_one_and_delete(query, None) // If the username is incorrect, the token is compromised (so delete either way)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::PermissionsError)?;

            token.username == username
        }
        None => {
            try_edit_user(&app, &req, None, &username).await?;
            true
        }
    };

    if authorized {
        Ok(SetPassword {
            username: username.to_owned(),
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_ban_user(
    app: &AppData,
    req: &HttpRequest,
    username: &str,
) -> Result<BanUser, UserError> {
    let session = req.get_session();
    if is_moderator(app, &session).await? {
        Ok(BanUser {
            username: username.to_owned(),
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(super) async fn is_super_user(app: &AppData, req: &HttpRequest) -> Result<bool, UserError> {
    let session = req.get_session();
    match get_session_role(app, &session).await? {
        UserRole::Admin => Ok(true),
        _ => Ok(false),
    }
}

async fn get_session_role(app: &AppData, session: &Session) -> Result<UserRole, UserError> {
    if let Some(username) = session.get::<String>("username").unwrap_or(None) {
        get_user_role(app, &username).await
    } else {
        session.purge();
        Err(UserError::LoginRequiredError)
    }
}

async fn get_user_role(app: &AppData, username: &str) -> Result<UserRole, UserError> {
    let query = doc! {"username": username};
    Ok(app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .map(|user| user.role)
        .unwrap_or(UserRole::User))
}

pub(super) async fn is_moderator(app: &AppData, session: &Session) -> Result<bool, UserError> {
    let role = get_session_role(app, session).await?;
    Ok(role >= UserRole::Moderator)
}

async fn has_group_containing(app: &AppData, owner: &str, member: &str) -> Result<bool, UserError> {
    let query = doc! {"username": member};
    match app
        .users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
    {
        Some(user) => match user.group_id {
            Some(group_id) => {
                let query = doc! {"owner": owner};
                let cursor = app
                    .groups
                    .find(query, None)
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;
                let groups = cursor
                    .try_collect::<Vec<_>>()
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;
                let group_ids = groups
                    .into_iter()
                    .map(|group| group.id)
                    .collect::<HashSet<_>>();
                Ok(group_ids.contains(&group_id))
            }
            None => Ok(false),
        },
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, http, test, web, App, HttpResponse};
    use netsblox_cloud_common::{api, Group, User};

    use crate::test_utils;

    #[actix_web::test]
    async fn test_try_create_user() {
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        };
        test_utils::setup()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_moderator() {
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Moderator),
        };
        test_utils::setup()
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_moderator_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Moderator),
        };
        test_utils::setup()
            .with_users(&[admin.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/test")
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_member_other_user() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        };
        test_utils::setup()
            .with_users(&[owner, other.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .cookie(test_utils::cookie::new(&other.username))
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_member_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        };
        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .cookie(test_utils::cookie::new(&owner.username))
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_member_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        dbg!(&group);
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        };
        test_utils::setup()
            .with_users(&[owner, admin.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .cookie(test_utils::cookie::new(&admin.username))
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_create_member_moderator() {
        let moderator: User = api::NewUser {
            username: "moderator".into(),
            email: "moderator@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Moderator),
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        dbg!(&group);
        let user_data = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        };
        test_utils::setup()
            .with_users(&[owner, moderator.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(create_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri("/test")
                    .cookie(test_utils::cookie::new(&moderator.username))
                    .set_json(user_data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_user_self() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[ignore]
    #[actix_web::test]
    async fn test_try_view_user_auth_host() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_view_user_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let viewer: User = api::NewUser {
            username: "viewer".into(),
            email: "viewer@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        test_utils::setup()
            .with_users(&[user, viewer.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&viewer.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_user_group_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let group = Group::new(owner.username.clone(), "some_group".into());
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user, owner.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(view_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_self() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let viewer: User = api::NewUser {
            username: "viewer".into(),
            email: "viewer@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        test_utils::setup()
            .with_users(&[user, viewer.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&viewer.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_moderator() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let viewer: User = api::NewUser {
            username: "viewer".into(),
            email: "viewer@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Moderator),
        }
        .into();
        test_utils::setup()
            .with_users(&[user, viewer.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&viewer.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_peer() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let viewer: User = api::NewUser {
            username: "viewer".into(),
            email: "viewer@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user, viewer.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&viewer.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_other_owner() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let viewer: User = api::NewUser {
            username: "viewer".into(),
            email: "viewer@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(viewer.username.clone(), "some_group".into());
        test_utils::setup()
            .with_users(&[user, viewer.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&viewer.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_user_group_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let group = Group::new(owner.username.clone(), "some_group".into());
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: Some(group.id.clone()),
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user, owner.clone()])
            .with_groups(&[group])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_ban_user_self() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(ban_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_ban_user_other_user() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(ban_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_ban_user_moderator() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let mod_user: User = api::NewUser {
            username: "mod".into(),
            email: "mod@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Moderator),
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), mod_user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(ban_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&mod_user.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_ban_user_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let mod_user: User = api::NewUser {
            username: "mod".into(),
            email: "mod@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone(), mod_user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(ban_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&mod_user.username))
                    .uri("/test")
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    // helper endpoints to check permissions
    #[get("/test")]
    async fn create_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        data: web::Json<api::NewUser>,
    ) -> Result<HttpResponse, UserError> {
        try_create_user(&app, &req, data.into_inner()).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn view_test(
        app: web::Data<AppData>,
        req: HttpRequest,
    ) -> Result<HttpResponse, UserError> {
        try_view_user(&app, &req, None, "user").await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn edit_test(
        app: web::Data<AppData>,
        req: HttpRequest,
    ) -> Result<HttpResponse, UserError> {
        try_edit_user(&app, &req, None, "user").await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn ban_test(
        app: web::Data<AppData>,
        req: HttpRequest,
    ) -> Result<HttpResponse, UserError> {
        try_ban_user(&app, &req, "user").await?;
        Ok(HttpResponse::Ok().finish())
    }
}
