use std::{collections::HashMap, time::SystemTime};

use cloud::api::PublishState;
use cloud::api::UserRole;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::DateTime;
use netsblox_cloud_common as cloud;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct User {
    username: String,
    email: String,
    //hash: String,

    //last_login_at: Option<f32>,
    group_id: Option<ObjectId>,
    linked_accounts: Option<Vec<LinkedAccount>>,
    services_hosts: Option<Vec<ServiceHost>>,
    created_at: Option<f32>,
}

impl From<User> for cloud::User {
    fn from(user: User) -> cloud::User {
        let salt = passwords::PasswordGenerator::new()
            .length(8)
            .exclude_similar_characters(true)
            .numbers(true)
            .spaces(false)
            .generate_one()
            .expect("Unable to generate salt");

        cloud::User {
            username: user.username,
            email: user.email,
            group_id: user
                .group_id
                .map(|id| cloud::api::GroupId::new(id.to_string())),
            created_at: user
                .created_at
                .map(|unix_ts| DateTime::from_millis(unix_ts as i64))
                .unwrap_or_else(|| DateTime::from_system_time(SystemTime::now())),
            role: UserRole::User,
            salt,
            hash: "None".to_owned(), // Password needs to be reset
            linked_accounts: user
                .linked_accounts
                .map(|accounts| {
                    accounts
                        .into_iter()
                        .map(|acct| acct.into())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(Vec::new),
            services_hosts: user.services_hosts.map(|hosts| {
                hosts
                    .into_iter()
                    .map(|acct| acct.into())
                    .collect::<Vec<_>>()
            }),
            service_settings: HashMap::new(),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct LinkedAccount {
    username: String,
    //r#type: String,
}

impl From<LinkedAccount> for cloud::api::LinkedAccount {
    fn from(acct: LinkedAccount) -> cloud::api::LinkedAccount {
        let strategy = String::from("snap");
        cloud::api::LinkedAccount {
            username: acct.username,
            strategy,
        }
    }
}

// TODO: clear off any references to textanalysis.netsblox.org?
#[derive(Deserialize)]
pub(crate) struct ServiceHost {
    url: String,
    categories: Vec<String>,
}

impl From<ServiceHost> for cloud::api::ServiceHost {
    fn from(host: ServiceHost) -> cloud::api::ServiceHost {
        cloud::api::ServiceHost {
            url: host.url,
            categories: host.categories,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Library {
    name: String,
    owner: String,
    blocks: String,
    notes: String,
    public: Option<bool>,
    needs_approval: Option<bool>,
}

impl From<Library> for cloud::Library {
    fn from(lib: Library) -> cloud::Library {
        let needs_approval = lib.needs_approval.unwrap_or(false);
        let public = lib.public.unwrap_or(false);
        let state = if needs_approval {
            PublishState::PendingApproval
        } else if public {
            PublishState::Public
        } else {
            PublishState::Private
        };

        cloud::Library {
            name: lib.name,
            owner: lib.owner,
            blocks: lib.blocks,
            notes: lib.notes,
            state,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ProjectMetadata {
    #[serde(rename = "_id")]
    pub(crate) id: ObjectId,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) collaborators: Vec<String>,
    pub(crate) roles: HashMap<String, RoleMetadata>,

    //pub(crate) transient: Option<bool>,
    //#[serde(rename = "camelCase")]
    //pub(crate) delete_at: Option<DateTime>,
    #[serde(rename = "camelCase")]
    pub(crate) last_update_at: Option<f32>,
    //#[serde(rename = "camelCase")]
    //pub(crate) last_updated_at: Option<DateTime>,
    #[serde(rename = "PascalCase")]
    pub(crate) public: Option<bool>,
}

impl ProjectMetadata {
    pub(crate) fn state(&self) -> PublishState {
        self.public
            .map(|is_public| {
                if is_public {
                    PublishState::Public
                } else {
                    PublishState::Private
                }
            })
            .unwrap_or(PublishState::Private)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RoleMetadata {
    pub(crate) project_name: Option<String>,
    pub(crate) source_code: Option<String>,
    pub(crate) media: Option<String>,
    //source_size: Option<u32>,
    //media_size: Option<u32>,
    //public: Option<bool>,
    //thumbnail: Option<String>,
    //notes: Option<String>,
    //updated: Option<DateTime>,
}

// #[derive(Deserialize)]
// pub(crate) struct OAuthClient {
//     owner: Option<String>,
//     name: String,
// }

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub(crate) struct OAuthToken {
//     client_id: String,
//     username: String,
//     created_at: DateTime,
// }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Group {
    #[serde(rename = "_id")]
    pub(crate) id: ObjectId,
    name: String,
    owner: Option<String>,

    pub(crate) members: Option<Vec<String>>,
}

impl From<Group> for cloud::Group {
    fn from(group: Group) -> cloud::Group {
        cloud::Group {
            id: cloud::api::GroupId::new(group.id.to_string()),
            name: group.name,
            owner: group.owner.unwrap_or_else(|| String::from("admin")), // old groups are transferred to the admin account
            service_settings: HashMap::new(),
            services_hosts: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BannedAccount {
    pub username: String,
    pub email: String,
    //hash: String,

    //last_login_at: Option<f32>,
    //group_id: Option<ObjectId>,
    //linked_accounts: Option<Vec<LinkedAccount>>,
    //services_hosts: Option<Vec<ServiceHost>>,
    //created_at: Option<DateTime>,
    banned_at: DateTime,
}

impl From<BannedAccount> for cloud::BannedAccount {
    fn from(acct: BannedAccount) -> cloud::BannedAccount {
        cloud::BannedAccount {
            username: acct.username,
            email: acct.email,
            banned_at: acct.banned_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_group() {
        let group_str = "{\"_id\": \"599aed7fc4913219dca051d2\", \"name\": \"test_group\"}";
        let _group: Group = serde_json::from_str(group_str)
            .unwrap_or_else(|_err| panic!("Unable to parse group from {}", group_str));
    }
}
