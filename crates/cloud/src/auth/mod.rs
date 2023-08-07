pub(crate) mod collaboration;
pub(crate) mod groups;
pub(crate) mod hosts;
pub(crate) mod libraries;
pub(crate) mod network;
pub(crate) mod projects;
pub(crate) mod users;

pub(crate) use crate::auth::collaboration::*;
pub(crate) use crate::auth::groups::*;
pub(crate) use crate::auth::hosts::*;
pub(crate) use crate::auth::libraries::*;
pub(crate) use crate::auth::network::*;
pub(crate) use crate::auth::projects::*;
pub(crate) use crate::auth::users::*;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::network::topology;
use actix_session::{Session, SessionExt};
use actix_web::HttpRequest;
use futures::TryStreamExt;
use mongodb::bson::doc;
use netsblox_cloud_common::api::{self, ClientId, UserRole};
use netsblox_cloud_common::ProjectMetadata;
use std::collections::HashSet;

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
    source: &String,
    target: &String,
) -> Result<InviteLink, UserError> {
    // TODO: ensure we can edit the source
    // TODO: source -> target are friends
    todo!()
}
