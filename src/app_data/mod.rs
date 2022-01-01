use futures::future::join_all;
use futures::{join, StreamExt};
use mongodb::bson::doc;
use std::collections::HashSet;

use crate::models::{CollaborationInvitation, Group, Project, ProjectMetadata, User};
use crate::models::{RoleData, RoleMetadata};
use crate::network::topology::Topology;
use actix::{Actor, Addr};
use futures::TryStreamExt;
use mongodb::options::FindOptions;
use mongodb::{Collection, Database};
use rusoto_s3::{
    CreateBucketOutput, CreateBucketRequest, GetObjectOutput, GetObjectRequest, PutObjectOutput,
    PutObjectRequest, S3Client, S3,
};

pub struct AppData {
    prefix: &'static str,
    bucket: String,
    s3: S3Client,
    pub db: Database,
    pub network: Addr<Topology>,
    pub groups: Collection<Group>,
    pub users: Collection<User>,
    pub project_metadata: Collection<ProjectMetadata>,
    pub collab_invites: Collection<CollaborationInvitation>,
}

impl AppData {
    pub fn new(
        db: Database,
        s3: S3Client,
        bucket: String,
        network: Option<Addr<Topology>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let network = network.unwrap_or(Topology::new().start());
        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        let project_metadata = db.collection::<ProjectMetadata>(&(prefix.to_owned() + "projects"));
        let collab_invites = db.collection::<CollaborationInvitation>(
            &(prefix.to_owned() + "collaborationInvitations"),
        );
        AppData {
            db,
            network,
            s3,
            bucket,
            groups,
            users,
            prefix,
            project_metadata,

            collab_invites,
        }
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
    ) -> ProjectMetadata {
        let query = doc! {"owner": &owner};
        let projection = doc! {"name": true};
        let options = FindOptions::builder().projection(projection).build();
        let cursor = self.project_metadata.find(query, options).await.unwrap();
        let project_names = cursor
            .try_collect::<Vec<ProjectMetadata>>()
            .await
            .unwrap()
            .iter()
            .map(|md| md.name.to_owned())
            .collect();

        let unique_name = get_unique_name(project_names, name);
        let roles = roles.unwrap_or(vec![RoleData {
            project_name: "myRole".to_owned(),
            source_code: "".to_owned(),
            media: "".to_owned(),
        }]);

        let role_mds = join_all(
            roles
                .iter()
                .map(|role| self.upload_role(&owner, &unique_name, role)),
        )
        .await;

        let metadata = ProjectMetadata::new(owner, name, role_mds);
        self.project_metadata
            .insert_one(metadata.clone(), None)
            .await
            .unwrap();
        metadata
    }

    async fn upload_role(&self, owner: &str, project_name: &str, role: &RoleData) -> RoleMetadata {
        let basepath = format!("users/{}/{}/{}", owner, project_name, &role.project_name);
        let src_path = format!("{}/source_code.xml", &basepath);
        let media_path = format!("{}/media.xml", owner);

        self.upload(&media_path, role.media.to_owned()).await;
        self.upload(&src_path, role.source_code.to_owned()).await;

        RoleMetadata {
            project_name: role.project_name.to_owned(),
            source_code: src_path,
            media: media_path,
        }
    }

    async fn upload(&self, key: &str, body: String) -> PutObjectOutput {
        let request = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            body: Some(String::into_bytes(body).into()),
            ..Default::default()
        };
        self.s3.put_object(request).await.unwrap()
    }

    async fn download(&self, key: &str) -> String {
        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            ..Default::default()
        };

        let output = self.s3.get_object(request).await.unwrap();
        let byte_str = output
            .body
            .unwrap()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await
            .unwrap();

        String::from_utf8(byte_str).unwrap()
    }

    pub async fn fetch_project(&self, metadata: &ProjectMetadata) -> Project {
        // TODO: populate the source code, media for each role
        todo!();
    }

    pub async fn delete_project(&self, metadata: ProjectMetadata) -> bool {
        todo!();
    }

    pub async fn fetch_role(&self, metadata: &RoleMetadata) -> RoleData {
        let (source_code, media) = join!(
            self.download(&metadata.source_code),
            self.download(&metadata.media),
        );
        RoleData {
            project_name: metadata.project_name.to_owned(),
            source_code,
            media,
        }
    }

    pub async fn save_role(
        &self,
        metadata: &ProjectMetadata,
        role_id: &str,
        source_code: &str,
        media: &str,
    ) -> bool {
        todo!();
    }

    pub async fn create_role(
        &self,
        metadata: ProjectMetadata,
        name: &str,
        source_code: Option<String>,
        media: Option<String>,
    ) -> Result<bool, std::io::Error> {
        // FIXME: incorrect signature
        //let role_id = Uuid::new_v4();

        // let role_names = metadata
        //     .roles
        //     .into_values()
        //     .map(|r| r.project_name)
        //     .collect::<HashSet<String>>();
        // let role_name = get_unique_name(&metadata, &body.name);
        todo!();
    }
}

fn get_unique_name(existing: Vec<String>, name: &str) -> String {
    let names: HashSet<std::string::String> = HashSet::from_iter(existing.iter().cloned());
    let mut base_name = name;
    let mut role_name = base_name.to_owned();
    let mut number: u8 = 2;
    while names.contains(&role_name) {
        role_name = format!("{} ({})", base_name, number);
        number += 1;
    }
    role_name
}
