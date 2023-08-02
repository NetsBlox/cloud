use std::collections::HashMap;
use std::io::BufWriter;
use std::sync::{Arc, RwLock};

// TODO: is there any shared fn-ality across actions?
use crate::errors::{InternalError, UserError};
use crate::network::topology::{self, TopologyActor};
use crate::{auth, libraries};
use actix::Addr;
use actix_web::web::Bytes;
use futures::future::join_all;
use futures::stream::FuturesUnordered;
use futures::{join, TryStreamExt};
use image::{
    codecs::png::PngEncoder, ColorType, EncodableLayout, GenericImageView, ImageEncoder,
    ImageFormat, RgbaImage,
};
use lazy_static::lazy_static;
use log::warn;
use lru::LruCache;
use mongodb::bson::{doc, DateTime};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::Collection;
use netsblox_cloud_common::api::{BrowserClientState, RoleData, RoleId, SaveState};
use netsblox_cloud_common::{
    api::{self, PublishState},
    ProjectMetadata,
};
use netsblox_cloud_common::{Project, RoleMetadata};
use rusoto_s3::{
    DeleteObjectRequest, GetObjectRequest, PutObjectOutput, PutObjectRequest, S3Client, S3,
};
use uuid::Uuid;

// FIXME: pass this as an argument to ProjectActions
lazy_static! {
    static ref PROJECT_CACHE: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>> =
        Arc::new(RwLock::new(LruCache::new(500)));
}

pub(crate) struct ProjectActions {
    // TODO: can I make cached projects?
    project_metadata: Collection<ProjectMetadata>,
    bucket: String,
    s3: S3Client,
    network: Addr<TopologyActor>,
}

