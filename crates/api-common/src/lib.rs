#[cfg(feature = "bson")]
mod bson;
pub mod oauth;

use core::fmt;
use derive_more::{Display, Error, FromStr};
use lazy_static::lazy_static;
use regex::Regex;
use rustrict::CensorStr;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::Value;
use std::{collections::HashMap, marker::PhantomData, str::FromStr, time::SystemTime};
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

pub struct ValidateOptions {
    min: usize,
    max: usize,
    deny_profanity: bool,
}

pub trait Validate {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E>;
    fn get_options() -> ValidateOptions;
    fn validate<E: de::Error>(str: impl Into<String>) -> Result<String, E> {
        let ValidateOptions {
            min,
            max,
            deny_profanity,
            ..
        } = Self::get_options();
        let string: String = str.into();
        let char_count = string.chars().count();

        if char_count < min || char_count > max {
            let exp = format!("Name must be between {min} and {max} characters");
            return Err(E::invalid_length(char_count, &exp.as_str()));
        }

        if deny_profanity && string.is_inappropriate() {
            return Err(E::invalid_value(
                de::Unexpected::Other("profanity"),
                &"a name without profanity",
            ));
        }

        Self::test_regex(&string)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, TS)]
pub struct Name<T: Validate>(String, std::marker::PhantomData<T>);

impl<T: Validate> Name<T> {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Name::<T>(name.into(), std::marker::PhantomData::<T>)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: Validate> fmt::Display for Name<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write! {f, "{}", self.0}
    }
}

struct NameVisitor<T: Validate>(PhantomData<T>);

// //TODO: FUTURE: look for modern alternative to lazy_static
// //TODO: FUTURE: Look for a crate that would do this cleanly
// //TODO: FUTURE: It may be harder for Name, due to the
// //TODO: FUTURE: Setup custom errors in src/error.rs
impl<'de, T: Validate> Visitor<'de> for NameVisitor<T> {
    type Value = Name<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(r"a string that matches the regex: ^[\w\d_][\w\d_ \(\)\.,'\-!]*$")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        let name: String = T::validate(value.to_owned())?;
        Ok(Name(name, PhantomData::<T>))
    }
}
impl<'de, T: Validate> Deserialize<'de> for Name<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(NameVisitor::<T>(PhantomData::<T>))
    }
}
impl<T: Validate> Serialize for Name<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct ProjectNameValidator;
pub type ProjectName = Name<ProjectNameValidator>;

