use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime};
pub use netsblox_core::{
    FriendInvite, FriendLinkState, LinkedAccount, ProjectId, RoleMetadata, SaveState, ServiceHost,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::SystemTime};
use uuid::Uuid;

pub type GroupId = String;
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub hash: String,
    pub group_id: Option<GroupId>,
    pub admin: Option<bool>, // TODO: use roles instead? What other roles would we even have?
    pub created_at: u32,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
}
// TODO: implement Responder (omit the hash)

impl From<User> for Bson {
    fn from(user: User) -> Bson {
        Bson::Document(doc! {
            "username": user.username,
            "email": user.email,
            "hash": user.hash,
            "groupId": user.group_id,
            "admin": user.admin,
            "createdAt": user.created_at,
            "linkedAccounts": user.linked_accounts,
            "servicesHosts": user.services_hosts,
        })
    }
}

impl From<User> for netsblox_core::User {
    fn from(user: User) -> netsblox_core::User {
        netsblox_core::User {
            username: user.username,
            email: user.email,
            group_id: user.group_id,
            admin: user.admin,
            created_at: user.created_at,
            linked_accounts: user.linked_accounts,
            services_hosts: user.services_hosts,
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
pub struct ProjectMetadata {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: DateTime,
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime,
    pub save_state: SaveState,
    pub roles: HashMap<String, RoleMetadata>,
}

impl ProjectMetadata {
    pub fn new(owner: &str, name: &str, roles: Vec<RoleMetadata>) -> ProjectMetadata {
        let origin_time = DateTime::from_system_time(SystemTime::now());
        let roles = roles
            .into_iter()
            .map(|role| (Uuid::new_v4().to_string(), role))
            .collect::<HashMap<_, _>>();

        ProjectMetadata {
            id: Uuid::new_v4().to_string(),
            owner: owner.to_owned(),
            name: name.to_owned(),
            updated: origin_time,
            origin_time,
            thumbnail: "".to_owned(),
            public: false,
            collaborators: vec![],
            save_state: SaveState::TRANSIENT,
            roles,
        }
    }
}

impl From<ProjectMetadata> for netsblox_core::ProjectMetadata {
    fn from(metadata: ProjectMetadata) -> netsblox_core::ProjectMetadata {
        netsblox_core::ProjectMetadata {
            id: metadata.id,
            owner: metadata.owner,
            name: metadata.name,
            origin_time: metadata.origin_time.to_system_time(),
            updated: metadata.updated.to_system_time(),
            thumbnail: metadata.thumbnail,
            public: metadata.public,
            collaborators: metadata.collaborators,
            save_state: metadata.save_state,
            roles: metadata.roles,
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
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime,
    pub save_state: SaveState,
    pub roles: HashMap<String, RoleData>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct RoleData {
    pub project_name: String,
    pub source_code: String,
    pub media: String,
}

impl RoleData {
    pub fn to_xml(self) -> String {
        format!(
            "<role name=\"{}\">{}{}</role>",
            self.project_name, self.source_code, self.media
        ) // TODO: escape the names?
    }

    pub fn to_project_xml(name: &str, roles: Vec<RoleData>) -> String {
        let APP_NAME = "NetsBlox";
        let role_str: String = roles
            .into_iter()
            .map(|role| role.to_xml())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<room name=\"{}\" app=\"{}\">{}</room>",
            name, APP_NAME, role_str
        )
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub owner: String,
    pub name: String,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum InvitationState {
    PENDING,
    ACCEPTED,
    REJECTED,
}

impl From<InvitationState> for Bson {
    fn from(state: InvitationState) -> Bson {
        match state {
            InvitationState::PENDING => Bson::String("PENDING".to_owned()),
            InvitationState::ACCEPTED => Bson::String("ACCEPTED".to_owned()),
            InvitationState::REJECTED => Bson::String("REJECTED".to_owned()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvitation {
    pub _id: Option<ObjectId>,
    pub sender: String,
    pub receiver: String,
    pub project_id: ObjectId,
    pub state: InvitationState,
    pub created_at: DateTime,
}

impl CollaborationInvitation {
    pub fn new(sender: String, receiver: String, project_id: ObjectId) -> CollaborationInvitation {
        let created_at = DateTime::from_system_time(SystemTime::now());
        CollaborationInvitation {
            _id: None,
            sender,
            receiver,
            project_id,
            state: InvitationState::PENDING,
            created_at,
        }
    }
}
impl From<CollaborationInvitation> for Bson {
    fn from(invite: CollaborationInvitation) -> Self {
        let doc = Bson::Document(doc! {
            "sender": invite.sender,
            "receiver": invite.receiver,
            "projectId": invite.project_id,
            "state": invite.state,
            "created_at": invite.created_at,
        });
        // TODO: add _id
        // if let Some(id) = self._id {
        //     doc.as_document().unwrap().insert

        // }
        doc
    }
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_uuid_ser() {}
}
