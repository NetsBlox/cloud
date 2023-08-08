pub(crate) mod collaboration;
pub(crate) mod groups;
pub(crate) mod hosts;
pub(crate) mod libraries;
pub(crate) mod network;
pub(crate) mod oauth;
pub(crate) mod projects;
pub(crate) mod users;

pub(crate) use crate::auth::collaboration::*;
pub(crate) use crate::auth::groups::*;
pub(crate) use crate::auth::hosts::*;
pub(crate) use crate::auth::libraries::*;
pub(crate) use crate::auth::network::*;
pub(crate) use crate::auth::oauth::*;
pub(crate) use crate::auth::projects::*;
pub(crate) use crate::auth::users::*;

use crate::app_data::AppData;
use crate::errors::UserError;
use actix_web::HttpRequest;

/// Invite link is an authorized directed link btwn users to be
/// used to send invitations like occupant, collaboration invites
pub(crate) struct InviteLink {
    pub(crate) source: String,
    pub(crate) target: String,
    _private: (),
}

pub(crate) async fn try_invite_link(
    app: &AppData,
    req: &HttpRequest,
    source: &str,
    target: &str,
) -> Result<InviteLink, UserError> {
    try_edit_user(app, req, None, source)
        .await
        .map(|_eu| InviteLink {
            source: source.to_owned(),
            target: target.to_owned(),
            _private: (),
        })
    // TODO: ensure source -> target are friends
}