impl ProjectActions {
    pub async fn create_project(
        &self,
        eu: &auth::EditUser,
        project_data: api::CreateProjectData,
    ) -> Result<ProjectMetadata, UserError> {
        let name = project_data.name.to_owned();
        let mut roles = project_data
            .roles
            .unwrap_or_default()
            .into_iter()
            .map(|role| (RoleId::new(Uuid::new_v4().to_string()), role))
            .collect::<HashMap<_, _>>();
        let owner = &eu.username;
        let unique_name = self.get_valid_project_name(owner, &name).await?;

        // Prepare the roles (ensure >=1 exists; upload them)
        if roles.is_empty() {
            roles.insert(
                RoleId::new(Uuid::new_v4().to_string()),
                RoleData {
                    name: "myRole".to_owned(),
                    code: "".to_owned(),
                    media: "".to_owned(),
                },
            );
        };

        let role_mds: Vec<RoleMetadata> = join_all(
            roles
                .values()
                .map(|role| self.upload_role(owner, &unique_name, role)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<RoleMetadata>, _>>()?;

        let roles: HashMap<RoleId, RoleMetadata> =
            roles.keys().cloned().zip(role_mds.into_iter()).collect();

        let save_state = project_data.save_state.unwrap_or(SaveState::CREATED);
        let metadata = ProjectMetadata::new(owner, &unique_name, roles, save_state);
        self.project_metadata
            .insert_one(metadata.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(metadata)
    }

    pub(crate) async fn get_project(
        &self,
        view_proj: &auth::ViewProject,
    ) -> Result<api::Project, UserError> {
        let metadata = view_proj.metadata.clone();
        let (keys, values): (Vec<_>, Vec<_>) = metadata.roles.clone().into_iter().unzip();
        // TODO: make fetch_role fallible
        let role_data = join_all(values.iter().map(|v| self.fetch_role(v))).await;

        let roles = keys
            .into_iter()
            .zip(role_data)
            .filter_map(|(k, data)| data.map(|d| (k, d)).ok())
            .collect::<HashMap<RoleId, _>>();

        let project = Project {
            id: metadata.id,
            name: metadata.name,
            owner: metadata.owner,
            updated: metadata.updated,
            state: metadata.state,
            collaborators: metadata.collaborators,
            origin_time: metadata.origin_time,
            save_state: metadata.save_state,
            roles,
        };

        Ok(project.into())
    }

    pub(crate) async fn get_latest_project(
        &self,
        vp: &auth::ViewProject,
    ) -> Result<api::Project, UserError> {
        let metadata = vp.metadata.clone();
        let roles = metadata
            .roles
            .keys()
            .map(|role_id| self.fetch_role_data(&vp.clone(), role_id.to_owned()))
            .collect::<FuturesUnordered<_>>()
            .try_collect::<HashMap<RoleId, RoleData>>()
            .await?;

        let project = api::Project {
            id: metadata.id,
            name: metadata.name,
            owner: metadata.owner,
            updated: metadata.updated.to_system_time(),
            state: metadata.state,
            collaborators: metadata.collaborators,
            origin_time: metadata.origin_time.to_system_time(),
            save_state: metadata.save_state,
            roles,
        };

        Ok(project)
    }

    pub(crate) async fn get_project_thumbnail(
        &self,
        vp: &auth::ViewProject,
        aspect_ratio: Option<f32>,
    ) -> Result<Bytes, UserError> {
        let role_metadata = vp
            .metadata
            .roles
            .values()
            .max_by_key(|md| md.updated)
            .ok_or(UserError::ThumbnailNotFoundError)?;

        // TODO: only fetch the code
        let role = self.fetch_role(role_metadata).await?;
        let thumbnail = role
            .code
            .split("<thumbnail>data:image/png;base64,")
            .nth(1)
            .and_then(|text| text.split("</thumbnail>").next())
            .ok_or(UserError::ThumbnailNotFoundError)
            .and_then(|thumbnail_str| {
                base64::decode(thumbnail_str)
                    .map_err(|err| InternalError::Base64DecodeError(err).into())
            })
            .and_then(|image_data| {
                image::load_from_memory_with_format(&image_data, ImageFormat::Png)
                    .map_err(|err| InternalError::ThumbnailDecodeError(err).into())
            })?;

        let image_content = if let Some(aspect_ratio) = aspect_ratio {
            let (width, height) = thumbnail.dimensions();
            let current_ratio = (width as f32) / (height as f32);
            let (resized_width, resized_height) = if current_ratio < aspect_ratio {
                let new_width = (aspect_ratio * (height as f32)) as u32;
                (new_width, height)
            } else {
                let new_height = ((width as f32) / aspect_ratio) as u32;
                (width, new_height)
            };

            let top_offset: u32 = (resized_height - height) / 2;
            let left_offset: u32 = (resized_width - width) / 2;
            let mut image = RgbaImage::new(resized_width, resized_height);
            for x in 0..width {
                for y in 0..height {
                    let pixel = thumbnail.get_pixel(x, y);
                    image.put_pixel(x + left_offset, y + top_offset, pixel);
                }
            }
            // encode the bytes as a png
            let mut png_bytes = BufWriter::new(Vec::new());
            let encoder = PngEncoder::new(&mut png_bytes);
            let color = ColorType::Rgba8;
            encoder
                .write_image(image.as_bytes(), resized_width, resized_height, color)
                .map_err(InternalError::ThumbnailEncodeError)?;
            actix_web::web::Bytes::copy_from_slice(&png_bytes.into_inner().unwrap())
        } else {
            let (width, height) = thumbnail.dimensions();
            let mut png_bytes = BufWriter::new(Vec::new());
            let encoder = PngEncoder::new(&mut png_bytes);
            let color = ColorType::Rgba8;
            encoder
                .write_image(thumbnail.as_bytes(), width, height, color)
                .map_err(InternalError::ThumbnailEncodeError)?;
            actix_web::web::Bytes::copy_from_slice(&png_bytes.into_inner().unwrap())
        };

        Ok(image_content)
    }

    pub(crate) fn get_project_metadata(&self, vp: &auth::ViewProject) -> api::ProjectMetadata {
        vp.metadata.clone().into()
    }

    pub(crate) async fn rename_project(
        &self,
        ep: &auth::EditProject,
        new_name: &str,
    ) -> Result<api::ProjectMetadata, UserError> {
        let metadata = &ep.metadata;
        let name = self
            .get_valid_project_name(&metadata.owner, &new_name)
            .await?;

        let query = doc! {"id": &metadata.id};
        let update = doc! {"$set": {"name": &name}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());
        Ok(updated_metadata.into())
    }

    pub(crate) fn get_collaborators(&self, md: &auth::ViewProject) -> Vec<String> {
        md.metadata.collaborators.clone()
    }

    pub(crate) async fn remove_collaborator(
        &self,
        ep: &auth::EditProject,
        collaborator: &str,
    ) -> Result<api::ProjectMetadata, UserError> {
        let query = doc! {"id": &ep.metadata.id};
        let update = doc! {"$pull": {"collaborators": &collaborator}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(metadata.clone());

        Ok(metadata.into())
    }

    pub(crate) async fn set_latest_role(
        &self,
        md: &auth::EditProject,
        role_id: &RoleId,
        id: &Uuid,
        data: RoleData,
    ) -> Result<(), UserError> {
        md.metadata
            .roles
            .keys()
            .position(|key| key == role_id)
            .ok_or(UserError::RoleNotFoundError)?;

        self.network.do_send(topology::RoleDataResponse {
            id: id.to_owned(),
            data,
        });
        Ok(())
    }

    pub(crate) async fn publish_project(
        &self,
        ep: &auth::EditProject,
    ) -> Result<api::PublishState, UserError> {
        let state = if self.is_approval_required(&ep.metadata).await? {
            PublishState::PendingApproval
        } else {
            PublishState::Public
        };

        let query = doc! {"id": &ep.metadata.id};
        let update = doc! {"$set": {"state": &state}};
        self.project_metadata
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(state)
    }

    pub(crate) async fn unpublish_project(
        &self,
        edit: &auth::EditProject,
    ) -> Result<api::PublishState, UserError> {
        let query = doc! {"id": &edit.metadata.id};
        let state = PublishState::Private;
        let update = doc! {"$set": {"state": &state}};
        self.project_metadata
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        Ok(state)
    }

    pub(crate) async fn fetch_role_data(
        &self,
        vp: &auth::ViewProject,
        role_id: RoleId,
    ) -> Result<(RoleId, RoleData), UserError> {
        let role_md = vp
            .metadata
            .roles
            .get(&role_id)
            .ok_or(UserError::RoleNotFoundError)?;

        // Try to fetch the role data from the current occupants
        let state = BrowserClientState {
            project_id: vp.metadata.id.clone(),
            role_id: role_id.clone(),
        };

        let task = self
            .network
            .send(topology::GetRoleRequest { state })
            .await
            .map_err(InternalError::ActixMessageError)?;
        let request_opt = task.run().await.ok_or(UserError::InternalError);

        let active_role = if let Ok(request) = request_opt {
            request.send().await.ok()
        } else {
            None
        };

        // If unable to retrieve role data from current occupants (unoccupied or error),
        // fetch the latest from the database
        let role_data = match active_role {
            Some(role_data) => role_data,
            None => self.fetch_role(role_md).await?,
        };
        Ok((role_id, role_data))
    }

    pub async fn create_role(
        &self,
        ep: &auth::EditProject,
        role_data: RoleData,
    ) -> Result<ProjectMetadata, UserError> {
        let metadata = ep.metadata;
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
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());
        Ok(updated_metadata)
    }

    pub(crate) async fn rename_role(
        &self,
        ep: &auth::EditProject,
        role_id: RoleId,
        name: &str,
    ) -> Result<ProjectMetadata, UserError> {
        ensure_valid_name(name)?;
        if ep.metadata.roles.contains_key(&role_id) {
            let query = doc! {"id": &ep.metadata.id};
            let update = doc! {"$set": {format!("roles.{}.name", role_id): name}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();

            let updated_metadata = self
                .project_metadata
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::ProjectNotFoundError)?;

            self.on_room_changed(updated_metadata.clone());
            Ok(updated_metadata)
        } else {
            Err(UserError::RoleNotFoundError)
        }
    }

    pub(crate) async fn delete_role(
        &self,
        ep: &auth::EditProject,
        role_id: RoleId,
    ) -> Result<ProjectMetadata, UserError> {
        if ep.metadata.roles.keys().count() == 1 {
            return Err(UserError::CannotDeleteLastRoleError);
        }

        let query = doc! {"id": &ep.metadata.id};
        let update = doc! {"$unset": {format!("roles.{}", role_id): &""}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());
        Ok(updated_metadata)
    }

    pub(crate) async fn get_role(
        &self,
        vp: &auth::ViewProject,
        role_id: RoleId,
    ) -> Result<RoleData, UserError> {
        let role_md = vp
            .metadata
            .roles
            .get(&role_id)
            .ok_or(UserError::RoleNotFoundError)?;

        let role = self.fetch_role(role_md).await?;
        Ok(role)
    }

    /// Send updated room state and update project cache when room structure is changed or renamed
    pub async fn save_role(
        &self,
        ep: &auth::EditProject,
        role_id: &RoleId,
        role: RoleData,
    ) -> Result<ProjectMetadata, UserError> {
        let metadata = &ep.metadata;
        let role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role)
            .await?;

        // check if the (public) project needs to be re-approved
        let state = match metadata.state {
            PublishState::Public => {
                let needs_approval = libraries::is_approval_required(&role.code);
                if needs_approval {
                    PublishState::PendingApproval
                } else {
                    PublishState::Public
                }
            }
            _ => metadata.state.clone(),
        };

        let query = doc! {"id": &metadata.id};
        let update = doc! {
            "$set": {
                &format!("roles.{}", role_id): role_md,
                "saveState": SaveState::SAVED,
                "state": state,
            }
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());

        Ok(updated_metadata)
    }

    pub async fn delete_project(
        &self,
        dp: &auth::DeleteProject,
    ) -> Result<ProjectMetadata, UserError> {
        let query = doc! {"id": &dp.metadata.id};
        let metadata = self
            .project_metadata
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        let paths = metadata
            .roles
            .clone()
            .into_values()
            .flat_map(|role| vec![role.code, role.media]);

        join_all(paths.map(move |path| self.delete(path)))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        self.network
            .do_send(topology::ProjectDeleted::new(metadata.clone()));

        Ok(metadata)
    }

    // Helper functions
    async fn fetch_role(&self, metadata: &RoleMetadata) -> Result<RoleData, InternalError> {
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

        String::from_utf8(byte_str).map_err(|_err| InternalError::S3ContentError)
    }

    async fn delete(&self, key: String) -> Result<(), UserError> {
        let request = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key,
            ..Default::default()
        };

        self.s3
            .delete_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)?;

        Ok(())
    }

