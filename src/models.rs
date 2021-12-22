use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    //pub friends: Option<Vec<String>>,
}

impl Into<Bson> for User {
    fn into(self) -> Bson {
        Bson::Document(doc! {
            "username": self.username,
            "email": self.email,
            "hash": self.hash,
            "groupId": self.group_id,
            "createdAt": self.created_at,
            "linkedAccounts": Into::<Bson>::into(self.linked_accounts)
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct ProjectMetadata {
    pub _id: ObjectId,
    pub owner: String,
    pub name: String,
    pub updated: DateTime,
    pub thumbnail: String,
    pub public: bool,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: DateTime, // FIXME: set the case
    pub roles: HashMap<String, RoleMetadata>,
}

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleMetadata {
    project_name: String,
    source_code: String,
    media: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RoleData {
    project_name: String,
    source_code: String,
    media: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LinkedAccount {
    pub username: String,
    pub strategy: String, // TODO: migrate type -> strategy
}

impl Into<Bson> for LinkedAccount {
    fn into(self) -> Bson {
        Bson::Document(doc! {
            "username": self.username,
            "strategy": self.strategy,
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
    url: String,
    categories: Vec<String>,
}

impl Into<Bson> for ServiceHost {
    fn into(self) -> Bson {
        Bson::Document(doc! {
            "url": self.url,
            "categories": Into::<Bson>::into(self.categories)
        })
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationInvite {
    pub sender: String,
    pub receiver: String,
    pub project: String,
    // TODO: role?
}
