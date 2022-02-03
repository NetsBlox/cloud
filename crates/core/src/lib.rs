#[cfg(feature = "bson")]
mod bson;

use core::fmt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr, time::SystemTime};

#[derive(Deserialize, Serialize)]
pub struct InvitationResponse {
    pub response: FriendLinkState,
}

pub type GroupId = String;
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub group_id: Option<GroupId>,
    pub admin: Option<bool>, // TODO: use roles instead? What other roles would we even have?
    pub created_at: u32,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHost {
    pub url: String,
    pub categories: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LinkedAccount {
    pub username: String,
    pub strategy: String, // TODO: migrate type -> strategy
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum FriendLinkState {
    PENDING,
    APPROVED,
    REJECTED,
    DELETED,
    BLOCKED,
}

#[derive(Debug)]
pub struct ParseFriendLinkStateError;

impl fmt::Display for ParseFriendLinkStateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid friend link state")
    }
}

impl FromStr for FriendLinkState {
    type Err = ParseFriendLinkStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(FriendLinkState::PENDING),
            "approved" => Ok(FriendLinkState::APPROVED),
            "rejected" => Ok(FriendLinkState::REJECTED),
            "deleted" => Ok(FriendLinkState::DELETED),
            "blocked" => Ok(FriendLinkState::BLOCKED),
            _ => Err(ParseFriendLinkStateError),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FriendInvite {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub created_at: SystemTime,
}

pub type ProjectId = String;
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: SystemTime,
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<String, RoleMetadata>,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum SaveState {
    TRANSIENT,
    BROKEN,
    SAVED,
}
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct RoleMetadata {
    pub project_name: String, // TODO: Change this to "name"?
    pub source_code: String,
    pub media: String,
}
