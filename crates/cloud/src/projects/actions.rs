use crate::app_data::AppData;
use crate::auth;
use crate::errors::{InternalError, UserError};
use mongodb::bson::doc;
use netsblox_cloud_common::{
    api::{self, PublishState},
    ProjectMetadata,
};

pub(crate) async fn get_project(
    app: &AppData,
    md: &auth::ViewProjectMetadata,
) -> Result<api::Project, UserError> {
    app.fetch_project(&md.metadata).await.map(|p| p.into())
}

pub(crate) async fn publish_project(
    app: &AppData,
    edit: &auth::EditProject,
) -> Result<api::PublishState, UserError> {
    let state = if is_approval_required(&app, &edit.metadata).await? {
        PublishState::PendingApproval
    } else {
        PublishState::Public
    };

    let query = doc! {"id": &edit.metadata.id};
    let update = doc! {"$set": {"state": &state}};
    app.project_metadata
        .update_one(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(state)
}

pub(crate) async fn unpublish_project(
    app: &AppData,
    edit: &auth::EditProject,
) -> Result<api::PublishState, UserError> {
    let query = doc! {"id": &edit.metadata.id};
    let state = PublishState::Private;
    let update = doc! {"$set": {"state": &state}};
    app.project_metadata
        .find_one_and_update(query, update, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    Ok(state)
}

async fn is_approval_required(
    app: &AppData,
    metadata: &ProjectMetadata,
) -> Result<bool, UserError> {
    for role_md in metadata.roles.values() {
        let role = app.fetch_role(role_md).await?;
        if libraries::is_approval_required(&role.code) {
            return Ok(true);
        }
    }
    Ok(false)
}
