use actix_web::HttpRequest;
use netsblox_cloud_common::api;

use crate::auth;
use crate::utils;

use crate::app_data::AppData;
use crate::errors::UserError;

pub(crate) struct ViewSettings {
    pub(crate) username: String,
    pub(crate) requesting_host: Option<api::ServiceHostId>,
    _private: (),
}

pub(crate) struct ViewGroupSettings {
    pub(crate) id: api::GroupId,
    pub(crate) requesting_host: Option<api::ServiceHostId>,
    _private: (),
}

pub(crate) struct UpdateSettings {
    pub(crate) username: String,
    pub(crate) host: api::ServiceHostId,
    pub(crate) update: api::ServiceHostSettings,
    _private: (),
}

pub(crate) struct UpdateGroupSettings {
    pub(crate) id: api::GroupId,
    pub(crate) host: api::ServiceHostId,
    pub(crate) update: api::ServiceHostSettings,
    _private: (),
}

pub(crate) struct DeleteSettings {
    pub(crate) username: String,
    pub(crate) host: api::ServiceHostId,
    _private: (),
}

pub(crate) struct DeleteGroupSettings {
    pub(crate) id: api::GroupId,
    pub(crate) host: api::ServiceHostId,
    _private: (),
}

pub(crate) async fn try_update_user_settings(
    app: &AppData,
    req: &HttpRequest,
    username: String,
    host: api::ServiceHostId,
    update: api::ServiceHostSettings,
) -> Result<UpdateSettings, UserError> {
    // You can update if you can delete this host's settings
    let ds = auth::try_delete_user_settings(app, req, username, host).await?;
    Ok(UpdateSettings {
        username: ds.username,
        host: ds.host,
        update,
        _private: (),
    })
}

pub(crate) async fn try_view_user_settings(
    app: &AppData,
    req: &HttpRequest,
    username: String,
) -> Result<ViewSettings, UserError> {
    let vu = auth::try_view_user(app, req, None, &username).await?;
    let host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .map(|host| host.id);

    Ok(ViewSettings {
        username: vu.username,
        requesting_host: host,
        _private: (),
    })
}

pub(crate) async fn try_delete_user_settings(
    app: &AppData,
    req: &HttpRequest,
    username: String,
    host: api::ServiceHostId,
) -> Result<DeleteSettings, UserError> {
    let eu = auth::try_edit_user(app, req, None, &username).await?;
    let requesting_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .map(|host| host.id);

    // If a authorized host is deleting a setting, it can only do so
    // for its own settings. Otherwise, someone with edit_user
    // authorization can delete any setting
    if requesting_host.is_some_and(|h| h != host) {
        Err(UserError::PermissionsError)
    } else {
        Ok(DeleteSettings {
            username: eu.username,
            host,
            _private: (),
        })
    }
}

pub(crate) async fn try_view_group_settings(
    app: &AppData,
    req: &HttpRequest,
    group_id: api::GroupId,
) -> Result<ViewGroupSettings, UserError> {
    let vg = auth::try_view_group(app, req, &group_id).await?;
    let host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .map(|host| host.id);

    Ok(ViewGroupSettings {
        id: vg.id,
        requesting_host: host,
        _private: (),
    })
}

pub(crate) async fn try_update_group_settings(
    app: &AppData,
    req: &HttpRequest,
    group_id: api::GroupId,
    host: api::ServiceHostId,
    update: api::ServiceHostSettings,
) -> Result<UpdateGroupSettings, UserError> {
    // You can update if you can delete this host's settings
    let dgs = auth::try_delete_group_settings(app, req, group_id, host).await?;
    Ok(UpdateGroupSettings {
        id: dgs.id,
        host: dgs.host,
        update,
        _private: (),
    })
}

pub(crate) async fn try_delete_group_settings(
    app: &AppData,
    req: &HttpRequest,
    group_id: api::GroupId,
    host: api::ServiceHostId,
) -> Result<DeleteGroupSettings, UserError> {
    let eg = auth::try_edit_group(app, req, &group_id).await?;
    let requesting_host = utils::get_authorized_host(&app.authorized_services, req)
        .await?
        .map(|host| host.id);

    // If a authorized host is deleting a setting, it can only do so
    // for its own settings. Otherwise, someone with edit_group
    // authorization can delete any setting
    if requesting_host.is_some_and(|h| h != host) {
        Err(UserError::PermissionsError)
    } else {
        Ok(DeleteGroupSettings {
            id: eg.id,
            host,
            _private: (),
        })
    }
}
