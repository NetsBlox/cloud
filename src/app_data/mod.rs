use futures::future::join_all;
use futures::join;
use mongodb::bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::config::Settings;
use crate::models::{CollaborationInvitation, Group, Project, ProjectMetadata, User};
use crate::models::{RoleData, RoleMetadata};
use crate::network::topology::{SetStorage, TopologyActor};
use actix::{Actor, Addr};
use futures::TryStreamExt;
use mongodb::{Collection, Database};
use rusoto_s3::{GetObjectRequest, PutObjectOutput, PutObjectRequest, S3Client, S3};

pub struct AppData {
    prefix: &'static str,
    bucket: String,
    s3: S3Client,
    pub settings: Settings,
    pub db: Database,
    pub network: Addr<TopologyActor>,
    pub groups: Collection<Group>,
    pub users: Collection<User>,
    pub project_metadata: Collection<ProjectMetadata>,
    pub collab_invites: Collection<CollaborationInvitation>,
}

impl AppData {
    pub fn new(
        settings: Settings,
        db: Database,
        s3: S3Client,
        bucket: String,
        network: Option<Addr<TopologyActor>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        let project_metadata = db.collection::<ProjectMetadata>(&(prefix.to_owned() + "projects"));
        let collab_invites = db.collection::<CollaborationInvitation>(
            &(prefix.to_owned() + "collaborationInvitations"),
        );
        let network = network.unwrap_or_else(|| TopologyActor {}.start());
        network.do_send(SetStorage {
            project_metadata: project_metadata.clone(),
        });
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
        // FIXME: Update the type if we use the projection
        //let projection = doc! {"name": true};
        //let options = FindOptions::builder().projection(projection).build();
        let cursor = self.project_metadata.find(query, None).await.unwrap();
        let project_names = cursor
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .iter()
            .map(|md| md.name.to_owned())
            .collect();

        let unique_name = get_unique_name(project_names, name);
        let roles = roles.unwrap_or_else(|| vec![RoleData {
            project_name: "myRole".to_owned(),
            source_code: "".to_owned(),
            media: "".to_owned(),
        }]);

        let role_mds = join_all(
            roles
                .iter()
                .map(|role| self.upload_role(owner, &unique_name, role)),
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
        let is_guest = owner.starts_with('_');
        let top_level = if is_guest { "guests" } else { "users" };
        let basepath = format!(
            "{}/{}/{}/{}",
            top_level, owner, project_name, &role.project_name
        );
        let src_path = format!("{}/source_code.xml", &basepath);
        let media_path = format!("{}/media.xml", &basepath);

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
        let (keys, values): (Vec<_>, Vec<_>) = metadata.roles.clone().into_iter().unzip();
        let role_data = join_all(values.iter().map(|v| self.fetch_role(v))).await;

        let roles = keys
            .into_iter()
            .zip(role_data)
            .collect::<HashMap<_, _>>();

        Project {
            id: metadata.id,
            name: metadata.name.to_owned(),
            owner: metadata.owner.to_owned(),
            updated: metadata.updated.to_owned(),
            thumbnail: metadata.thumbnail.to_owned(),
            public: metadata.public.to_owned(),
            collaborators: metadata.collaborators.to_owned(),
            origin_time: metadata.origin_time,
            roles,
        }
    }

    pub async fn delete_project(&self, metadata: ProjectMetadata) -> bool {
        // TODO: send update to any current occupants
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
        role: RoleData,
    ) -> Option<RoleMetadata> {
        let role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role)
            .await;
        let query = doc! {"id": metadata.id};
        let update = doc! {"$set": {&format!("roles.{}", role_id): role_md, "transient": false}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .unwrap()
            .unwrap(); // TODO: not found error

        updated_metadata.roles.get(role_id).map(|md| md.to_owned())
    }

    pub async fn create_role(
        &self,
        metadata: ProjectMetadata,
        role_data: RoleData,
    ) -> Result<ProjectMetadata, std::io::Error> {
        let mut role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role_data)
            .await;

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let role_names = metadata
            .roles
            .into_values()
            .map(|r| r.project_name)
            .collect::<Vec<_>>();
        let role_name = get_unique_name(role_names, &role_md.project_name);
        role_md.project_name = role_name;

        let role_id = Uuid::new_v4();
        let query = doc! {"id": metadata.id};
        let update = doc! {"$set": {&format!("roles.{}", role_id): role_md}};
        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .unwrap()
            .expect("Project not found.");

        Ok(updated_metadata)
    }
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
