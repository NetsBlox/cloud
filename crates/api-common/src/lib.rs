#[cfg(feature = "bson")]
mod bson;
pub mod oauth;

use core::fmt;
use derive_more::{Display, Error, FromStr};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::Value;
use std::{cmp::Ordering, collections::HashMap, str::FromStr, time::SystemTime};
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

#[derive(Serialize, Deserialize, Debug)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    pub password: Option<String>,
    pub group_id: Option<GroupId>,
    pub role: Option<UserRole>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UserRole {
    User = 0,
    Teacher = 1,
    Moderator = 2,
    Admin = 3,
}

impl PartialOrd for UserRole {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let my_val = *self as u32;
        let other_val = *other as u32;
        my_val.partial_cmp(&other_val)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkTraceMetadata {
    pub id: String,
    pub start_time: SystemTime,
    pub end_time: Option<SystemTime>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub project_id: ProjectId,
    pub recipients: Vec<ClientState>,
    pub time: SystemTime,
    pub source: ClientState,

    pub content: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OccupantInvite {
    pub username: String,
    pub project_id: ProjectId,
    pub role_id: RoleId,
    pub created_at: SystemTime,
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
            "teacher" => Ok(UserRole::Teacher),
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BannedAccount {
    pub username: String,
    pub email: String,
    pub banned_at: SystemTime,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub credentials: Credentials,
    pub client_id: Option<ClientId>, // TODO: add a secret token for the client?
}

#[derive(Deserialize, Serialize, Debug, Clone)]
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

pub type FriendLinkId = String; // FIXME: switch to newtype
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FriendLink {
    pub id: FriendLinkId,
    pub sender: String,
    pub recipient: String,
    pub state: FriendLinkState,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum FriendLinkState {
    Pending,
    Approved,
    Rejected,
    Blocked,
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
            "pending" => Ok(FriendLinkState::Pending),
            "approved" => Ok(FriendLinkState::Approved),
            "rejected" => Ok(FriendLinkState::Rejected),
            "blocked" => Ok(FriendLinkState::Blocked),
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(id: String) -> Self {
        ProjectId(id)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash)]
pub struct RoleId(String);

impl RoleId {
    pub fn new(id: String) -> Self {
        RoleId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: SystemTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub network_traces: Vec<NetworkTraceMetadata>,
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<RoleId, RoleMetadata>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum SaveState {
    Created,
    Transient,
    Broken,
    Saved,
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
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<RoleId, RoleData>,
}

impl Project {
    pub fn to_xml(&self) -> String {
        let role_str: String = self
            .roles
            .values()
            .map(|role| role.to_xml())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<room name=\"{}\" app=\"{}\">{}</room>",
            self.name, APP_NAME, role_str
        )
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDataResponse {
    pub id: Uuid,
    pub data: RoleData,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoleData {
    pub name: String,
    pub code: String,
    pub media: String,
}

impl RoleData {
    pub fn to_xml(&self) -> String {
        let name = self.name.replace('\"', "\\\"");
        format!("<role name=\"{}\">{}{}</role>", name, self.code, self.media)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateData {
    pub state: ClientState,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClientState {
    Browser(BrowserClientState),
    External(ExternalClientState),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserClientState {
    pub role_id: RoleId,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Clone, Hash, Eq, PartialEq)]
pub struct AppId(String);

impl AppId {
    pub fn new(addr: &str) -> Self {
        Self(addr.to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D>(deserializer: D) -> Result<AppId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Value::String(s) = value {
            Ok(AppId::new(s.as_str()))
        } else {
            Err(de::Error::custom("Invalid App ID expected a string"))
        }
    }
}

struct AppIdVisitor;
impl<'de> Visitor<'de> for AppIdVisitor {
    type Value = AppId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an App ID string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        println!("deserializing {}", value);
        Ok(AppId::new(value))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        println!("deserializing {}", value);
        Ok(AppId::new(value.as_str()))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalClientState {
    pub address: String,
    pub app_id: AppId,
}

#[derive(Serialize, Deserialize)]
pub struct CreateLibraryData {
    pub name: String,
    pub notes: String,
    pub blocks: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PublishState {
    Private,
    ApprovalDenied,
    PendingApproval,
    Public,
}

impl PartialOrd for PublishState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.eq(other) {
            Some(Ordering::Equal)
        } else if matches!(self, PublishState::Private) {
            Some(Ordering::Less)
        } else if matches!(other, PublishState::Private) {
            Some(Ordering::Greater)
        } else if matches!(self, PublishState::ApprovalDenied) {
            Some(Ordering::Less)
        } else if matches!(other, PublishState::ApprovalDenied) {
            Some(Ordering::Greater)
        } else if matches!(self, PublishState::PendingApproval) {
            Some(Ordering::Less)
        } else {
            // other must be PendingApproval and we are Public
            Some(Ordering::Greater)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LibraryMetadata {
    pub owner: String,
    pub name: String,
    pub notes: String,
    pub state: PublishState,
}

impl LibraryMetadata {
    pub fn new(
        owner: String,
        name: String,
        state: PublishState,
        notes: Option<String>,
    ) -> LibraryMetadata {
        LibraryMetadata {
            owner,
            name,
            notes: notes.unwrap_or_default(),
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, FromStr)]
pub struct GroupId(String);

impl GroupId {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
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
    Pending,
    Accepted,
    Rejected,
}

pub type InvitationId = String;
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub project_id: ProjectId,
    pub state: InvitationState,
    pub created_at: SystemTime,
}

impl CollaborationInvite {
    pub fn new(sender: String, receiver: String, project_id: ProjectId) -> Self {
        CollaborationInvite {
            id: Uuid::new_v4().to_string(),
            sender,
            receiver,
            project_id,
            state: InvitationState::Pending,
            created_at: SystemTime::now(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectData {
    pub name: String,
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleData {
    pub name: String,
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectData {
    pub owner: Option<String>,
    pub name: String,
    pub roles: Option<Vec<RoleData>>,
    pub client_id: Option<ClientId>,
    pub save_state: Option<SaveState>,

    #[cfg(test)]
    pub role_dict: Option<HashMap<RoleId, RoleData>>,
}

// Network debugging data
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientId(String);

impl ClientId {
    pub fn new(addr: String) -> Self {
        Self(addr)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Display, Error)]
#[display(fmt = "Invalid client ID. Must start with a _")]
pub struct ClientIDError;

impl FromStr for ClientId {
    type Err = ClientIDError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('_') {
            Ok(ClientId::new(s.to_owned()))
        } else {
            Err(ClientIDError)
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExternalClient {
    pub username: Option<String>,
    pub address: String,
    pub app_id: AppId,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RoomState {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub roles: HashMap<RoleId, RoleState>,
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
    pub id: ClientId,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OccupantInviteData {
    pub username: String,
    pub role_id: RoleId,
    pub sender: Option<String>,
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
    pub groups: HashMap<GroupId, String>,
}

/// Send message request (for authorized services)
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SendMessage {
    pub sender: Option<SendMessageSender>,
    pub target: SendMessageTarget,
    // TODO: Should we only allow "message" types or any sort of message?
    pub content: Value,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SendMessageSender {
    Username(String),
    Client(ClientId),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SendMessageTarget {
    Address {
        address: String,
    },
    #[serde(rename_all = "camelCase")]
    Room {
        project_id: ProjectId,
    },
    #[serde(rename_all = "camelCase")]
    Role {
        project_id: ProjectId,
        role_id: RoleId,
    },
    #[serde(rename_all = "camelCase")]
    Client {
        state: Option<ClientState>,
        client_id: ClientId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn deserialize_project_id() {
        let project_id_str = &format!("\"{}\"", Uuid::new_v4());
        let _project_id: ProjectId = serde_json::from_str(project_id_str)
            .unwrap_or_else(|_err| panic!("Unable to parse ProjectId from {}", project_id_str));
    }

    #[test]
    fn deserialize_role_id() {
        let role_id_str = &format!("\"{}\"", Uuid::new_v4());
        let _role_id: RoleId = serde_json::from_str(role_id_str)
            .unwrap_or_else(|_err| panic!("Unable to parse RoleId from {}", role_id_str));
    }

    #[test]
    fn should_compare_roles() {
        assert!(UserRole::Teacher > UserRole::User);
        assert!(UserRole::Moderator > UserRole::User);
        assert!(UserRole::Admin > UserRole::User);

        assert!(UserRole::User == UserRole::User);
        assert!(UserRole::Teacher == UserRole::Teacher);
        assert!(UserRole::Moderator == UserRole::Moderator);
        assert!(UserRole::Admin == UserRole::Admin);

        assert!(UserRole::Admin > UserRole::Moderator);
    }

    #[test]
    fn serialize_userroles_as_strings() {
        let role_str = serde_json::to_string(&UserRole::User).unwrap();
        assert_eq!(&role_str, "\"user\"");
    }

    #[test]
    fn deserialize_app_id_lowercase() {
        let app_id_str = String::from("\"NetsBlox\"");
        let app_id: AppId = serde_json::from_str(&app_id_str).unwrap();
        assert_eq!(&app_id.as_str(), &"netsblox");
        assert_eq!(app_id, AppId::new("netsblox"));
    }

    #[test]
    fn publish_state_priv_lt_pending() {
        assert!(PublishState::Private < PublishState::PendingApproval);
    }

    #[test]
    fn publish_state_pending_lt_public() {
        assert!(PublishState::PendingApproval < PublishState::Public);
    }

    #[test]
    fn publish_state_public_eq() {
        assert!(PublishState::Public == PublishState::Public);
    }
}
