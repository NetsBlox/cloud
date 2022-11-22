use std::{collections::HashMap, time::SystemTime};

use cloud::api::PublishState;
use cloud::api::UserRole;
use mongodb::bson::DateTime;
use netsblox_cloud_common as cloud;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct User {
    username: String,
    email: String,
    hash: String,

    last_login_at: Option<u32>,
    group_id: Option<u32>,
    linked_accounts: Option<Vec<LinkedAccount>>,
    services_hosts: Option<Vec<ServiceHost>>,
    created_at: Option<DateTime>,
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
                .unwrap_or_else(|| DateTime::from_system_time(SystemTime::now())),
            role: UserRole::User,
            salt,
            hash: "None".to_owned(), // Password needs to be reset
            linked_accounts: user
                .linked_accounts
                .into_iter()
                .map(|acct| acct.into())
                .collect::<Vec<_>>(),
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
    r#type: String,
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
pub(crate) struct Project {
    name: String,
    owner: String,
    collaborators: Vec<String>,
    roles: HashMap<String, RoleData>,

    transient: Option<bool>,
    #[serde(rename = "camelCase")]
    delete_at: Option<DateTime>,
    #[serde(rename = "camelCase")]
    last_update_at: Option<u32>,
    #[serde(rename = "camelCase")]
    last_updated_at: Option<DateTime>,
    #[serde(rename = "PascalCase")]
    public: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RoleData {
    project_name: String,
    source_code: String,
    media: String,

    source_size: Option<u32>,
    media_size: Option<u32>,
    public: Option<bool>,
    thumbnail: Option<String>,
    notes: Option<String>,
    updated: Option<DateTime>,
}

// TODO: need to get the source_code, media from s3
#[derive(Deserialize)]
pub(crate) struct OAuthClient {
    owner: Option<String>,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthToken {
    client_id: String,
    username: String,
    created_at: DateTime,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Group {
    _id: String,
    name: String,
    owner: String,

    members: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BannedAccount {
    username: String,
    email: String,
    hash: String,

    last_login_at: Option<u32>,
    group_id: Option<u32>,
    linked_accounts: Option<Vec<LinkedAccount>>,
    services_hosts: Option<Vec<ServiceHost>>,
    created_at: Option<DateTime>,
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
