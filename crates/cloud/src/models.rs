use mongodb::bson::{doc, Bson, DateTime};
pub use netsblox_core::{
    CollaborationInvite, CreateGroupData, FriendInvite, FriendLinkState, Group, GroupId,
    InvitationState, LinkedAccount, ProjectId, RoleData, RoleMetadata, SaveState, ServiceHost,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::SystemTime};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub hash: String,
    pub salt: String,
    pub group_id: Option<GroupId>,
    pub admin: Option<bool>, // TODO: use roles instead? What other roles would we even have?
    pub created_at: u32,
    pub linked_accounts: Vec<LinkedAccount>,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

impl From<User> for Bson {
    fn from(user: User) -> Bson {
        Bson::Document(doc! {
            "username": user.username,
            "email": user.email,
            "hash": user.hash,
            "salt": user.salt,
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

impl From<Project> for netsblox_core::Project {
    fn from(project: Project) -> netsblox_core::Project {
        netsblox_core::Project {
            id: project.id,
            owner: project.owner,
            name: project.name,
            origin_time: project.origin_time.to_system_time(),
            updated: project.updated.to_system_time(),
            thumbnail: project.thumbnail,
            public: project.public,
            collaborators: project.collaborators,
            save_state: project.save_state,
            roles: project.roles,
        }
    }
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_uuid_ser() {}
}
