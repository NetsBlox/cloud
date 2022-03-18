use actix_web::rt::time;
use actix_web::HttpRequest;
use futures::future::join_all;
use futures::join;
use mongodb::bson::{doc, Document};
use mongodb::options::{FindOneAndUpdateOptions, IndexOptions, ReturnDocument};
use rusoto_core::credential::StaticProvider;
use rusoto_core::Region;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use uuid::Uuid;

use crate::config::Settings;
use crate::errors::{InternalError, UserError};
use crate::models::{
    CollaborationInvite, FriendLink, Group, Project, ProjectMetadata, SaveState, User,
};
use crate::models::{OccupantInvite, RoleData, RoleMetadata};
use crate::network::topology::{self, SetStorage, TopologyActor};
use actix::{Actor, Addr};
use futures::TryStreamExt;
use mongodb::{Client, Collection, Database, IndexModel};
use rusoto_s3::{
    CreateBucketRequest, DeleteObjectOutput, DeleteObjectRequest, GetObjectRequest,
    PutObjectOutput, PutObjectRequest, S3Client, S3,
};

#[derive(Clone)]
pub struct AppData {
    prefix: &'static str,
    bucket: String,
    tor_exit_nodes: Collection<TorNode>,
    s3: S3Client,
    pub settings: Settings,
    db: Database,
    pub network: Addr<TopologyActor>,
    pub groups: Collection<Group>,
    pub users: Collection<User>,
    pub friends: Collection<FriendLink>,
    pub project_metadata: Collection<ProjectMetadata>,
    pub collab_invites: Collection<CollaborationInvite>,
    pub occupant_invites: Collection<OccupantInvite>,
}

impl AppData {
    pub fn new(
        client: Client,
        settings: Settings,
        network: Option<Addr<TopologyActor>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let db = client.database(&settings.database.name);
        let region = Region::Custom {
            name: settings.s3.region_name.clone(),
            endpoint: settings.s3.endpoint.clone(),
        };
        let s3 = S3Client::new_with(
            rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
            StaticProvider::new(
                settings.s3.credentials.access_key.clone(),
                settings.s3.credentials.secret_key.clone(),
                None,
                None,
            ),
            //StaticProvider::from(AwsCredentials::default()),
            region,
        );

        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        let project_metadata = db.collection::<ProjectMetadata>(&(prefix.to_owned() + "projects"));

        let collab_invites =
            db.collection::<CollaborationInvite>(&(prefix.to_owned() + "collaborationInvitations"));
        let occupant_invites =
            db.collection::<OccupantInvite>(&(prefix.to_owned() + "occupantInvites"));
        let friends = db.collection::<FriendLink>(&(prefix.to_owned() + "friends"));
        let network = network.unwrap_or_else(|| TopologyActor {}.start());
        let tor_exit_nodes = db.collection::<TorNode>(&(prefix.to_owned() + "torExitNodes"));
        network.do_send(SetStorage {
            project_metadata: project_metadata.clone(),
        });
        let bucket = settings.s3.bucket.clone();
        AppData {
            settings,
            db,
            network,
            s3,
            bucket,
            groups,
            users,
            prefix,
            project_metadata,

            collab_invites,
            occupant_invites,
            friends,
            tor_exit_nodes,
        }
    }

