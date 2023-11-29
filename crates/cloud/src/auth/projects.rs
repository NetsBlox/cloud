use actix_session::SessionExt;
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::{api, ProjectMetadata};

use crate::app_data::AppData;
use crate::errors::UserError;
use crate::utils;

use super::{is_moderator, ManageSystem};

/// Permissions to view a specific project
pub(crate) struct ViewProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

/// Permissions to list projects for a given owner or with a given collaborator
pub(crate) struct ListProjects {
    pub(crate) username: String,
    pub(crate) visibility: api::PublishState,
    _private: (),
}

pub(crate) struct EditProject {
    pub(crate) metadata: ProjectMetadata,
    _private: (),
}

pub(crate) struct DeleteProject {
    pub(crate) id: api::ProjectId,
    _private: (),
}

impl DeleteProject {
    /// Get project deletion permissions from system management permissions.
    pub(crate) fn from_manage_system(_witness: &ManageSystem, id: api::ProjectId) -> Self {
        Self { id, _private: () }
    }
}

/// Permissions to approve projects that require manual approval
pub(crate) struct ModerateProjects {
    _private: (),
}

pub(crate) async fn try_view_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<ViewProject, UserError> {
    // FIXME: if owned by guest account, should everyone be able to see it?
    let metadata = app.get_project_metadatum(project_id).await?;
    let is_auth_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .is_some();

    if is_auth_host {
        return Ok(ViewProject {
            metadata,
            _private: (),
        });
    }

    let can_view = match metadata.state {
        api::PublishState::Private => {
            // Allow viewing if:
            // we can edit the project or...
            let auth_ep = can_edit_project(app, req, client_id, &metadata).await;
            if auth_ep.is_ok() {
                true
            } else {
                // the user has been invited to the project
                if let Some(username) = utils::get_username(req) {
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

    let is_logged_in = utils::get_username(req).is_some();
    if can_view {
        Ok(ViewProject {
            metadata,
            _private: (),
        })
    } else if is_logged_in {
        Err(UserError::PermissionsError)
    } else {
        Err(UserError::LoginRequiredError)
    }
}

pub(crate) async fn try_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<EditProject, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;

    can_edit_project(app, req, client_id.as_ref(), &metadata).await
}

pub(crate) async fn try_delete_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<api::ClientId>,
    project_id: &api::ProjectId,
) -> Result<DeleteProject, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;

    // Only the owner can delete projects
    super::try_edit_user(app, req, client_id.as_ref(), &metadata.owner)
        .await
        .map(|_eu| DeleteProject {
            id: metadata.id,
            _private: (),
        })
}

pub(crate) async fn try_list_projects(
    app: &AppData,
    req: &HttpRequest,
    username: &str,
) -> Result<ListProjects, UserError> {
    let auth_eu = super::try_edit_user(app, req, None, username).await;
    let visibility = if let Ok(_auth_eu) = auth_eu {
        api::PublishState::Private
    } else {
        api::PublishState::PendingApproval
    };

    Ok(ListProjects {
        username: username.to_owned(),
        visibility,
        _private: (),
    })
}

pub(crate) async fn can_edit_project(
    app: &AppData,
    req: &HttpRequest,
    client_id: Option<&api::ClientId>,
    project: &ProjectMetadata,
) -> Result<EditProject, UserError> {
    let is_owner = client_id
        .map(|id| id.as_str() == project.owner)
        .unwrap_or(false);

    if !is_owner {
        let username = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;
        if !project.collaborators.contains(&username) {
            // if we are not a collaborator, then we must be able to edit the owner
            super::try_edit_user(app, req, client_id, &project.owner).await?;
        }
    }

    Ok(EditProject {
        metadata: project.to_owned(),
        _private: (),
    })
}

pub(crate) async fn try_moderate_projects(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ModerateProjects, UserError> {
    let session = req.get_session();
    if is_moderator(app, &session).await? {
        Ok(ModerateProjects { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

fn flatten<T>(nested: Option<Option<T>>) -> Option<T> {
    match nested {
        Some(x) => x,
        None => None,
    }
}

#[cfg(test)]
mod test_utils {
    use super::*;

    impl ViewProject {
        pub(crate) fn test(metadata: ProjectMetadata) -> Self {
            Self {
                metadata,
                _private: (),
            }
        }
    }

    impl DeleteProject {
        pub(crate) fn test(metadata: ProjectMetadata) -> Self {
            Self {
                id: metadata.id,
                _private: (),
            }
        }
    }

    impl EditProject {
        pub(crate) fn test(metadata: ProjectMetadata) -> Self {
            Self {
                metadata,
                _private: (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[actix_web::test]
    #[ignore]
    async fn test_try_view_project_owner() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_view_project_invited() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_view_project_group_owner() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_view_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_try_view_project_403() {
        todo!();
    }
}
