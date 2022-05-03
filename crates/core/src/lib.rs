#[cfg(feature = "bson")]
mod bson;

use core::fmt;
use derive_more::{Display, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr, time::SystemTime};
use uuid::Uuid;

const APP_NAME: &str = "NetsBlox";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    pub client_id: String,
    pub username: Option<String>,
    pub services_hosts: Vec<ServiceHost>,
    pub cloud_url: String,
}

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
    pub role: UserRole,
    pub created_at: SystemTime,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: Option<String>,
    pub group_id: Option<String>,
    pub role: Option<UserRole>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum UserRole {
    User,
    Moderator,
    Admin,
}

#[derive(Debug, Display, Error)]
#[display(fmt = "Unable to parse user role. Expected admin, moderator, or user.")]
pub struct UserRoleError;

impl FromStr for UserRole {
    type Err = UserRoleError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(UserRole::Admin),
            "moderator" => Ok(UserRole::Moderator),
            "user" => Ok(UserRole::User),
            _ => Err(UserRoleError),
        }
    }
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
    pub strategy: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub credentials: Credentials,
    pub client_id: Option<String>, // TODO: add a secret token for the client?
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Credentials {
    Snap { username: String, password: String },
    NetsBlox { username: String, password: String },
}

impl From<Credentials> for LinkedAccount {
    fn from(creds: Credentials) -> LinkedAccount {
        match creds {
            Credentials::Snap { username, .. } => LinkedAccount {
                username,
                strategy: "snap".to_owned(),
            },
            Credentials::NetsBlox { username, .. } => LinkedAccount {
                // TODO: should this panic?
                username,
                strategy: "netsblox".to_owned(),
            },
        }
    }
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
#[derive(Deserialize, Serialize, Clone, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum SaveState {
    CREATED,
    TRANSIENT,
    BROKEN,
    SAVED,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RoleMetadata {
    pub name: String,
    pub code: String,
    pub media: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: SystemTime,
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<String, RoleData>,
}

impl Project {
    pub fn to_xml(&self) -> String {
        let role_str: String = self
            .roles
            .clone()
            .into_values()
            // .into_iter()
            .map(|role| role.to_xml())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<room name=\"{}\" app=\"{}\">{}</room>",
            self.name, APP_NAME, role_str
        )
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoleData {
    pub name: String,
    pub code: String,
    pub media: String,
}

impl RoleData {
    pub fn to_xml(self) -> String {
        let name = self.name.replace("\"", "\\\"");
        format!("<role name=\"{}\">{}{}</role>", name, self.code, self.media)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateData {
    pub state: ClientState,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ClientState {
    Browser(BrowserClientState),
    External(ExternalClientState),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserClientState {
    pub role_id: String,
    pub project_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExternalClientState {
    pub address: String,
    pub app_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateLibraryData {
    pub name: String,
    pub notes: String,
    pub blocks: String,
}

#[derive(Serialize, Deserialize)]
pub enum LibraryPublishState {
    Private,
    PendingApproval,
    ApprovalDenied,
    Public,
}

#[derive(Serialize, Deserialize)]
pub struct LibraryMetadata {
    pub owner: String,
    pub name: String,
    pub notes: String,
    pub state: LibraryPublishState,
}

impl LibraryMetadata {
    pub fn new(
        owner: String,
        name: String,
        state: LibraryPublishState,
        notes: Option<String>,
    ) -> LibraryMetadata {
        LibraryMetadata {
            owner,
            name,
            notes: notes.unwrap_or_else(String::new),
            state,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupData {
    pub name: String,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub owner: String,
    pub name: String,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateGroupData {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum InvitationState {
    PENDING,
    ACCEPTED,
    REJECTED,
}

pub type InvitationId = String;
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub project_id: String,
    pub state: InvitationState,
    pub created_at: SystemTime,
}

impl CollaborationInvite {
    pub fn new(sender: String, receiver: String, project_id: String) -> Self {
        CollaborationInvite {
            id: Uuid::new_v4().to_string(),
            sender,
            receiver,
            project_id,
            state: InvitationState::PENDING,
            created_at: SystemTime::now(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectData {
    pub name: String,
    pub client_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleData {
    pub name: String,
    pub client_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectData {
    pub owner: Option<String>,
    pub name: String,
    pub roles: Option<Vec<RoleData>>,
    pub client_id: Option<ClientID>,
    pub save_state: Option<SaveState>,
}

// Network debugging data
pub type ClientID = String;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExternalClient {
    pub username: Option<String>,
    pub address: String,
    pub app_id: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RoomState {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub roles: HashMap<String, RoleState>,
    pub collaborators: Vec<String>,
    pub version: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RoleState {
    pub name: String,
    pub occupants: Vec<OccupantState>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct OccupantState {
    pub id: ClientID,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OccupantInviteData {
    pub username: String,
    pub role_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedServiceHost {
    pub url: String,
    pub id: String,
    pub public: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub username: Option<String>,
    pub state: Option<ClientState>,
}

/// Service settings for a given user categorized by origin
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServiceSettings {
    /// Service settings owned by the user
    pub user: Option<String>,
    /// Service settings owned by a group in which the user is a member
    pub member: Option<String>,
    /// Service settings owned by a groups created by the user
    pub groups: Vec<String>,
}

/// Send message request (for authorized services)
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SendMessage {
    pub target: SendMessageTarget,
    // TODO: Should we only allow "message" types or any sort of message?
    pub content: Value,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum SendMessageTarget {
    Address {
        address: String,
    },
    #[serde(rename_all = "camelCase")]
    Room {
        project_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Role {
        project_id: String,
        role_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Client {
        project_id: String,
        role_id: String,
        client_id: ClientID,
    },
}
