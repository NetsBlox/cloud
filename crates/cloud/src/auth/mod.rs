use crate::app_data::AppData;
use crate::errors::UserError;
use actix_session::Session;
use netsblox_cloud_common::api::{ClientId, ProjectId};
use netsblox_cloud_common::ProjectMetadata;

// pub(crate) struct ViewAction;
// pub(crate) struct EditAction;

pub(crate) struct ViewProjectMetadata {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_view_project(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientId>,
    project_id: &ProjectId,
) -> Result<ViewProjectMetadata, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;
    Ok(ViewProjectMetadata {
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

    if can_edit_project(app, session, client_id.as_ref(), &metadata).await? {
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
                    can_edit_user(app, session, &project.owner).await
                }
            }
            None => Err(UserError::LoginRequiredError),
        }
    }
}
