use mongodb::bson::{self, doc, document::Document, Bson, DateTime};
pub use netsblox_api_common as api;
use netsblox_api_common::{
    oauth, ClientState, LibraryMetadata, NewUser, PublishState, RoleId, UserRole,
};
use netsblox_api_common::{
    FriendInvite, FriendLinkState, GalleryId, GroupId, InvitationState, LinkedAccount, ProjectId,
    RoleData, S3Key, SaveState, ServiceHost, ServiceHostScope,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub hash: String,
    pub salt: Option<String>,
    pub group_id: Option<GroupId>,
    pub role: UserRole,
    pub created_at: DateTime,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
    pub service_settings: HashMap<String, String>,
}

impl User {
    pub fn is_member(&self) -> bool {
        self.group_id.is_some()
    }
}

impl From<User> for Bson {
    fn from(user: User) -> Bson {
        Bson::Document(doc! {
            "username": user.username,
            "email": user.email,
            "hash": user.hash,
            "salt": user.salt,
            "groupId": user.group_id,
            "role": user.role,
            "createdAt": user.created_at,
            "linkedAccounts": user.linked_accounts,
            "servicesHosts": user.services_hosts,
            "serviceSettings": bson::to_bson(&user.service_settings).unwrap(),
        })
    }
}

impl From<User> for netsblox_api_common::User {
    fn from(user: User) -> netsblox_api_common::User {
        netsblox_api_common::User {
            username: user.username,
            email: user.email,
            group_id: user.group_id,
            role: user.role,
            created_at: user.created_at.to_system_time(),
            linked_accounts: user.linked_accounts,
            services_hosts: user.services_hosts,
        }
    }
}

