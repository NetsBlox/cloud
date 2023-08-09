use actix_session::SessionExt;
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::{api, ProjectMetadata};

use crate::app_data::AppData;
use crate::errors::UserError;
use crate::utils;

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
    let metadata = app.get_project_metadatum(project_id).await?;
    let is_auth_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .is_some();

    println!("is auth host? {}", is_auth_host);
    if is_auth_host {
        return Ok(ViewProject {
            metadata,
            _private: (),
        });
    }

    let session = req.get_session();

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
            metadata,
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
        println!("not the owner");
        let username = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;
        println!("username: {}", &username);
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

fn flatten<T>(nested: Option<Option<T>>) -> Option<T> {
    match nested {
        Some(x) => x,
        None => None,
    }
}