    pub async fn initialize(&self) {
        // Create the s3 bucket
        let bucket = &self.settings.s3.bucket;
        let request = CreateBucketRequest {
            bucket: bucket.clone(),
            ..Default::default()
        };
        self.s3.create_bucket(request).await;

        self.db
            .run_command(
                doc! {
                    "createIndexes": &(self.prefix.to_owned() + "projects"),
                    "indexes": [
                        {
                            "key": {"deleteAt": 1},
                            "name": "broken_project_ttl",
                            "sparse": true,
                            "expireAfterSeconds": 10,
                        }
                    ],
                },
                None,
            )
            .await
            .unwrap();

        let index_opts = IndexOptions::builder()
            .expire_after(Duration::from_secs(60 * 60))
            .build();
        let invite_index = IndexModel::builder()
            .keys(doc! {"createdAt": 1})
            .options(index_opts)
            .build();
        self.occupant_invites
            .create_index(invite_index, None)
            .await
            .unwrap();

        self.occupant_invites
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"project_id": 1, "role_id": 1})
                    .build(),
                None,
            )
            .await
            .unwrap();

        self.project_metadata
            .create_index(IndexModel::builder().keys(doc! {"id": 1}).build(), None)
            .await
            .unwrap();

        self.tor_exit_nodes
            .create_index(IndexModel::builder().keys(doc! {"addr": 1}).build(), None)
            .await
            .unwrap();

        self.start_update_interval();
    }

    fn start_update_interval(&self) {
        let tor_exit_nodes = self.tor_exit_nodes.clone();
        actix_web::rt::spawn(async move {
            let one_day = Duration::from_secs(60 * 60 * 24);
            let mut interval = time::interval(one_day);
            loop {
                update_tor_nodes(&tor_exit_nodes).await; // TODO: add logging
                interval.tick().await;
            }
        });
    }

    pub fn collection<T>(&self, name: &str) -> Collection<T> {
        let name = &(self.prefix.to_owned() + name);
        self.db.collection::<T>(name)
    }

    pub async fn import_project(
        &self,
        owner: &str,
        name: &str,
        roles: Option<Vec<RoleData>>,
    ) -> Result<ProjectMetadata, UserError> {
        let query = doc! {"owner": &owner};
        // FIXME: Update the type if we use the projection
        //let projection = doc! {"name": true};
        //let options = FindOptions::builder().projection(projection).build();
        // TODO: validate the project name (no profanity)
        let cursor = self.project_metadata.find(query, None).await.unwrap(); // FIXME: This can throw an error
        let project_names = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_err| InternalError::DatabaseConnectionError)?
            .iter()
            .map(|md| md.name.to_owned())
            .collect();

        let unique_name = get_unique_name(project_names, name);
        let roles = roles.unwrap_or_else(|| {
            vec![RoleData {
                name: "myRole".to_owned(),
                code: "".to_owned(),
                media: "".to_owned(),
            }]
        });

        let role_mds: Vec<RoleMetadata> = join_all(
            roles
                .iter()
                .map(|role| self.upload_role(owner, &unique_name, role)),
        )
        .await
        .into_iter()
        .map(|res| res.unwrap())
        .collect();

        let metadata = ProjectMetadata::new(owner, &unique_name, role_mds);
        self.project_metadata
            .insert_one(metadata.clone(), None)
            .await
            .map_err(|_err| InternalError::DatabaseConnectionError)?;

        Ok(metadata)
    }

    async fn upload_role(
        &self,
        owner: &str,
        project_name: &str,
        role: &RoleData,
    ) -> Result<RoleMetadata, UserError> {
        let is_guest = owner.starts_with('_');
        let top_level = if is_guest { "guests" } else { "users" };
        let basepath = format!("{}/{}/{}/{}", top_level, owner, project_name, &role.name);
        let src_path = format!("{}/code.xml", &basepath);
        let media_path = format!("{}/media.xml", &basepath);

        self.upload(&media_path, role.media.to_owned()).await?;
        self.upload(&src_path, role.code.to_owned()).await?;

        Ok(RoleMetadata {
            name: role.name.to_owned(),
            code: src_path,
            media: media_path,
        })
    }

    async fn delete(&self, key: &str) -> DeleteObjectOutput {
        let request = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            ..Default::default()
        };
        self.s3.delete_object(request).await.unwrap()
    }

    async fn upload(&self, key: &str, body: String) -> Result<PutObjectOutput, InternalError> {
        let request = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            body: Some(String::into_bytes(body).into()),
            ..Default::default()
        };
        self.s3
            .put_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)
    }

    async fn download(&self, key: &str) -> Result<String, InternalError> {
        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            ..Default::default()
        };

        let output = self
            .s3
            .get_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)?;
        let byte_str = output
            .body
            .unwrap()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await
            .map_err(|_err| InternalError::S3ContentError)?;

        Ok(String::from_utf8(byte_str).map_err(|_err| InternalError::S3ContentError)?)
    }

    pub async fn fetch_project(&self, metadata: &ProjectMetadata) -> Result<Project, UserError> {
        let (keys, values): (Vec<_>, Vec<_>) = metadata.roles.clone().into_iter().unzip();
        // TODO: make fetch_role fallible
        let role_data = join_all(values.iter().map(|v| self.fetch_role(v))).await;

        let roles = keys
            .into_iter()
            .zip(role_data)
            .filter_map(|(k, data)| data.map(|d| (k, d)).ok())
            .collect::<HashMap<_, _>>();

        Ok(Project {
            // TODO: refactor?
            id: metadata.id.to_owned(),
            name: metadata.name.to_owned(),
            owner: metadata.owner.to_owned(),
            updated: metadata.updated.to_owned(),
            thumbnail: metadata.thumbnail.to_owned(),
            public: metadata.public.to_owned(),
            collaborators: metadata.collaborators.to_owned(),
            origin_time: metadata.origin_time,
            save_state: metadata.save_state.to_owned(),
            roles,
        })
    }

    pub async fn delete_project(&self, metadata: ProjectMetadata) -> Result<(), UserError> {
        let query = doc! {"id": &metadata.id};
        let result = self
            .project_metadata
            .find_one_and_delete(query, None)
            .await
            .unwrap();

        if let Some(metadata) = result {
            let paths: Vec<_> = metadata
                .roles
                .into_values()
                .flat_map(|role| vec![role.code, role.media])
                .collect();

            for path in paths {
                self.delete(&path).await;
            }
            // TODO: send update to any current occupants
            Ok(())
        } else {
            Err(UserError::ProjectNotFoundError)
        }
    }

    pub async fn fetch_role(&self, metadata: &RoleMetadata) -> Result<RoleData, InternalError> {
        let (code, media) = join!(
            self.download(&metadata.code),
            self.download(&metadata.media),
        );
        Ok(RoleData {
            name: metadata.name.to_owned(),
            code: code?,
            media: media?,
        })
    }

    pub async fn save_role(
        &self,
        metadata: &ProjectMetadata,
        role_id: &str,
        role: RoleData,
    ) -> Result<RoleMetadata, UserError> {
        let role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role)
            .await?;
        let query = doc! {"id": &metadata.id};
        let update =
            doc! {"$set": {&format!("roles.{}", role_id): role_md, "saveState": SaveState::SAVED}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(|_err| InternalError::DatabaseConnectionError)?
            .ok_or_else(|| UserError::ProjectNotFoundError)?;

        self.network.do_send(topology::SendRoomState {
            project: updated_metadata.clone(),
        });

        Ok(updated_metadata
            .roles
            .get(role_id)
            .map(|md| md.to_owned())
            .unwrap())
    }

    pub async fn create_role(
        &self,
        metadata: ProjectMetadata,
        role_data: RoleData,
    ) -> Result<ProjectMetadata, UserError> {
        let mut role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role_data)
            .await?;

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let role_names = metadata
            .roles
            .into_values()
            .map(|r| r.name)
            .collect::<Vec<_>>();
        let role_name = get_unique_name(role_names, &role_md.name);
        role_md.name = role_name;

        let role_id = Uuid::new_v4();
        let query = doc! {"id": metadata.id};
        let update = doc! {"$set": {&format!("roles.{}", role_id): role_md}};
        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .unwrap()
            .expect("Project not found.");

        self.network.do_send(topology::SendRoomState {
            project: updated_metadata.clone(),
        });
        Ok(updated_metadata)
    }

    pub async fn ensure_not_tor_ip(&self, req: HttpRequest) -> Result<(), UserError> {
        match req.peer_addr().map(|addr| addr.ip()) {
            Some(addr) => {
                let addr = addr.to_string();
                println!("checking if {} is exit node", addr);
                let query = doc! {"addr": addr};
                let node = self
                    .tor_exit_nodes
                    .find_one(query, None)
                    .await
                    .map_err(|_err| InternalError::DatabaseConnectionError)?;

                if node.is_some() {
                    return Err(UserError::TorAddressError);
                } else {
                    Ok(())
                }
            }
            None => Ok(()),
        }
    }
}

