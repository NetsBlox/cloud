use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::SystemTime};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub email: String,
    pub hash: String,
    pub group_id: Option<ObjectId>,
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
            "createdAt": user.created_at,
            "linkedAccounts": user.linked_accounts,
        })
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub id: ObjectId,
    pub owner: String,
    pub name: String,
    pub updated: DateTime,
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime, // FIXME: set the case
    pub roles: HashMap<String, RoleMetadata>,
}

impl ProjectMetadata {
    pub fn new(owner: &str, name: &str, roles: Vec<RoleMetadata>) -> ProjectMetadata {
        let origin_time = DateTime::from_system_time(SystemTime::now());
        let roles = roles
            .into_iter()
            .map(|role| (Uuid::new_v4().to_string(), role))
            .collect::<HashMap<String, RoleMetadata>>();

        ProjectMetadata {
            id: ObjectId::new(),
            owner: owner.to_owned(),
            name: name.to_owned(),
            updated: origin_time,
            origin_time,
            thumbnail: "".to_owned(),
            public: false,
            collaborators: vec![],
            roles,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    _id: ObjectId,
    owner: String,
    name: String,
    updated: DateTime,
    thumbnail: String,
    public: bool,
    collaborators: std::vec::Vec<String>,
    origin_time: DateTime, // FIXME: set the case
    roles: HashMap<String, RoleData>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct RoleMetadata {
    pub project_name: String,
    pub source_code: String,
    pub media: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleData {
    pub project_name: String,
    pub source_code: String,
    pub media: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LinkedAccount {
    pub username: String,
    pub strategy: String, // TODO: migrate type -> strategy
}

impl From<LinkedAccount> for Bson {
    fn from(account: LinkedAccount) -> Bson {
        Bson::Document(doc! {
            "username": account.username,
            "strategy": account.strategy,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub _id: ObjectId,
    pub owner: String,
    pub name: String,
    pub services_hosts: Option<Vec<ServiceHost>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHost {
    pub url: String,
    pub categories: Vec<String>,
}

impl From<ServiceHost> for Bson {
    fn from(host: ServiceHost) -> Bson {
        Bson::Document(doc! {
            "url": host.url,
            "categories": host.categories
        })
    }
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

#[derive(Deserialize, Serialize)]
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
