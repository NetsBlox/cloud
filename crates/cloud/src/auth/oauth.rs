use actix_web::HttpRequest;

use crate::{app_data::AppData, errors::UserError};

use super::ensure_is_auth_host_or_admin;

pub(crate) struct ManageClient {
    _private: (),
}

pub(crate) async fn try_manage_client(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ManageClient, UserError> {
    ensure_is_auth_host_or_admin(app, req)
        .await
        .map(|_| ManageClient { _private: () })
}