impl From<NewUser> for User {
    fn from(user_data: NewUser) -> Self {
        let salt = passwords::PasswordGenerator::new()
            .length(8)
            .exclude_similar_characters(true)
            .numbers(true)
            .spaces(false)
            .generate_one()
            .unwrap_or_else(|_err| "salt".to_owned());

        let hash: String = if let Some(pwd) = user_data.password {
            sha512(&(pwd + &salt))
        } else {
            "None".to_owned()
        };

        User {
            username: user_data.username,
            hash,
            salt: Some(salt),
            email: user_data.email,
            group_id: user_data.group_id,
            created_at: DateTime::from_system_time(SystemTime::now()),
            linked_accounts: std::vec::Vec::new(),
            role: user_data.role.unwrap_or(UserRole::User),
            services_hosts: None,
            service_settings: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BannedAccount {
    pub username: String,
    pub email: String,
    pub banned_at: DateTime,
}

impl BannedAccount {
    pub fn new(username: String, email: String) -> BannedAccount {
        let banned_at = DateTime::now();
        BannedAccount {
            username,
            email,
            banned_at,
        }
    }
}

impl From<BannedAccount> for Bson {
    fn from(account: BannedAccount) -> Self {
        Bson::Document(doc! {
            "username": account.username,
            "email": account.email,
            "bannedAt": account.banned_at,
        })
    }
}

impl From<BannedAccount> for api::BannedAccount {
    fn from(account: BannedAccount) -> Self {
        api::BannedAccount {
            username: account.username,
            email: account.email,
            banned_at: account.banned_at.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub owner: String,
    pub name: String,
    pub services_hosts: Option<Vec<ServiceHost>>,
    pub service_settings: HashMap<String, String>,
}

impl Group {
    pub fn new(owner: String, name: String) -> Self {
        Self {
            id: api::GroupId::new(Uuid::new_v4().to_string()),
            name,
            owner,
            service_settings: HashMap::new(),
            services_hosts: None,
        }
    }

    pub fn from_data(owner: String, data: api::CreateGroupData) -> Self {
        Self {
            id: api::GroupId::new(Uuid::new_v4().to_string()),
            owner,
            name: data.name,
            service_settings: HashMap::new(),
            services_hosts: data.services_hosts,
        }
    }
}

impl From<Group> for netsblox_api_common::Group {
    fn from(group: Group) -> netsblox_api_common::Group {
        netsblox_api_common::Group {
            id: group.id,
            owner: group.owner,
            name: group.name,
            services_hosts: group.services_hosts,
        }
    }
}

impl From<Group> for Bson {
    fn from(group: Group) -> Self {
        let mut settings = Document::new();
        group.service_settings.into_iter().for_each(|(k, v)| {
            settings.insert(k, v);
        });

        Bson::Document(doc! {
            "id": group.id,
            "owner": group.owner,
            "name": group.name,
            "serviceSettings": settings,
            "servicesHosts": group.services_hosts,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub project_id: ProjectId,
    pub state: InvitationState,
    pub created_at: DateTime,
}

impl CollaborationInvite {
    pub fn new(sender: String, receiver: String, project_id: ProjectId) -> Self {
        CollaborationInvite {
            id: Uuid::new_v4().to_string(),
            sender,
            receiver,
            project_id,
            state: InvitationState::Pending,
            created_at: DateTime::from_system_time(SystemTime::now()),
        }
    }
}

impl From<CollaborationInvite> for Bson {
    fn from(invite: CollaborationInvite) -> Self {
        Bson::Document(doc! {
            "id": invite.id,
            "sender": invite.sender,
            "receiver": invite.receiver,
            "projectId": invite.project_id,
            "state": invite.state,
            "createdAt": invite.created_at,
        })
    }
}

impl From<CollaborationInvite> for netsblox_api_common::CollaborationInvite {
    fn from(user: CollaborationInvite) -> netsblox_api_common::CollaborationInvite {
        netsblox_api_common::CollaborationInvite {
            id: user.id,
            sender: user.sender,
            receiver: user.receiver,
            project_id: user.project_id,
            state: user.state,
            created_at: user.created_at.to_system_time(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FriendLink {
    pub id: api::FriendLinkId,
    pub sender: String,
    pub recipient: String,
    pub state: FriendLinkState,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

impl FriendLink {
    pub fn new(sender: String, recipient: String, state: Option<FriendLinkState>) -> FriendLink {
        let created_at = DateTime::from_system_time(SystemTime::now());
        FriendLink {
            id: Uuid::new_v4().to_string(),
            sender,
            recipient,
            state: state.unwrap_or(FriendLinkState::Pending),
            created_at,
            updated_at: created_at,
        }
    }
}

impl From<FriendLink> for api::FriendLink {
    fn from(link: FriendLink) -> api::FriendLink {
        api::FriendLink {
            id: link.id,
            sender: link.sender,
            recipient: link.recipient,
            state: link.state,
            created_at: link.created_at.into(),
            updated_at: link.updated_at.into(),
        }
    }
}

impl From<FriendLink> for FriendInvite {
    fn from(link: FriendLink) -> FriendInvite {
        FriendInvite {
            id: link.id,
            sender: link.sender,
            recipient: link.recipient,
            created_at: link.created_at.to_system_time(),
        }
    }
}

impl From<FriendLink> for Bson {
    fn from(link: FriendLink) -> Bson {
        Bson::Document(doc! {
            "id": link.id,
            "sender": link.sender,
            "recipient": link.recipient,
            "state": link.state,
            "createdAt": link.created_at,
            "updatedAt": link.updated_at,
        })
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkTraceMetadata {
    pub id: String,
    pub start_time: DateTime,
    pub end_time: Option<DateTime>,
}

impl NetworkTraceMetadata {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            start_time: DateTime::now(),
            end_time: None,
        }
    }
}

impl From<NetworkTraceMetadata> for Bson {
    fn from(link: NetworkTraceMetadata) -> Bson {
        Bson::Document(doc! {
            "id": link.id,
            "startTime": link.start_time,
            "endTime": link.end_time,
        })
    }
}

impl From<NetworkTraceMetadata> for netsblox_api_common::NetworkTraceMetadata {
    fn from(trace: NetworkTraceMetadata) -> netsblox_api_common::NetworkTraceMetadata {
        netsblox_api_common::NetworkTraceMetadata {
            id: trace.id,
            start_time: trace.start_time.into(),
            end_time: trace.end_time.map(|t| t.into()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Bucket(String);

impl Bucket {
    pub fn new(bucket: String) -> Self {
        Bucket(bucket)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// TODO: Explain gallery projects
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GalleryProjectMetadata {
    pub gallery_id: GalleryId,
    pub id: ProjectId,

    // owner (for permissions)
    pub owner: String,

    // metadata
    pub name: String,
    pub updated: DateTime,
    pub origin_time: DateTime,
    pub thumbnail: String,
    // (path to the) xml contents (on s3)
    //pub versions: Vec<String>,  // TODO: worry about versions later
    pub content: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: DateTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime,
    pub save_state: SaveState,
    pub delete_at: Option<DateTime>,
    pub network_traces: Vec<NetworkTraceMetadata>,
    pub roles: HashMap<RoleId, RoleMetadata>,
}

impl ProjectMetadata {
    pub fn new(
        owner: &str,
        name: &str,
        roles: HashMap<RoleId, RoleMetadata>,
        save_state: SaveState,
    ) -> ProjectMetadata {
        let origin_time = DateTime::now();

        let delete_at = match save_state {
            SaveState::Saved => None,
            _ => {
                // if not saved, set the project to be deleted in 10 minutes if not joined
                let ten_minutes = Duration::new(10 * 60, 0);
                let ten_mins_from_now = SystemTime::now().checked_add(ten_minutes).unwrap();
                Some(DateTime::from_system_time(ten_mins_from_now))
            }
        };

        ProjectMetadata {
            id: ProjectId::new(Uuid::new_v4().to_string()),
            owner: owner.to_owned(),
            name: name.to_owned(),
            updated: origin_time,
            origin_time,
            state: PublishState::Private,
            collaborators: vec![],
            save_state,
            delete_at,
            network_traces: Vec::new(),
            roles,
        }
    }
}

impl From<ProjectMetadata> for Bson {
    fn from(metadata: ProjectMetadata) -> Bson {
        let mut roles = Document::new();
        metadata.roles.into_iter().for_each(|(id, md)| {
            roles.insert(id.as_str(), md);
        });

        Bson::Document(doc! {
            "id": metadata.id,
            "owner": metadata.owner,
            "name": metadata.name,
            "updated": metadata.updated,
            "originTime": metadata.origin_time,
            "state": metadata.state,
            "collaborators": metadata.collaborators,
            "saveState": metadata.save_state,
            "roles": roles,
            "deleteAt": metadata.delete_at,
            "networkTraces": metadata.network_traces,
        })
    }
}

impl From<ProjectMetadata> for netsblox_api_common::ProjectMetadata {
    fn from(metadata: ProjectMetadata) -> netsblox_api_common::ProjectMetadata {
        netsblox_api_common::ProjectMetadata {
            id: metadata.id,
            owner: metadata.owner,
            name: metadata.name,
            origin_time: metadata.origin_time.to_system_time(),
            updated: metadata.updated.to_system_time(),
            state: metadata.state,
            collaborators: metadata.collaborators,
            save_state: metadata.save_state,
            network_traces: metadata
                .network_traces
                .into_iter()
                .map(|t| t.into())
                .collect(),
            roles: metadata
                .roles
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: DateTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime,
    pub save_state: SaveState,
    pub roles: HashMap<RoleId, RoleData>,
}

impl From<Project> for netsblox_api_common::Project {
    fn from(project: Project) -> netsblox_api_common::Project {
        netsblox_api_common::Project {
            id: project.id,
            owner: project.owner,
            name: project.name,
            origin_time: project.origin_time.to_system_time(),
            updated: project.updated.to_system_time(),
            state: project.state,
            collaborators: project.collaborators,
            save_state: project.save_state,
            roles: project.roles,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoleMetadata {
    pub name: String,
    pub code: S3Key,
    pub media: S3Key,
    pub updated: DateTime,
}

impl From<RoleMetadata> for netsblox_api_common::RoleMetadata {
    fn from(metadata: RoleMetadata) -> netsblox_api_common::RoleMetadata {
        netsblox_api_common::RoleMetadata {
            name: metadata.name,
            code: metadata.code.clone(),
            media: metadata.media.clone(),
        }
    }
}

impl From<RoleMetadata> for Bson {
    fn from(metadata: RoleMetadata) -> Bson {
        Bson::Document(doc! {
            "name": metadata.name,
            "code": metadata.code,
            "media": metadata.media,
            "updated": metadata.updated,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OccupantInvite {
    pub username: String,
    pub project_id: ProjectId,
    pub role_id: RoleId,
    created_at: DateTime,
}

impl OccupantInvite {
    pub fn new(target: String, project_id: ProjectId, role_id: RoleId) -> Self {
        OccupantInvite {
            project_id,
            username: target,
            role_id,
            created_at: DateTime::from_system_time(SystemTime::now()),
        }
    }
}

impl From<OccupantInvite> for api::OccupantInvite {
    fn from(invite: OccupantInvite) -> api::OccupantInvite {
        api::OccupantInvite {
            username: invite.username,
            project_id: invite.project_id,
            role_id: invite.role_id,
            created_at: invite.created_at.into(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub project_id: ProjectId,
    pub recipients: Vec<ClientState>,
    pub time: DateTime,
    pub source: ClientState,

    pub content: serde_json::Value,
}

/// log message type
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogMessage {
    pub sender: String,
    pub recipients: Vec<String>,
    pub content: serde_json::Value,
    pub created_at: DateTime,
}

// NOTE: timestamped on conversion
impl From<api::LogMessage> for LogMessage {
    fn from(value: api::LogMessage) -> Self {
        LogMessage {
            sender: value.sender,
            recipients: value.recipients,
            content: value.content,
            created_at: DateTime::now(),
        }
    }
}

impl From<LogMessage> for api::LogMessage {
    fn from(value: LogMessage) -> Self {
        api::LogMessage {
            sender: value.sender,
            recipients: value.recipients,
            content: value.content,
        }
    }
}

impl SentMessage {
    pub fn new(
        project_id: ProjectId,
        source: ClientState,
        recipients: Vec<ClientState>,
        content: serde_json::Value,
    ) -> Self {
        let time = DateTime::now();
        SentMessage {
            project_id,
            recipients,
            time,
            source,
            content,
        }
    }
}

impl From<SentMessage> for api::SentMessage {
    fn from(msg: SentMessage) -> api::SentMessage {
        api::SentMessage {
            project_id: msg.project_id,
            recipients: msg.recipients,
            time: msg.time.into(),
            source: msg.source,
            content: msg.content,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetPasswordToken {
    pub username: String,
    pub secret: String,
    pub created_at: DateTime,
}

impl SetPasswordToken {
    pub fn new(username: String) -> Self {
        let secret = Uuid::new_v4().to_string();
        let created_at = DateTime::from_system_time(SystemTime::now());

        SetPasswordToken {
            username,
            secret,
            created_at,
        }
    }
}

impl From<SetPasswordToken> for Bson {
    fn from(token: SetPasswordToken) -> Bson {
        Bson::Document(doc! {
            "username": token.username,
            "secret": token.secret,
            "createdAt": token.created_at,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedServiceHost {
    pub url: String,
    pub id: String,
    pub visibility: ServiceHostScope,
    pub secret: String,
}

impl AuthorizedServiceHost {
    pub fn new(url: String, id: String, visibility: ServiceHostScope) -> Self {
        let secret = Uuid::new_v4().to_string();
        AuthorizedServiceHost {
            url,
            id,
            secret,
            visibility,
        }
    }

    pub fn auth_header(&self) -> (&'static str, String) {
        let token = self.id.clone() + ":" + &self.secret;
        ("X-Authorization", token)
    }
}

impl From<AuthorizedServiceHost> for Bson {
    fn from(host: AuthorizedServiceHost) -> Bson {
        Bson::Document(doc! {
            "url": host.url,
            "id": host.id,
            "visibility": host.visibility,
            "secret": host.secret,
        })
    }
}

impl From<netsblox_api_common::AuthorizedServiceHost> for AuthorizedServiceHost {
    fn from(data: netsblox_api_common::AuthorizedServiceHost) -> AuthorizedServiceHost {
        AuthorizedServiceHost::new(data.url, data.id, data.visibility)
    }
}

impl From<AuthorizedServiceHost> for netsblox_api_common::AuthorizedServiceHost {
    fn from(host: AuthorizedServiceHost) -> netsblox_api_common::AuthorizedServiceHost {
        netsblox_api_common::AuthorizedServiceHost {
            id: host.id,
            url: host.url,
            visibility: host.visibility,
        }
    }
}

impl From<AuthorizedServiceHost> for netsblox_api_common::ServiceHost {
    fn from(host: AuthorizedServiceHost) -> netsblox_api_common::ServiceHost {
        let categories = match host.visibility {
            ServiceHostScope::Public(cats) => cats,
            ServiceHostScope::Private => Vec::new(),
        };

        netsblox_api_common::ServiceHost {
            url: host.url,
            categories,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub owner: String,
    pub name: String,
    pub notes: String,
    pub blocks: String,
    pub state: PublishState,
}

impl From<Library> for LibraryMetadata {
    fn from(library: Library) -> LibraryMetadata {
        LibraryMetadata::new(
            library.owner.clone(),
            library.name.clone(),
            library.state,
            Some(library.notes),
        )
    }
}

impl From<Library> for Bson {
    fn from(library: Library) -> Self {
        Bson::Document(doc! {
            "owner": library.owner,
            "name": library.name,
            "notes": library.notes,
            "blocks": library.blocks,
            "state": library.state,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthClient {
    pub id: oauth::ClientId,
    pub name: String,
    created_at: DateTime,
    hash: String,
    salt: String,
}

impl OAuthClient {
    pub fn new(name: String, password: String) -> Self {
        let salt = passwords::PasswordGenerator::new()
            .length(8)
            .exclude_similar_characters(true)
            .numbers(true)
            .spaces(false)
            .generate_one()
            .unwrap_or_else(|_err| "salt".to_owned());

        let hash = sha512(&(password + &salt));
        Self {
            id: oauth::ClientId::new(Uuid::new_v4().to_string()),
            name,
            created_at: DateTime::from_system_time(SystemTime::now()),
            hash,
            salt,
        }
    }
}

impl From<OAuthClient> for Bson {
    fn from(client: OAuthClient) -> Bson {
        Bson::Document(doc! {
            "id": client.id,
            "name": client.name,
            "createdAt": client.created_at,
            "hash": client.hash,
            "salt": client.salt,
        })
    }
}

impl From<OAuthClient> for oauth::Client {
    fn from(client: OAuthClient) -> oauth::Client {
        oauth::Client {
            id: client.id,
            name: client.name,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthToken {
    pub id: oauth::TokenId,
    pub client_id: oauth::ClientId,
    pub username: String,
    pub created_at: DateTime,
}

impl OAuthToken {
    pub fn new(client_id: oauth::ClientId, username: String) -> Self {
        let id = oauth::TokenId::new(Uuid::new_v4().to_string());
        let created_at = DateTime::from_system_time(SystemTime::now());

        Self {
            id,
            client_id,
            username,
            created_at,
        }
    }
}

impl From<OAuthToken> for oauth::Token {
    fn from(token: OAuthToken) -> oauth::Token {
        oauth::Token {
            id: token.id,
            client_id: token.client_id,
            username: token.username,
            created_at: token.created_at.to_system_time(),
        }
    }
}

impl From<OAuthToken> for Bson {
    fn from(token: OAuthToken) -> Bson {
        Bson::Document(doc! {
            "id": token.id,
            "client_id": token.client_id,
            "username": token.username,
            "createdAt": token.created_at,
        })
    }
}

pub(crate) fn sha512(text: &str) -> String {
    let mut hasher = Sha512::new();
    hasher.update(text);
    let hash = hasher.finalize();
    hex::encode(hash)
}

/// A magic link is used for password-less login. It has no
/// api version since exposing it via the api would be a pretty
/// serious security vulnerability.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MagicLink {
    pub id: api::MagicLinkId,
    pub email: String,
    pub created_at: DateTime,
}

impl MagicLink {
    pub fn new(email: String) -> Self {
        Self {
            id: api::MagicLinkId::new(Uuid::new_v4().to_string()),
            email,
            created_at: DateTime::now(),
        }
    }
}

impl From<MagicLink> for Bson {
    fn from(link: MagicLink) -> Bson {
        Bson::Document(doc! {
            "id": link.id,
            "email": link.email,
            "createdAt": link.created_at,
        })
    }
}

/// A Gallery allows the owner to retrieve project information of the members  
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Gallery {
    pub id: GalleryId,
    pub owner: String,
    pub name: String,
    pub state: api::PublishState,
}

impl Gallery {
    pub fn new(owner: String, name: String, state: api::PublishState) -> Self {
        Self {
            id: api::GalleryId::new(Uuid::new_v4().to_string()),
            name,
            owner,
            state,
        }
    }
}

impl From<Gallery> for netsblox_api_common::Gallery {
    fn from(gallery: Gallery) -> netsblox_api_common::Gallery {
        netsblox_api_common::Gallery {
            id: gallery.id,
            owner: gallery.owner,
            name: gallery.name,
            state: gallery.state,
        }
    }
}

impl From<Gallery> for Bson {
    fn from(gallery: Gallery) -> Self {
        Bson::Document(doc! {
            "id": gallery.id,
            "owner": gallery.owner,
            "name": gallery.name,
            "state": gallery.state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dont_schedule_deletion_for_saved_projects() {
        let metadata =
            ProjectMetadata::new("owner", "someProject", HashMap::new(), SaveState::Saved);
        assert!(metadata.delete_at.is_none());
    }

    #[test]
    fn test_schedule_deletion_for_created_projects() {
        // This gives them 10 minutes to be occupied before deletion
        let metadata =
            ProjectMetadata::new("owner", "someProject", HashMap::new(), SaveState::Created);
        assert!(metadata.delete_at.is_some());
    }

    #[test]
    fn test_pub_auth_host_to_host_preserves_cats() {
        let categories = vec!["cat1".into()];
        let auth_host = AuthorizedServiceHost {
            url: "http://localhost:8000".into(),
            id: "SomeTrustedHost".into(),
            secret: "SomeSecret".into(),
            visibility: ServiceHostScope::Public(categories.clone()),
        };
        let host: ServiceHost = auth_host.into();

        assert_eq!(host.categories.len(), 1);
        assert_eq!(&host.categories.into_iter().next().unwrap(), "cat1");
    }

    #[test]
    fn test_priv_auth_host_to_host_no_cats() {
        let auth_host = AuthorizedServiceHost {
            url: "http://localhost:8000".into(),
            id: "SomeTrustedHost".into(),
            secret: "SomeSecret".into(),
            visibility: ServiceHostScope::Private,
        };
        let host: ServiceHost = auth_host.into();
        assert_eq!(host.categories.len(), 0);
    }
}
