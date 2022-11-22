use mongodb::bson::{self, doc, document::Document, Bson, DateTime};
pub use netsblox_api_common as api;
use netsblox_api_common::{
    oauth, ClientState, LibraryMetadata, NewUser, OccupantInviteData, PublishState, RoleId,
    UserRole,
};
use netsblox_api_common::{
    FriendInvite, FriendLinkState, GroupId, InvitationState, LinkedAccount, ProjectId, RoleData,
    SaveState, ServiceHost,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub hash: String,
    pub salt: String,
    pub group_id: Option<GroupId>,
    pub role: UserRole,
    pub created_at: DateTime,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
    pub service_settings: HashMap<String, String>,
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
            username: user_data.username.to_lowercase(),
            hash,
            salt,
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

#[derive(Serialize, Deserialize, Clone)]
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
            state: InvitationState::PENDING,
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

type FriendLinkId = String;
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FriendLink {
    pub id: FriendLinkId,
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
            state: state.unwrap_or(FriendLinkState::PENDING),
            created_at,
            updated_at: created_at,
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
        println!("from friend link! {:?}", link);
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

#[derive(Deserialize, Serialize, Clone)]
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
            start_time: DateTime::from_system_time(SystemTime::now()),
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

#[derive(Deserialize, Serialize, Clone)]
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
        roles: Vec<RoleMetadata>,
        save_state: SaveState,
    ) -> ProjectMetadata {
        let origin_time = DateTime::from_system_time(SystemTime::now());
        let roles = roles
            .into_iter()
            .map(|role| (RoleId::new(Uuid::new_v4().to_string()), role))
            .collect::<HashMap<_, _>>();

        let ten_minutes = Duration::new(10 * 60, 0);
        let delete_at =
            DateTime::from_system_time(SystemTime::now().checked_add(ten_minutes).unwrap());

        ProjectMetadata {
            id: ProjectId::new(Uuid::new_v4().to_string()),
            owner: owner.to_owned(),
            name: name.to_owned(),
            updated: origin_time,
            origin_time,
            state: PublishState::Private,
            collaborators: vec![],
            save_state,
            delete_at: Some(delete_at),
            network_traces: Vec::new(),
            roles,
        }
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
            roles: metadata
                .roles
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleMetadata {
    pub name: String,
    pub code: String,
    pub media: String,
    pub updated: DateTime,
}

impl From<RoleMetadata> for netsblox_api_common::RoleMetadata {
    fn from(metadata: RoleMetadata) -> netsblox_api_common::RoleMetadata {
        netsblox_api_common::RoleMetadata {
            name: metadata.name,
            code: metadata.code,
            media: metadata.media,
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
    pub fn new(project_id: ProjectId, req: OccupantInviteData) -> Self {
        OccupantInvite {
            project_id,
            username: req.username,
            role_id: req.role_id,
            created_at: DateTime::from_system_time(SystemTime::now()),
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

impl SentMessage {
    pub fn new(
        project_id: ProjectId,
        source: ClientState,
        recipients: Vec<ClientState>,
        content: serde_json::Value,
    ) -> Self {
        let time = DateTime::from_system_time(SystemTime::now());
        SentMessage {
            project_id,
            recipients,
            time,
            source,
            content,
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
    pub public: bool,
    pub secret: String,
}

impl AuthorizedServiceHost {
    pub fn new(url: String, id: String, public: bool) -> Self {
        let secret = Uuid::new_v4().to_string();
        AuthorizedServiceHost {
            url,
            id,
            public,
            secret,
        }
    }
}

impl From<AuthorizedServiceHost> for Bson {
    fn from(token: AuthorizedServiceHost) -> Bson {
        Bson::Document(doc! {
            "url": token.url,
            "id": token.id,
            "public": token.public,
            "secret": token.secret,
        })
    }
}

impl From<netsblox_api_common::AuthorizedServiceHost> for AuthorizedServiceHost {
    fn from(data: netsblox_api_common::AuthorizedServiceHost) -> AuthorizedServiceHost {
        AuthorizedServiceHost::new(data.url, data.id, data.public)
    }
}

impl From<AuthorizedServiceHost> for netsblox_api_common::AuthorizedServiceHost {
    fn from(host: AuthorizedServiceHost) -> netsblox_api_common::AuthorizedServiceHost {
        netsblox_api_common::AuthorizedServiceHost {
            id: host.id,
            url: host.url,
            public: host.public,
        }
    }
}

impl From<AuthorizedServiceHost> for netsblox_api_common::ServiceHost {
    fn from(host: AuthorizedServiceHost) -> netsblox_api_common::ServiceHost {
        netsblox_api_common::ServiceHost {
            url: host.url,
            categories: Vec::new(),
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

#[cfg(test)]
mod tests {
    #[actix_web::test]
    async fn test_uuid_ser() {}
}