async fn update_tor_nodes(tor_exit_nodes: &Collection<TorNode>) -> Result<(), UserError> {
    let url = "https://check.torproject.org/torbulkexitlist";
    let response = reqwest::get(url)
        .await
        .map_err(|_err| InternalError::TorNodeListFetchError)?;

    let node_list: Vec<TorNode> = response
        .text()
        .await
        .map_err(|_err| UserError::InternalError)?
        .split_ascii_whitespace()
        .map(|addr| TorNode {
            addr: addr.to_string(),
        })
        .collect();

    tor_exit_nodes
        .delete_many(doc! {}, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

    tor_exit_nodes
        .insert_many(node_list, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct TorNode {
    addr: String,
}

fn get_unique_name(existing: Vec<String>, name: &str) -> String {
    let names: HashSet<std::string::String> = HashSet::from_iter(existing.iter().cloned());
    let base_name = name;
    let mut role_name = base_name.to_owned();
    let mut number: u8 = 2;
    while names.contains(&role_name) {
        role_name = format!("{} ({})", base_name, number);
        number += 1;
    }
    role_name
}

#[cfg(test)]
mod tests {
    #[actix_web::test]
    async fn test_save_role_blob() {
        todo!();
    }

    #[actix_web::test]
    async fn test_save_role_set_transient_false() {
        todo!();
    }
}
