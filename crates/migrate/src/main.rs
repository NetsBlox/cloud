mod config;
mod origin;

use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, thread};

use crate::config::Config;
use clap::Parser;
use cloud::api::{PublishState, SaveState};
use derive_more::{Display, Error};
use futures::{future::join_all, stream::StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use mongodb::{
    bson::{doc, DateTime},
    options::UpdateOptions,
    Client, Database,
};
use netsblox_cloud_common as cloud;
use rusoto_core::{credential::StaticProvider, Region};
use rusoto_s3::{GetObjectRequest, PutObjectRequest, S3Client, S3};

#[derive(Debug)]
enum Migration {
    Libraries,
    Users,
    Projects,
    BannedAccounts,
}

#[derive(Debug, Display, Error)]
#[display(fmt = "Unable to parse user role. Expected admin, moderator, or user.")]
pub struct MigrationError;

impl FromStr for Migration {
    type Err = MigrationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "libraries" => Ok(Migration::Libraries),
            "users" => Ok(Migration::Users),
            "projects" => Ok(Migration::Projects),
            "banned-accounts" | "banned-accts" => Ok(Migration::BannedAccounts),
            _ => Err(MigrationError),
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// Path to configuration defining source, dst databases, s3, etc
    config_path: String,
    /// Only migrate users, projects, or libraries
    #[clap(long)]
    only: Option<Migration>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = Config::load(&args.config_path).unwrap();
    let src_db = connect_db(&config.source.database.url).await;
    let dst_db = connect_db(&config.target.database.url).await;

    if let Some(migration) = args.only {
        match migration {
            Migration::Users => migrate_users(&src_db, &dst_db).await,
            Migration::Libraries => migrate_libraries(&src_db, &dst_db).await,
            Migration::Projects => migrate_projects(&config, &src_db, &dst_db).await,
            Migration::BannedAccounts => migrate_banned_accts(&src_db, &dst_db).await,
        }
    } else {
        // migrate everything
        migrate_users(&src_db, &dst_db).await;
        migrate_libraries(&src_db, &dst_db).await;
        migrate_banned_accts(&src_db, &dst_db).await;
        migrate_projects(&config, &src_db, &dst_db).await;
    }
}

fn get_s3_client(config: &config::S3) -> S3Client {
    let region = Region::Custom {
        name: config.region_name.clone(),
        endpoint: config.endpoint.clone(),
    };
    S3Client::new_with(
        rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
        StaticProvider::new(
            config.credentials.access_key.clone(),
            config.credentials.secret_key.clone(),
            None,
            None,
        ),
        region,
    )
}

async fn connect_db(mongo_uri: &str) -> Database {
    Client::with_uri_str(mongo_uri)
        .await
        .expect("Could not connect to mongodb.")
        .default_database()
        .expect("Could not connect to default source database")
}

async fn download(
    client: &S3Client,
    bucket: &str,
    metadata: origin::ProjectMetadata,
) -> cloud::Project {
    let updated = metadata
        .last_update_at
        .map(|timestamp| DateTime::from_millis(timestamp as i64))
        .unwrap_or_else(DateTime::now);

    let state = metadata
        .public
        .map(|is_public| {
            if is_public {
                PublishState::Public
            } else {
                PublishState::Private
            }
        })
        .unwrap_or(PublishState::Private);

    let project_id = cloud::api::ProjectId::new(metadata.id.to_string());
    let owner = metadata.owner;
    let name = metadata.name;
    let collaborators = metadata.collaborators;
    let roles: HashMap<_, _> = join_all(
        metadata
            .roles
            .into_iter()
            .map(|(id, role)| download_role(client, bucket, id, role)),
    )
    .await
    .into_iter()
    .flatten()
    .collect();

    assert!(!roles.is_empty(), "{:?} has no roles!", project_id);
    cloud::Project {
        id: project_id,
        owner,
        name,
        collaborators,
        updated,
        state,
        origin_time: updated,
        save_state: SaveState::SAVED,
        roles,
    }
}

async fn download_role(
    client: &S3Client,
    bucket: &str,
    id: String,
    role_md: origin::RoleMetadata,
) -> Option<(cloud::api::RoleId, cloud::api::RoleData)> {
    if let (Some(code), Some(media), Some(name)) =
        (role_md.source_code, role_md.media, role_md.project_name)
    {
        let code = download_s3(client, bucket, &code).await;
        let media = download_s3(client, bucket, &media).await;

        let role = cloud::api::RoleData {
            name: name.to_owned(),
            code,
            media,
        };
        let role_id = cloud::api::RoleId::new(id.to_owned());

        Some((role_id, role))
    } else {
        None
    }
}

async fn upload(
    client: &S3Client,
    bucket: &str,
    project: cloud::Project,
) -> cloud::ProjectMetadata {
    let role_iter = project.roles.iter();
    let owner = project.owner;
    let name = project.name;
    let role_ids = role_iter.clone().map(|(k, _value)| k.to_owned());
    let role_data =
        join_all(role_iter.map(|(_id, data)| upload_role(client, bucket, &owner, &name, data)))
            .await
            .into_iter();
    let roles: HashMap<_, _> = role_ids.zip(role_data).into_iter().collect();

    cloud::ProjectMetadata {
        id: project.id,
        owner,
        name,
        collaborators: project.collaborators,
        updated: project.updated,
        origin_time: project.origin_time,
        state: project.state,
        save_state: project.save_state,
        delete_at: None,
        network_traces: Vec::new(),
        roles,
    }
}

async fn upload_role(
    client: &S3Client,
    bucket: &str,
    owner: &str,
    project_name: &str,
    role: &cloud::api::RoleData,
) -> cloud::RoleMetadata {
    let is_guest = owner.starts_with('_');
    let top_level = if is_guest { "guests" } else { "users" };
    let basepath = format!("{}/{}/{}/{}", top_level, owner, project_name, &role.name);
    let src_path = format!("{}/code.xml", &basepath);
    let media_path = format!("{}/media.xml", &basepath);

    upload_s3(client, bucket, &media_path, role.media.to_owned()).await;
    upload_s3(client, bucket, &src_path, role.code.to_owned()).await;

    cloud::RoleMetadata {
        name: role.name.to_owned(),
        code: src_path,
        media: media_path,
        updated: DateTime::now(),
    }
}

async fn upload_s3(client: &S3Client, bucket: &str, key: &str, body: String) {
    let request = PutObjectRequest {
        bucket: bucket.to_owned(),
        key: String::from(key),
        body: Some(String::into_bytes(body).into()),
        ..Default::default()
    };
    client.put_object(request).await.unwrap();
}

async fn download_s3(client: &S3Client, bucket: &str, key: &str) -> String {
    let request = GetObjectRequest {
        bucket: bucket.to_owned(),
        key: String::from(key),
        ..Default::default()
    };

    let output = client.get_object(request).await.unwrap();
    let byte_str = output
        .body
        .unwrap()
        .map_ok(|b| b.to_vec())
        .try_concat()
        .await
        .unwrap();

    String::from_utf8(byte_str).unwrap()
}

async fn migrate_users(src_db: &Database, dst_db: &Database) {
    let src_users = src_db.collection::<origin::User>("users");
    let dst_users = dst_db.collection::<cloud::User>("users");
    let count = src_users
        .estimated_document_count(None)
        .await
        .expect("Unable to estimate document count for users");
    let progress = ProgressBar::new(count);
    progress.println("Migrating users...");
    let mut cursor = src_users
        .find(None, None)
        .await
        .expect("Unable to fetch users");

    while let Some(user) = cursor.next().await {
        let new_user: cloud::User = user.expect("Unable to retrieve user").into();
        let query = doc! {"username": &new_user.username};
        let update = doc! {"$setOnInsert": &new_user};
        let opts = UpdateOptions::builder().upsert(true).build();
        dst_users
            .update_one(query, update, opts)
            .await
            .unwrap_or_else(|_err| panic!("Unable to update username: {}", &new_user.username));

        progress.inc(1);
    }
    progress.println("User migration complete.");
    progress.finish();

    // migrate groups
    let src_groups = src_db.collection::<origin::Group>("groups");
    let dst_groups = dst_db.collection::<cloud::Group>("groups");
    let count = src_groups
        .estimated_document_count(None)
        .await
        .expect("Unable to estimate document count for groups");
    let progress = ProgressBar::new(count);
    progress.println("Migrating groups...");

    let query = doc! {};
    let mut cursor = src_groups
        .find(query, None)
        .await
        .expect("Unable to fetch groups");

    while let Some(group) = cursor.next().await {
        let group = group.expect("Unable to retrieve group");
        if let Some(usernames) = group.members.clone() {
            for name in usernames {
                let query = doc! {"username": &name};
                let update = doc! {
                    "$set": {
                        "groupId": &group.id.to_string()
                    }
                };

                dst_users
                    .update_one(query, update, None)
                    .await
                    .unwrap_or_else(|_err| panic!("Unable to set group for {}", &name));
            }
        }

        let new_group: cloud::Group = group.into();
        let query = doc! {"id": &new_group.id};
        let update = doc! {"$setOnInsert": &new_group};
        dst_groups
            .update_one(query, update, None)
            .await
            .unwrap_or_else(|_err| panic!("Unable to update group: {}", &new_group.id));
        progress.inc(1);
    }
    progress.println("Group migration complete.");
    progress.finish();

    drop(src_groups);
    drop(dst_groups);
}

async fn migrate_libraries(src_db: &Database, dst_db: &Database) {
    let src_libraries = src_db.collection::<origin::Library>("libraries");
    let dst_libraries = dst_db.collection::<cloud::Library>("libraries");

    let count = src_libraries
        .estimated_document_count(None)
        .await
        .expect("Unable to estimate document count for libraries");
    let progress = ProgressBar::new(count);
    progress.println("Migrating libraries...");
    let query = doc! {};
    let mut cursor = src_libraries.find(query, None).await.unwrap();

    while let Some(library) = cursor.next().await {
        let library = library.unwrap();
        let new_lib: cloud::Library = library.into();
        let query = doc! {
            "owner": &new_lib.owner,
            "name": &new_lib.name
        };
        let update = doc! {"$setOnInsert": &new_lib};
        dst_libraries.update_one(query, update, None).await.unwrap();
        progress.inc(1);
    }
    progress.println("Library migration complete.");
    progress.finish();

    drop(src_libraries);
    drop(dst_libraries);
}

async fn migrate_banned_accts(src_db: &Database, dst_db: &Database) {
    let src_bans = src_db.collection::<origin::BannedAccount>("bannedAccounts");
    let dst_bans = dst_db.collection::<cloud::BannedAccount>("bannedAccounts");

    let count = src_bans
        .estimated_document_count(None)
        .await
        .expect("Unable to estimate document count for banned accounts");
    let progress = ProgressBar::new(count);
    progress.println("Migrating banned accounts...");
    let query = doc! {};
    let mut cursor = src_bans.find(query, None).await.unwrap();

    while let Some(account) = cursor.next().await {
        let account = account.unwrap();
        let new_acct: cloud::BannedAccount = account.into();
        let query = doc! {
            "username": &new_acct.username,
        };
        let update = doc! {"$setOnInsert": &new_acct};
        dst_bans.update_one(query, update, None).await.unwrap();

        progress.inc(1);
    }
    progress.println("Banned account migration complete.");
    progress.finish();
}

async fn migrate_projects(config: &Config, src_db: &Database, dst_db: &Database) {
    let src_projects = src_db.collection::<origin::ProjectMetadata>("projects");
    let dst_projects = dst_db.collection::<cloud::ProjectMetadata>("projects");
    let src_s3 = get_s3_client(&config.source.s3);
    let dst_s3 = get_s3_client(&config.target.s3);

    let query = doc! {"transient": false}; // FIXME: what if transient isn't set
    let count = src_projects
        .count_documents(query.clone(), None)
        .await
        .expect("Unable to estimate document count for banned accounts");
    let progress = ProgressBar::new(count);
    progress.println("Migrating projects...");

    let mut cursor = src_projects.find(query, None).await.unwrap();

    while let Some(metadata) = cursor.next().await {
        let metadata = metadata.unwrap();
        let query = doc! {
            "owner": &metadata.owner,
            "name": &metadata.name,
        };
        let exists = dst_projects.find_one(query, None).await.unwrap().is_some();
        if !exists {
            let project = download(&src_s3, &config.source.s3.bucket, metadata).await;
            let metadata = upload(&dst_s3, &config.target.s3.bucket, project).await;
            dst_projects.insert_one(&metadata, None).await.unwrap();
            // throttle to about 2k req/sec to avoid 503 errors from AWS
            thread::sleep(Duration::from_millis(10));
        }
        progress.inc(1);
    }
    progress.println("Project migration complete.");
    progress.finish();
}
