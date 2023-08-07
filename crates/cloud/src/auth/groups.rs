use crate::app_data::AppData;
use actix_web::HttpRequest;
use netsblox_cloud_common::api;

use crate::errors::UserError;

pub(crate) struct ViewGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_view_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<ViewGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct EditGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_edit_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: Option<&api::GroupId>,
) -> Result<EditGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}

pub(crate) struct DeleteGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) async fn try_delete_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    // TODO: allow authorized host
    // TODO: check if the current user is the owner of the group
    todo!()
}
