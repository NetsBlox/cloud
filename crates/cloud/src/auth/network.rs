use super::is_super_user;
use crate::app_data::AppData;
use crate::auth;
use crate::errors::{InternalError, UserError};
use crate::network::topology;
use actix_session::SessionExt;
use actix_web::HttpRequest;
use netsblox_cloud_common::api::{self, ClientId};
use netsblox_cloud_common::ProjectMetadata;

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

    let auth_eu = auth::try_edit_user(app, req, None, &username).await?;

    Ok(EvictClient {
        project,
        id: client_id.to_owned(),
        _private: (),
    })
}

pub(crate) struct ListActiveRooms {
    _private: (),
}

pub(crate) async fn try_list_rooms(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListActiveRooms, UserError> {
    let session = req.get_session();
    if is_super_user(app, &session).await? {
        Ok(ListActiveRooms { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct ListClients {
    _private: (),
}

pub(crate) async fn try_list_clients(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ListClients, UserError> {
    let session = req.get_session();
    if is_super_user(app, &session).await? {
        Ok(ListClients { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct SendMessage {
    _private: (),
}

pub(crate) async fn try_send_message(
    app: &AppData,
    req: &HttpRequest,
    msg: &api::SendMessage,
) -> Result<SendMessage, UserError> {
    todo!();
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
