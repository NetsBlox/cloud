use mongodb::bson::{doc, Bson, DateTime};
pub use netsblox_core::{
    CreateGroupData, FriendInvite, FriendLinkState, Group, GroupId, InvitationState, LinkedAccount,
    ProjectId, RoleData, RoleMetadata, SaveState, ServiceHost,
};
use netsblox_core::{NewUser, OccupantInviteReq, UserRole};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

use crate::users::sha512;

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
        })
    }
}

impl From<User> for netsblox_core::User {
    fn from(user: User) -> netsblox_core::User {
        netsblox_core::User {
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
            .unwrap_or("salt".to_owned());

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
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub project_id: String,
    pub state: InvitationState,
    pub created_at: DateTime,
}

impl CollaborationInvite {
    pub fn new(sender: String, receiver: String, project_id: String) -> Self {
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

impl From<CollaborationInvite> for netsblox_core::CollaborationInvite {
    fn from(user: CollaborationInvite) -> netsblox_core::CollaborationInvite {
        netsblox_core::CollaborationInvite {
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
    pub delete_at: Option<DateTime>,
    pub roles: HashMap<String, RoleMetadata>,
}

impl ProjectMetadata {
    pub fn new(owner: &str, name: &str, roles: Vec<RoleMetadata>) -> ProjectMetadata {
        let origin_time = DateTime::from_system_time(SystemTime::now());
        let roles = roles
            .into_iter()
            .map(|role| (Uuid::new_v4().to_string(), role))
            .collect::<HashMap<_, _>>();

        let ten_minutes = Duration::new(10 * 60, 0);
        let delete_at =
            DateTime::from_system_time(SystemTime::now().checked_add(ten_minutes).unwrap());

        ProjectMetadata {
            id: Uuid::new_v4().to_string(),
            owner: owner.to_owned(),
            name: name.to_owned(),
            updated: origin_time,
            origin_time,
            thumbnail: "".to_owned(),
            public: false,
            collaborators: vec![],
            save_state: SaveState::CREATED,
            delete_at: Some(delete_at),
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

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OccupantInvite {
    pub username: String,
    pub project_id: ProjectId,
    pub role_id: String,
    created_at: DateTime,
}

impl OccupantInvite {
    pub fn new(project_id: ProjectId, req: OccupantInviteReq) -> Self {
        OccupantInvite {
            project_id,
            username: req.username,
            role_id: req.role_id,
            created_at: DateTime::from_system_time(SystemTime::now()),
        }
    }
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_uuid_ser() {}
}
