use std::collections::HashSet;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use actix_session::Session;
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
    // TODO: can we manually extract the session from the request?
    // TODO: then we could simplify the method signature
    session: &Session,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<ViewProject, UserError> {
    // FIXME: update this to use the project actions?
    // that won't work bc I need to bypass the permissions...
    let metadata = app.get_project_metadatum(project_id).await?;

    match metadata.state {
        PublishState::Private => {
            if let Some(username) = session.get::<String>("username").unwrap_or(None) {
                let query = doc! {"username": username};
                let invite = flatten(app.occupant_invites.find_one(query, None).await.ok());
                if invite.is_some() {
                    return Ok(true);
                }
            }

            can_edit_project(app, session, client_id.as_ref(), project).await
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
    session: &Session,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<EditProject, UserError> {
    let view_project = try_view_project(app, session, client_id.clone(), project_id).await?;
    let metadata = view_project.metadata;

    if can_edit_project(app, session, client_id.as_ref(), &metadata).await? {
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
    session: &Session,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<DeleteProject, UserError> {
    // TODO: check that they are the owner or can edit the owner

    let view_project = try_view_project(app, session, client_id.clone(), project_id).await?;
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

pub(crate) struct ViewUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) async fn try_view_user(
    app: &AppData,
    session: &Session,
    client_id: Option<&ClientId>,
    username: &str,
) -> Result<ViewUser, UserError> {
    try_edit_user(app, session, client_id, username)
        .await
        .map(|auth_eu| ViewUser {
            username: auth_eu.username,
            _private: (),
        })
}

pub(crate) struct EditUser {
    pub(crate) username: String,
    _private: (),
}

pub(crate) async fn try_edit_user(
    app: &AppData,
    session: &Session,
    client_id: Option<&ClientId>,
    username: &str,
) -> Result<EditUser, UserError> {
    // TODO: if it is a client ID, make sure it is the same as the client ID
    if let Some(requestor) = session.get::<String>("username").unwrap_or(None) {
        let can_edit = requestor == username
            || is_super_user(app, session).await?
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

pub(crate) struct ViewGroup {
    pub(crate) id: GroupId,
    _private: (),
}

pub(crate) async fn try_view_group(
    app: &AppData,
    session: &Session,
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
    session: &Session,
    group_id: &api::GroupId,
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
    session: &Session,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub async fn is_super_user(app: &AppData, session: &Session) -> Result<bool, UserError> {
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
    session: &Session,
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
                    try_edit_user(app, session, client_id, &project.owner).await?;
                    Ok(true)
                }
            }
            None => Err(UserError::LoginRequiredError),
        }
    }
}
