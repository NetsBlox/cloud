use super::is_super_user;
use crate::app_data::AppData;
use crate::errors::UserError;
use actix_web::HttpRequest;
use netsblox_macros::Witness;

#[derive(Witness)] // FIXME: this doesn't work
pub(crate) struct ViewAuthHosts {
    _private: (),
}

pub(crate) struct AuthorizeHost {
    _private: (),
}

pub(crate) async fn try_view_auth_hosts(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ViewAuthHosts, UserError> {
    if is_super_user(app, req).await? {
        Ok(ViewAuthHosts { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) async fn try_auth_host(
    app: &AppData,
    req: &HttpRequest,
) -> Result<AuthorizeHost, UserError> {
    if is_super_user(app, req).await? {
        Ok(AuthorizeHost { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}
