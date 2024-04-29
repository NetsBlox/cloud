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
use std::{collections::HashMap, str::FromStr, time::SystemTime};
use ts_rs::TS;
use uuid::Uuid;

const APP_NAME: &str = "NetsBlox";

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientConfig {
    pub client_id: String,
    #[ts(optional)]
    pub username: Option<String>,
    pub services_hosts: Vec<ServiceHost>,
    pub cloud_url: String,
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct InvitationResponse {
    pub response: FriendLinkState,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    pub username: String,
    pub email: String,
    #[ts(optional)]
    pub group_id: Option<GroupId>,
    pub role: UserRole,
    #[ts(skip)]
    pub created_at: SystemTime,
    pub linked_accounts: Vec<LinkedAccount>,
    #[ts(optional)]
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize, TS, Clone)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateUserData {
    pub email: Option<String>,
    pub group_id: Option<GroupId>,
    pub role: Option<UserRole>,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewUser {
    pub username: String,
    pub email: String,
    #[ts(optional)]
    pub password: Option<String>,
    #[ts(optional)]
    pub group_id: Option<GroupId>,
    #[ts(optional)]
    pub role: Option<UserRole>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UserRole {
    User,
    Teacher,
    Moderator,
    Admin,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkTraceMetadata {
    pub id: String,
    #[ts(type = "any")] // FIXME
    pub start_time: SystemTime,
    #[ts(type = "any | null")] // FIXME
    #[ts(optional)]
    pub end_time: Option<SystemTime>,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SentMessage {
    pub project_id: ProjectId,
    pub recipients: Vec<ClientState>,
    #[ts(type = "any")] // FIXME
    pub time: SystemTime,
    pub source: ClientState,

    #[ts(type = "any")]
    pub content: serde_json::Value,
}

#[derive(TS, Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OccupantInvite {
    pub username: String,
    pub project_id: ProjectId,
    pub role_id: RoleId,
    #[ts(type = "any")] // FIXME
    pub created_at: SystemTime,
}

#[derive(Debug, Display, Error, TS)]
#[display(fmt = "Unable to parse user role. Expected admin, moderator, or user.")]
#[ts(export)]
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

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceHost {
    pub url: String,
    pub categories: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct LinkedAccount {
    pub username: String,
    pub strategy: String,
}

#[derive(TS, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BannedAccount {
    pub username: String,
    pub email: String,
    #[ts(type = "any")] // FIXME
    pub banned_at: SystemTime,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoginRequest {
    pub credentials: Credentials,
    #[ts(optional)]
    pub client_id: Option<ClientId>, // TODO: add a secret token for the client?
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[ts(export)]
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
#[derive(TS, Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FriendLink {
    pub id: FriendLinkId,
    pub sender: String,
    pub recipient: String,
    pub state: FriendLinkState,
    #[ts(type = "any")] // FIXME
    pub created_at: SystemTime,
    #[ts(type = "any")] // FIXME
    pub updated_at: SystemTime,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
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

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FriendInvite {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    #[ts(type = "any")] // FIXME
    pub created_at: SystemTime,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, TS)]
#[ts(export)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(id: String) -> Self {
        ProjectId(id)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, TS)]
#[ts(export)]
pub struct RoleId(String);

impl RoleId {
    pub fn new(id: String) -> Self {
        RoleId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, TS)]
#[ts(export)]
pub struct S3Key(String);

impl S3Key {
    pub fn new(key: String) -> Self {
        S3Key(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    #[ts(type = "any")] // FIXME
    pub updated: SystemTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub network_traces: Vec<NetworkTraceMetadata>,
    #[ts(type = "any")] // FIXME
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<RoleId, RoleMetadata>,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub enum SaveState {
    Created,
    Transient,
    Broken,
    Saved,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoleMetadata {
    pub name: String,
    pub code: S3Key,
    pub media: S3Key,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Project {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    #[ts(type = "any")] // FIXME
    pub updated: SystemTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    #[ts(type = "any")] // FIXME
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

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoleDataResponse {
    pub id: Uuid,
    pub data: RoleData,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[ts(export)]
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

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientStateData {
    pub state: ClientState,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClientState {
    Browser(BrowserClientState),
    External(ExternalClientState),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowserClientState {
    pub role_id: RoleId,
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Clone, Hash, Eq, PartialEq, TS)]
#[ts(export)]
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalClientState {
    pub address: String,
    pub app_id: AppId,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateLibraryData {
    pub name: String,
    pub notes: String,
    pub blocks: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, TS)]
#[ts(export)]
pub enum PublishState {
    Private,
    ApprovalDenied,
    PendingApproval,
    Public,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
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

#[derive(Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateGroupData {
    pub name: String,
    #[ts(optional)]
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, FromStr, TS)]
#[ts(export)]
pub struct GroupId(String);

impl GroupId {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Group {
    pub id: GroupId,
    pub owner: String,
    pub name: String,
    #[ts(optional)]
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateGroupData {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, FromStr, TS)]
#[ts(export)]
pub struct GalleryId(String);

impl GalleryId {
    #[must_use]
    pub fn new(name: String) -> Self {
        Self(name)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Gallery {
    pub id: GalleryId,
    pub owner: String,
    pub name: String,
    pub state: PublishState,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeGalleryData {
    #[ts(optional)]
    // TODO: Future, use newtype and add constraints.
    // In general, we need to automate checking for a valid 'name'
    //
    pub name: Option<String>,
    #[ts(optional)]
    pub state: Option<PublishState>,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GalleryProjectMetadata {
    pub gallery_id: GalleryId,
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    #[ts(type = "any")] // FIXME
    pub updated: SystemTime,
    #[ts(type = "any")] // FIXME
    pub origin_time: SystemTime,
    pub thumbnail: String,
    pub versions: Vec<Option<S3Key>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateGalleryProjectData {
    pub owner: String,
    pub name: String,
    pub thumbnail: String, // should I extract from xml?
    pub project_xml: String,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateGalleryProjectData {
    pub owner: String,
    pub name: String,
    pub thumbnail: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub enum InvitationState {
    Pending,
    Accepted,
    Rejected,
}

pub type InvitationId = String;

#[derive(TS, Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollaborationInvite {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub project_id: ProjectId,
    pub state: InvitationState,
    #[ts(type = "any")] // FIXME
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

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateProjectData {
    pub name: String,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateRoleData {
    pub name: String,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateProjectData {
    #[ts(optional)]
    pub owner: Option<String>,
    pub name: String,
    #[ts(optional)]
    pub roles: Option<Vec<RoleData>>,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
    #[ts(optional)]
    pub save_state: Option<SaveState>,
}

// Network debugging data
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, TS)]
#[ts(export)]
pub struct ClientId(String);

impl ClientId {
    pub fn new(addr: String) -> Self {
        Self(addr)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Display, Error, TS)]
#[display(fmt = "Invalid client ID. Must start with a _")]
#[ts(export)]
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

#[derive(Deserialize, Serialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExternalClient {
    #[ts(optional)]
    pub username: Option<String>,
    pub address: String,
    pub app_id: AppId,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoomState {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub roles: HashMap<RoleId, RoleState>,
    pub collaborators: Vec<String>,
    pub version: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoleState {
    pub name: String,
    pub occupants: Vec<OccupantState>,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct OccupantState {
    pub id: ClientId,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OccupantInviteData {
    pub username: String,
    pub role_id: RoleId,
    #[ts(optional)]
    pub sender: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorizedServiceHost {
    pub url: String,
    pub id: String,
    pub visibility: ServiceHostScope,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ServiceHostScope {
    Public(Vec<String>),
    Private,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientInfo {
    #[ts(optional)]
    pub username: Option<String>,
    #[ts(optional)]
    pub state: Option<ClientState>,
}

/// Service settings for a given user categorized by origin
#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[ts(export)]
pub struct ServiceSettings {
    /// Service settings owned by the user
    #[ts(optional)]
    pub user: Option<String>,
    /// Service settings owned by a group in which the user is a member
    #[ts(optional)]
    pub member: Option<String>,
    /// Service settings owned by a groups created by the user
    pub groups: HashMap<GroupId, String>,
}

/// Send message request (for authorized services)
#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SendMessage {
    pub sender: Option<SendMessageSender>,
    pub target: SendMessageTarget,
    // TODO: Should we only allow "message" types or any sort of message?
    #[ts(type = "object")]
    pub content: Value,
}

/// Send message request (for authorized services)
#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogMessage {
    pub sender: String,
    pub recipients: Vec<String>,
    // TODO: Should we only allow "message" types or any sort of message?
    #[ts(type = "object")]
    pub content: Value,
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SendMessageSender {
    Username(String),
    Client(ClientId),
}

#[derive(Deserialize, Serialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
        #[ts(optional)]
        state: Option<ClientState>,
        client_id: ClientId,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct MagicLinkId(String);

impl MagicLinkId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MagicLinkLoginData {
    pub link_id: MagicLinkId,
    pub username: String,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
    #[ts(optional)]
    pub redirect_uri: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateMagicLinkData {
    pub email: String,
    #[ts(optional)]
    pub redirect_uri: Option<String>,
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

        assert!(UserRole::Moderator > UserRole::Teacher);
        assert!(UserRole::Admin > UserRole::Teacher);

        assert!(UserRole::Admin > UserRole::Moderator);

        assert!(UserRole::User == UserRole::User);
        assert!(UserRole::Teacher == UserRole::Teacher);
        assert!(UserRole::Moderator == UserRole::Moderator);
        assert!(UserRole::Admin == UserRole::Admin);
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
