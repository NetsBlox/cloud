use actix_session::SessionExt;
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::{api, ProjectMetadata};

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};

pub(crate) struct ViewProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_view_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<ViewProject, UserError> {
    // FIXME: if owned by guest account, should everyone be able to see it?
    let session = req.get_session();
    let metadata = app.get_project_metadatum(project_id).await?;

    let can_view = match metadata.state {
        api::PublishState::Private => {
            // Allow viewing if:
            // we can edit the project or...
            let auth_ep = can_edit_project(app, req, client_id, &metadata).await;
            if auth_ep.is_ok() {
                true
            } else {
                // the user has been invited to the project
                if let Some(username) = session.get::<String>("username").unwrap_or(None) {
                    let query = doc! {"username": username};
                    let invite = flatten(app.occupant_invites.find_one(query, None).await.ok());
                    invite.is_some()
                } else {
                    false
                }
            }
        }
        // Allow viewing projects pending approval. Disclaimer should be on client side
        // Client can also disable JS or simply prompt the user if he/she would still like
        // to open the project
        _ => true,
    };

    if can_view {
        Ok(ViewProject {
            metadata,
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct EditProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<EditProject, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;

    let auth_ep = can_edit_project(app, req, client_id.as_ref(), &metadata).await;
    if auth_ep.is_ok() {
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

// TODO: should I define a macro for this to automatically convert project IDs to metadata?
// Or should we define a trait for these authorization objects? try_auth?
pub(crate) async fn try_delete_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<DeleteProject, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;

    let auth_dp = can_delete_project(app, req, client_id.as_ref(), &metadata).await;
    if auth_dp.is_ok() {
        Ok(DeleteProject {
            metadata,
            _private: (),
        })
    } else {
        Err(UserError::PermissionsError)
    }
}

async fn can_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    project: &ProjectMetadata,
) -> Result<EditProject, UserError> {
    let session = req.get_session();
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

fn flatten<T>(nested: Option<Option<T>>) -> Option<T> {
    match nested {
        Some(x) => x,
        None => None,
    }
}
