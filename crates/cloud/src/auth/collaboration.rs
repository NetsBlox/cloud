
use actix_web::HttpRequest;
use netsblox_cloud_common::{CollaborationInvite, ProjectMetadata};

use crate::{
    app_data::AppData,
    errors::{InternalError, UserError},
};

use super::try_edit_user;

pub(crate) struct InviteCollaborator {
    pub(crate) project: ProjectMetadata,
    _private: (),
}

pub(crate) async fn try_invite(
    app: &AppData,
    req: &HttpRequest,
    project_id: &ProjectId,
) -> Result<InviteCollaborator, UserError> {
    // Only the owner for now
    let metadata = app.get_project_metadatum(project_id).await?;

    try_edit_user(app, req, None, &metadata.owner).await?;

    Ok(InviteCollaborator {
        project: metadata,
        _private: (),
    })
}

pub(crate) struct RespondToCollabInvite {
    pub(crate) invite: CollaborationInvite,
    _private: (),
}

pub(crate) async fn try_respond_to_invite(
    app: &AppData,
    req: &HttpRequest,
    invite_id: &str,
) -> Result<RespondToCollabInvite, UserError> {
    // Only the owner for now
    let query = doc! {"id": invite_id};
    let invite = app
        .collab_invites
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::InviteNotFoundError)?;

    try_edit_user(app, req, None, &invite.receiver).await?;

    Ok(RespondToCollabInvite {
        invite,
        _private: (),
    })
}