    async fn is_approval_required(&self, metadata: &ProjectMetadata) -> Result<bool, UserError> {
        for role_md in metadata.roles.values() {
            let role = self.fetch_role(role_md).await?;
            if libraries::is_approval_required(&role.code) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get a unique project name for the given user and preferred name.
    async fn get_valid_project_name(
        &self,
        owner: &str,
        basename: &str,
    ) -> Result<String, UserError> {
        ensure_valid_name(basename)?;

        let query = doc! {"owner": &owner};
        let cursor = self
            .project_metadata
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let project_names = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .iter()
            .map(|md| md.name.to_owned())
            .collect();

        Ok(get_unique_name(project_names, basename))
    }

    // FIXME: is there a better abstraction here that we could use?
    fn on_room_changed(&self, updated_project: ProjectMetadata) {
        self.network.do_send(topology::SendRoomState {
            project: updated_project.clone(),
        });

        self.update_project_cache(updated_project);
    }

    pub fn update_project_cache(&self, metadata: ProjectMetadata) {
        let mut cache = PROJECT_CACHE.write().unwrap();
        cache.put(metadata.id.clone(), metadata);
    }

    fn get_cached_project_metadata<'a>(
        &self,
        ids: impl Iterator<Item = &'a api::ProjectId>,
    ) -> (Vec<ProjectMetadata>, Vec<&'a api::ProjectId>) {
        let mut results = Vec::new();
        let mut missing_projects = Vec::new();
        let mut cache = PROJECT_CACHE.write().unwrap();
        for id in ids {
            match cache.get(id) {
                Some(project_metadata) => results.push(project_metadata.clone()),
                None => missing_projects.push(id),
            }
        }
        (results, missing_projects)
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
            updated: DateTime::now(),
        })
    }

    async fn upload(&self, key: &str, body: String) -> Result<PutObjectOutput, InternalError> {
        let request = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            body: Some(String::into_bytes(body).into()),
            ..Default::default()
        };
        self.s3.put_object(request).await.map_err(|err| {
            warn!("Unable to upload to s3: {}", err);
            InternalError::S3Error
        })
    }
}
