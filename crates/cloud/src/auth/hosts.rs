use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use actix_session::SessionExt;
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::AuthorizedServiceHost;

pub(crate) struct ViewAuthHosts {
    _private: (),
}

pub(crate) async fn try_view_auth_hosts(
    app: &AppData,
    req: &HttpRequest,
) -> Result<ViewAuthHosts, UserError> {
    let session = req.get_session();
    if is_super_user(&app, &session).await? {
        Ok(ViewAuthHosts { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

pub(crate) struct AuthorizeHost {
    _private: (),
}

pub(crate) async fn try_auth_host(
    app: &AppData,
    req: &HttpRequest,
) -> Result<AuthorizeHost, UserError> {
    let session = req.get_session();
    if is_super_user(&app, &session).await? {
        Ok(AuthorizeHost { _private: () })
    } else {
        Err(UserError::PermissionsError)
    }
}

async fn ensure_is_authorized_host(
    app: &AppData,
    req: &HttpRequest,
    host_id: Option<&str>,
) -> Result<AuthorizedServiceHost, UserError> {
    let query = req
        .headers()
        .get("X-Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value_str| {
            let mut chunks = value_str.split(':');
            let id = chunks.next();
            let secret = chunks.next();
            id.and_then(|id| secret.map(|s| (id, s)))
        })
        .map(|(id, secret)| doc! {"id": id, "secret": secret})
        .ok_or(UserError::PermissionsError)?; // permissions error since there are no credentials

    let host = app
        .authorized_services
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::PermissionsError)?;

    if let Some(host_id) = host_id {
        if host_id != host.id {
            return Err(UserError::PermissionsError);
        }
    }
    Ok(host)
}
