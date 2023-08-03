use std::collections::HashSet;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::network::topology;
use actix_session::{Session, SessionExt};
use actix_web::HttpRequest;
use futures::TryStreamExt;
use mongodb::bson::doc;
use netsblox_cloud_common::api::{self, ClientId, GroupId, ProjectId, UserRole};
use netsblox_cloud_common::{ProjectMetadata, User};

// pub(crate) struct ViewAction;
// pub(crate) struct EditAction;

pub(crate) struct ViewProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_view_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<ViewProject, UserError> {
    // FIXME: if owned by guest account, should everyone be able to see it?

    // FIXME: update this to use the project actions?
    // that won't work bc I need to bypass the permissions...
    let session = req.get_session();
    let metadata = app.get_project_metadatum(project_id).await?;

    match metadata.state {
        PublishState::Private => {
            if let Some(username) = session.get::<String>("username").unwrap_or(None) {
                let query = doc! {"username": username};
                let invite = flatten(app.occupant_invites.find_one(query, None).await.ok());
                if invite.is_some() {
                    return Ok(ViewProject {
                        metadata,
                        _private: (),
                    });
                }
            }

            can_edit_project(app, req, client_id.as_ref(), project).await
        }
        _ => Ok(true),
    }
    // Allow viewing projects pending approval. Disclaimer should be on client side
    // Client can also disable JS or simply prompt the user if he/she would still like
    // to open the project
    // FIXME:
    Ok(ViewProject {
        metadata,
        _private: (),
    })
}

pub(crate) struct EditProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<EditProject, UserError> {
    let session = req.get_session();
    let view_project = try_view_project(app, req, client_id.clone(), project_id).await?;
    let metadata = view_project.metadata;

    if can_edit_project(app, req, client_id.as_ref(), &metadata).await? {
        Ok(EditProject {
            metadata,
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct DeleteProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_delete_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<DeleteProject, UserError> {
    // TODO: check that they are the owner or can edit the owner

    let view_project = try_view_project(app, req, client_id.clone(), project_id).await?;
    let metadata = view_project.metadata;

    // TODO: check that they can edit the owner
    todo!();
    // try_edit_user(app, session, client_id.as_ref(), &metadata).await? {
    //     Ok(DeleteProject {
    //         metadata,
    //         _private: (),
    //     })
    // } else {
    //     Err(UserError::PermissionsError)
    // }
}

pub(crate) struct CreateUser {
    pub(crate) data: api::NewUser,
    _private: (),
}

pub(crate) async fn try_create_user(
    app: &AppData,
    req: &HttpRequest,
    data: api::NewUser,
) -> Result<CreateUser, UserError> {
    // TODO:
    let session = req.get_session();
    let eg = try_edit_group(app, req, data.group_id.as_ref()).await?;
    // TODO: check the user role
    let role = data.role.as_ref().unwrap_or(&UserRole::User);
    match role {
        UserRole::User => {
            if let Some(group_id) = &data.group_id {
                ensure_can_edit_group(&app, &session, group_id).await?;
            }
        }
        _ => ensure_is_super_user(&app, &session).await?,
    };
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
    client_id: Option<&ClientId>,
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

pub(crate) struct ViewGroup {
    pub(crate) id: GroupId,
    _private: (),
}

pub(crate) async fn try_view_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<ViewGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct EditGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_edit_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: Option<&api::GroupId>,
) -> Result<EditGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct DeleteGroup {
    pub(crate) id: GroupId,
    _private: (),
}

pub(crate) async fn try_delete_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct ViewClient {
    pub(crate) id: ClientId,
    _private: (),
}

pub(crate) async fn try_view_client(
    app: &AppData,
    req: &HttpRequest,
    client_id: &api::ClientId,
) -> Result<ViewClient, UserError> {
    // TODO: allow authorized host
    // TODO: check if super user
    todo!()
}

pub(crate) struct EvictClient {
    pub(crate) project: Option<ProjectMetadata>,
    pub(crate) id: ClientId,
    _private: (),
}

pub(crate) async fn try_evict_client(
    app: &AppData,
    req: &HttpRequest,
    client_id: &api::ClientId,
) -> Result<EvictClient, UserError> {
    let project = get_project_for_client(app, client_id).await?;

    // client can be evicted by anyone who can edit the browser project
    if let Some(metadata) = project.clone() {
        let session = req.get_session();
        if can_edit_project(app, req, Some(client_id), &metadata).await? {
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

    let auth_eu = try_edit_user(app, req, None, &username).await?;

    Ok(EvictClient {
        project,
        id: client_id.to_owned(),
        _private: (),
    })
}

/// Invite link is an authorized directed link btwn users to be
/// used to send invitations like occupant, collaboration invites
pub(crate) struct InviteLink {
    pub(crate) source: String,
    pub(crate) target: String,
    _private: (),
}

pub(crate) async fn try_invite_link(
    app: &AppData,
    req: &HttpRequest,
    source: &String,
    target: &String,
) -> Result<InviteLink, UserError> {
    // TODO: ensure we can edit the source
    // TODO: source -> target are friends
    todo!()
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

async fn can_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&ClientId>,
    project: &ProjectMetadata,
) -> Result<bool, UserError> {
    let is_owner = client_id
        .map(|id| id.as_str() == project.owner)
        .unwrap_or(false);

    if is_owner {
        Ok(true)
    } else {
        match session.get::<String>("username").unwrap_or(None) {
            Some(username) => {
                if project.collaborators.contains(&username) {
                    Ok(true)
                } else {
                    try_edit_user(app, req, client_id, &project.owner).await?;
                    Ok(true)
                }
            }
            None => Err(UserError::LoginRequiredError),
        }
    }
}

async fn is_moderator(app: &AppData, session: &Session) -> Result<bool, UserError> {
    let role = get_session_role(app, session).await?;
    Ok(role >= UserRole::Moderator)
}

fn flatten<T>(nested: Option<Option<T>>) -> Option<T> {
    match nested {
        Some(x) => x,
        None => None,
    }
}