impl Validate for ProjectNameValidator {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E> {
        lazy_static! {
            static ref PROJECTNAME_REGEX: Regex =
                Regex::new(r"^[\w\d_][\w\d_ \(\)\.,'\-!]*$").unwrap();
        }
        if PROJECTNAME_REGEX.is_match(str) {
            Ok(str.to_owned())
        } else {
            Err(E::invalid_value(
                de::Unexpected::Other("invalid characters"),
                &r"a name without certain special characters. 
                '(', ')', '!', ',', '.', ''', and '-' are allowed, but not as the first character. 
                Regex: '^[\w\d_][\w\d_ \(\)\.,'!-]*$'",
            ))
        }
    }

    fn get_options() -> ValidateOptions {
        ValidateOptions {
            min: 1,
            max: 50,
            deny_profanity: true,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct LibraryNameValidator;
pub type LibraryName = Name<LibraryNameValidator>;

impl Validate for LibraryNameValidator {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E> {
        lazy_static! {
            static ref LIBRARYNAME_REGEX: Regex = Regex::new(r"^[A-zÀ-ÿ0-9 \(\)_-]+$").unwrap();
        }

        if LIBRARYNAME_REGEX.is_match(str) {
            Ok(str.to_owned())
        } else {
            Err(E::invalid_value(
                de::Unexpected::Other("invalid characters"),
                &"a name without certain special characters.
                '(', ')', '_', and '-' are allowed.
                Regex: \'^[A-zÀ-ÿ0-9 \\(\\)_-]+$\'",
            ))
        }
    }
    fn get_options() -> ValidateOptions {
        ValidateOptions {
            min: 1,
            max: 100,
            deny_profanity: false,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct UsernameValidator;
pub type Username = Name<UsernameValidator>;

impl Validate for UsernameValidator {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E> {
        lazy_static! {
            static ref USERNAME_REGEX: Regex = Regex::new(r"^[A-z][A-Z0-9_\-]+$").unwrap();
        }

        if USERNAME_REGEX.is_match(str) {
            Ok(str.to_owned())
        } else {
            Err(E::invalid_value(
                de::Unexpected::Other("invalid characters"),
                &"a name without special characters.
                [0-9], '-', and '_' are allowed, but not for the first character.
                Regex: \'^[A-z][A-z0-9_\\-]\'",
            ))
        }
    }

    fn get_options() -> ValidateOptions {
        ValidateOptions {
            min: 1,
            max: 50,
            deny_profanity: true,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct GroupNameValidator;
pub type GroupName = Name<GroupNameValidator>;

impl Validate for GroupNameValidator {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E> {
        lazy_static! {
            static ref GROUPNAME_REGEX: Regex =
                Regex::new(r"^[\w\d_][\w\d_ \(\)\.,'\-!]*$").unwrap();
        }
        if GROUPNAME_REGEX.is_match(str) {
            Ok(str.to_owned())
        } else {
            Err(E::invalid_value(
                de::Unexpected::Other("invalid characters"),
                &r"a name without certain special characters. 
                '(', ')', '!', ',', '.', ''', and '-' are allowed, but not as the first character. 
                Regex: '^[\w\d_][\w\d_ \(\)\.,'!-]*$'",
            ))
        }
    }

    fn get_options() -> ValidateOptions {
        ValidateOptions {
            min: 1,
            max: 50,
            deny_profanity: true,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct RoleNameValidator;
pub type RoleName = Name<RoleNameValidator>;

impl Validate for RoleNameValidator {
    fn test_regex<E: de::Error>(str: &str) -> Result<String, E> {
        lazy_static! {
            static ref ROLENAME_REGEX: Regex =
                Regex::new(r"^[\w\d_][\w\d_ \(\)\.,'\-!]*$").unwrap();
        }
        if ROLENAME_REGEX.is_match(str) {
            Ok(str.to_owned())
        } else {
            Err(E::invalid_value(
                de::Unexpected::Other("invalid characters"),
                &r"a name without certain special characters. 
                '(', ')', '!', ',', '.', ''', and '-' are allowed, but not as the first character. 
                Regex: '^[\w\d_][\w\d_ \(\)\.,'!-]*$'",
            ))
        }
    }

    fn get_options() -> ValidateOptions {
        ValidateOptions {
            min: 1,
            max: 50,
            deny_profanity: true,
        }
    }
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

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: ProjectName,
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
    pub name: RoleName,
    pub code: String,
    pub media: String,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Project {
    pub id: ProjectId,
    pub owner: String,
    pub name: ProjectName,
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
    pub name: RoleName,
    pub code: String,
    pub media: String,
}

impl RoleData {
    pub fn to_xml(&self) -> String {
        let name = self.name.to_string().replace('\"', "\\\"");
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
    pub name: GroupName,
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
    pub name: GroupName,
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
    pub name: ProjectName,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize, Debug, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateRoleData {
    pub name: RoleName,
    #[ts(optional)]
    pub client_id: Option<ClientId>,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateProjectData {
    #[ts(optional)]
    pub owner: Option<String>,
    pub name: ProjectName,
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
    pub name: RoleName,
    pub roles: HashMap<RoleId, RoleState>,
    pub collaborators: Vec<String>,
    pub version: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct RoleState {
    pub name: RoleName,
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
    fn deserialize_shortprojectname_error() {
        let name_str = String::from("\"\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
    }

    #[test]
    fn deserialize_profanity_projectname_error() {
        let name_str_1 = String::from("\"FUCK\"");
        let name_str_2 = String::from("\"DICK\"");
        let name_str_3 = String::from("\"hell\"");
        let name_str_4 = String::from("\"shitter\"");
        let name_str_5 = String::from("\"fukker\"");
        let name_str_6 = String::from("\"f@g\"");
        let name_1: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_1);
        let name_2: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_2);
        let name_3: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_3);
        let name_4: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_4);
        let name_5: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_5);
        let name_6: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str_6);
        assert!(name_1.is_err());
        assert!(name_2.is_err());
        assert!(name_3.is_err());
        assert!(name_4.is_err());
        assert!(name_5.is_err());
        assert!(name_6.is_err());
    }

    #[test]
    fn deserialize_leading_dash_projectname_error() {
        let name_str = String::from("\"-name\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
    }

    #[test]
    fn deserialize_leading_parentheses_projectname_error() {
        let name_str = String::from("\"(name\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
    }

    #[test]
    fn deserialize_leading_period_projectname_error() {
        let name_str = String::from("\".name\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
    }

    #[test]
    fn deserialize_x_is_valid_projectname() {
        let name_str = String::from("\"X\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_spaces() {
        let name_str = String::from("\"Player 1\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_leading_nums() {
        let name_str = String::from("\"2048 Game\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_dashes() {
        let name_str = String::from("\"player-i\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_long_name() {
        let name_str = String::from("\"RENAMED-rename-test-1696865702584\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_parens() {
        let name_str = String::from("\"untitled (20)\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_dots() {
        let name_str = String::from("\"untitled v1.2\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_comma() {
        let name_str = String::from("\"Lab2, SomeName\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_apostrophe() {
        let name_str = String::from("\"Brian's project\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_projectname_bang() {
        let name_str = String::from("\"Hello!\"");
        let name: Result<ProjectName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_libraryname() {
        let name_str = String::from("\"hello library\"");
        let name: Result<LibraryName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_libraryname_diacritic() {
        let name_str = String::from("\"hola libré\"");
        let name: Result<LibraryName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_ok());
    }

    #[test]
    fn deserialize_is_valid_libraryname_weird_symbol() {
        let name_str = String::from("\"<hola libré>\"");
        let name: Result<LibraryName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
    }

    #[test]
    fn deserialize_is_valid_libraryname_profanity() {
        let name_str = String::from("\"<hola pendejo>\"");
        let name: Result<LibraryName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
        let name_str = String::from("\"<hola fucker>\"");
        let name: Result<LibraryName, serde_json::Error> = serde_json::from_str(&name_str);
        assert!(name.is_err());
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
