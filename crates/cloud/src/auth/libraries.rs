use super::users::is_moderator;
use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use actix_session::SessionExt;
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::api::PublishState;
use netsblox_cloud_common::Library;

pub(crate) struct ListLibraries {
    pub(crate) username: String,
    pub(crate) visibility: PublishState,
    _private: (),
}

pub(crate) async fn try_list_libraries(
    app: &AppData,
    req: &HttpRequest,
    username: &str,
) -> Result<ListLibraries, UserError> {
    let visibility = if super::is_super_user(app, req).await? {
        PublishState::Private
    } else {
        PublishState::Public
    };

    Ok(ListLibraries {
        username: username.to_owned(),
        visibility,
        _private: (),
    })
}

pub(crate) struct ViewLibrary {
    pub(crate) library: Library,
    _private: (),
}

pub(crate) async fn try_view_library(
    app: &AppData,
    req: &HttpRequest,
    owner: &str,
    name: &str,
) -> Result<ViewLibrary, UserError> {
    let session = req.get_session();

    // Check that the library is public or the user is editable by the current sess
    let query = doc! {"owner": owner, "name": name};
    let library = app
        .libraries
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::LibraryNotFoundError)?;

    todo!();
    // if !matches!(library.state, PublishState::Public) {
    //     // check that we can edit the user
    //     try_edit_user(app, req, None, owner).await?;
    // }

    // Ok(ViewLibrary {
    //     library,
    //     _private: (),
    // })
}

pub(crate) struct EditLibrary {
    pub(crate) owner: String,
    _private: (),
}

pub(crate) async fn try_edit_library(
    app: &AppData,
    req: &HttpRequest,
    owner: &str,
) -> Result<EditLibrary, UserError> {
    todo!()
    // try_edit_user(app, req, None, owner).await?;

    // Ok(EditLibrary {
    //     owner: owner.to_owned(),
    //     _private: (),
    // })
}

pub(crate) struct PublishLibrary {
    pub(crate) owner: String,
    pub(crate) can_approve: bool,
    _private: (),
}

pub(crate) async fn try_publish_library(
    app: &AppData,
    req: &HttpRequest,
    owner: &str,
) -> Result<PublishLibrary, UserError> {
    todo!();
    // let session = req.get_session();
    // if is_moderator(app, &session).await? {
    //     Ok(PublishLibrary {
    //         owner: owner.to_owned(),
    //         can_approve: true,
    //         _private: (),
    //     })
    // } else {
    //     try_edit_user(app, req, None, owner).await?;

    //     Ok(PublishLibrary {
    //         owner: owner.to_owned(),
    //         can_approve: false,
    //         _private: (),
    //     })
    // }
}

pub(crate) struct ModerateLibraries {
    _private: (),
}

pub(crate) async fn try_moderate_libraries(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ModerateLibraries, UserError> {
    let session = req.get_session();
    if is_moderator(app, &session).await? {
        Ok(ModerateLibraries { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}
