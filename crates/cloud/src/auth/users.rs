use std::collections::HashSet;

use actix_session::SessionExt;
use actix_web::HttpRequest;
use futures::TryStreamExt;
use mongodb::bson::doc;
use netsblox_cloud_common::api::{self, ClientId, UserRole};

use crate::{
    app_data::AppData,
    auth,
    errors::{InternalError, UserError},
};

pub(crate) struct CreateUser {
    pub(crate) data: api::NewUser,
    _private: (),
}

pub(crate) async fn try_create_user(
    app: &AppData,
    req: &HttpRequest,
    data: api::NewUser,
) -> Result<CreateUser, UserError> {
    // TODO: make sure we can:
    // - edit the target group
    // - make the target user role
    let session = req.get_session();
    let eg = auth::try_edit_group(app, req, data.group_id.as_ref()).await?;
    // TODO: check the user role
    // let role = data.role.as_ref().unwrap_or(&UserRole::User);
    // match role {
    //     UserRole::User => {
    //         if let Some(group_id) = &data.group_id {
    //             auth::try_edit_group(&app, &session, group_id).await?;
    //         }
    //     }
    //     _ => ensure_is_super_user(&app, &session).await?,
    // };
    todo!()
}

pub(crate) struct ViewUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) async fn try_view_user(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&ClientId>,
    username: &str,
) -> Result<ViewUser, UserError> {
    // TODO: ensure authorized hosts can do this
    try_edit_user(app, req, client_id, username)
        .await
        .map(|auth_eu| ViewUser {
            username: auth_eu.username,
            _private: (),
        })
}

pub(crate) struct ListUsers {
    _private: (),
}

pub(crate) async fn try_list_users(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListUsers, UserError> {
    let session = req.get_session();
    if is_super_user(&app, &session).await? {
        Ok(ListUsers { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct EditUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) async fn try_edit_user(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    username: &str,
) -> Result<EditUser, UserError> {
    // TODO: if it is a client ID, make sure it is the same as the client ID
    let session = req.get_session();

    if let Some(requestor) = session.get::<String>("username").unwrap_or(None) {
        let can_edit = requestor == username
            || is_super_user(app, &session).await?
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

pub(crate) struct SetPassword {
    pub(crate) username: String,
    _private: (),
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

pub(crate) struct BanUser {
    pub(crate) username: String,
    _private: (),
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

async fn is_super_user(app: &AppData, session: &Session) -> Result<bool, UserError> {
    match get_session_role(app, session).await? {
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
    use actix_web::{test, HttpRequest};
    use netsblox_cloud_common::{api, User};

    use crate::test_utils;

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
                todo!()
                // TODO: how to make HttpRequest for testing?
                // let req: HttpRequest = test::TestRequest::get()
                //     .cookie(test_utils::cookie::new(&user.username))
                //     .to_request()
                //     .into();

                // let auth = try_view_user(&app_data, &req, None, &user.username).await;

                // assert!(auth.is_ok());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_view_user_auth_host() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_view_user_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_view_user_group_owner() {
        todo!();
    }
    #[actix_web::test]
    async fn test_try_edit_user_self() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_edit_user_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_edit_user_moderator() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_edit_user_peer() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_edit_user_other_owner() {
        todo!();
    }

    #[actix_web::test]
    async fn test_try_edit_user_group_owner() {
        todo!();
    }
}
