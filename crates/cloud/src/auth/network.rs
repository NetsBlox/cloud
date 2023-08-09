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
}

pub(crate) async fn try_view_client(
    app: &AppData,
    req: &HttpRequest,
    client_id: &api::ClientId,
) -> Result<ViewClient, UserError> {
    let is_auth_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .is_some();

    if is_auth_host {
        Ok(ViewClient {
            id: client_id.to_owned(),
            _private: (),
        })
    } else if is_super_user(app, req).await? {
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
    _msg: &api::SendMessage,
) -> Result<SendMessage, UserError> {
    // TODO: add support for sending for users
    let host = utils::get_authorized_host(&app.authorized_services, req).await?;

    host.map(|_host| SendMessage { _private: () })
        .ok_or(UserError::PermissionsError)
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
