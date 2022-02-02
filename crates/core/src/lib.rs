#[cfg(feature = "bson")]
mod bson;

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct InvitationResponse {
    pub response: FriendLinkState,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FriendLinkState {
    PENDING,
    APPROVED,
    REJECTED,
    DELETED,
    BLOCKED,
}
