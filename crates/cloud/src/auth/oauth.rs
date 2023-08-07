use actix_web::HttpRequest;

use crate::{app_data::AppData, errors::UserError};

use super::is_super_user;

pub(crate) struct ManageClient {
    _private: (),
}

pub(crate) async fn try_manage_client(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ManageClient, UserError> {
    if is_super_user(app, req).await? {
        Ok(ManageClient { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}
