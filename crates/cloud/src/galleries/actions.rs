use actix_web::web::Bytes;
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use mongodb::{Collection, Cursor};
use netsblox_cloud_common::api::ProjectId;

use crate::auth::{
    self, AddGalleryProject, DeleteGallery, DeleteGalleryProject, EditGallery, EditGalleryProject,
    ViewGallery, ViewGalleryProject,
};
use crate::errors::{InternalError, UserError};
use crate::utils;

use aws_sdk_s3 as s3;
use netsblox_cloud_common::{api, Bucket, Gallery, GalleryProjectMetadata, Version};

pub(crate) struct GalleryActions<'a> {
    galleries: &'a Collection<Gallery>,
    gallery_projects: &'a Collection<GalleryProjectMetadata>,

    bucket: &'a Bucket,
    s3: &'a s3::Client,
}

impl<'a> GalleryActions<'a> {
    pub(crate) fn new(
        galleries: &'a Collection<Gallery>,
        gallery_projects: &'a Collection<GalleryProjectMetadata>,

        bucket: &'a Bucket,
        s3: &'a s3::Client,
    ) -> Self {
        Self {
            galleries,
            gallery_projects,
            bucket,
            s3,
        }
    }

    pub(crate) async fn create_gallery(
        &self,
        eu: &auth::EditUser,
        name: &str,
        state: api::PublishState,
    ) -> Result<Gallery, UserError> {
        // create gallery
        let gallery = Gallery::new(eu.username.clone(), name.to_string(), state.clone());
        // create mongodb formatted gallery
        let query = doc! {
          "name": &gallery.name,
          "owner": &gallery.owner,
        };
        // options for mongodb insertion
        let update = doc! {"$setOnInsert": &gallery};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let result = self
            .galleries
            .update_one(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Err(UserError::GalleryExistsError)
        } else {
            Ok(gallery)
        }
    }

    pub(crate) async fn view_galleries(
        &self,
        eu: &auth::EditUser,
    ) -> Result<Vec<Gallery>, UserError> {
        let query = doc! {
          "owner": &eu.username,
        };

        let result = self
            .galleries
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(result)
    }

    pub(crate) async fn change_gallery(
        &self,
        egal: &EditGallery,
        change: api::ChangeGalleryData,
    ) -> Result<Gallery, UserError> {
        let query = doc! {"id": &egal.metadata.id};

        let mut update = doc! {"$set": {}};

        // NOTE: fix this, it is ugly
        if let Some(n) = change.name.clone() {
            update.get_document_mut("$set").unwrap().insert("name", n);
        }
        if let Some(s) = change.state.clone() {
            update.get_document_mut("$set").unwrap().insert("state", s);
        }

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let gallery = self
            .galleries
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?;

        Ok(gallery)
    }

    /// delete a gallery
    pub(crate) async fn delete_gallery(&self, dgal: &DeleteGallery) -> Result<Gallery, UserError> {
        // create mongodb query
        let query = doc! {
          "id": &dgal.metadata.id
        };
        // delete gallery
        let gallery = self
            .galleries
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?;

        Ok(gallery)
    }

    pub(crate) async fn add_gallery_project(
        &self,
        ap: &AddGalleryProject,
        project: api::CreateGalleryProjectData,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let gal_project = GalleryProjectMetadata::new(
            &ap.metadata,
            project.owner.as_str(),
            project.name.as_str(),
        );

        let key: api::S3Key = gal_project
            .versions
            .first()
            .ok_or(UserError::GalleryProjectVersionsEmptyError)?
            .key
            .clone();

        let query = doc! {
            "galleryId": gal_project.gallery_id.clone(),
            "owner": gal_project.owner.clone(),
            "name": gal_project.name.clone(),
        };
        // options for mongodb insertion
        let update = doc! {"$setOnInsert": &gal_project};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let result = self
            .gallery_projects
            .update_one(query.clone(), update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            return Err(UserError::GalleryProjectExistsError);
        }

        let s3_res = utils::upload(self.s3, self.bucket, &key, project.project_xml.clone()).await;

        //FIX: If this fails, then memory leak in mongo
        if let Err(e) = s3_res {
            self.gallery_projects
                .delete_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            return Err(e.into());
        }

        Ok(gal_project.clone())
    }

    /// returns project in gallery
    pub(crate) async fn get_gallery_project(
        &self,
        vgal: &ViewGallery,
        project_id: &ProjectId,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let query = doc! {"galleryId": &vgal.metadata.id, "id": project_id};

        let project = self
            .gallery_projects
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?;

        Ok(project)
    }

    /// returns project in gallery
    pub(crate) async fn get_gallery_project_thumbnail(
        &self,
        vgal: &ViewGallery,
        project_id: &ProjectId,
        aspect_ratio: Option<f32>,
    ) -> Result<Bytes, UserError> {
        let query = doc! {"galleryId": &vgal.metadata.id, "id": project_id};

        let key = self
            .gallery_projects
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?
            .versions
            .iter()
            .rev()
            .find(|ver| !ver.deleted)
            .ok_or(UserError::GalleryProjectVersionsEmptyError)?
            .key
            .clone();

        let code = utils::download(self.s3, self.bucket, &key).await?;
        let thumbnail = utils::get_thumbnail(&code, aspect_ratio)?;

        Ok(thumbnail)
    }
    /// returns projects in gallery
    pub(crate) async fn get_all_gallery_projects(
        &self,
        vgal: &ViewGallery,
    ) -> Result<Vec<GalleryProjectMetadata>, UserError> {
        let query = doc! {"galleryId": &vgal.metadata.id};

        let cursor: Cursor<GalleryProjectMetadata> = self
            .gallery_projects
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let projects: Vec<GalleryProjectMetadata> = cursor
            .try_collect()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(projects)
    }

    pub(crate) async fn add_gallery_project_version(
        &self,
        ap: &EditGalleryProject,
        xml: String,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let index: usize = ap.project.versions.len();

        let version: Version = Version::new(&ap.project.gallery_id, &ap.project.id, index);

        let query = doc! { "id": ap.project.id.clone() };
        let update = doc! {"$push": {"versions": version.clone()}};

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let result = self
            .gallery_projects
            .find_one_and_update(update.clone(), update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?;

        let s3_res = utils::upload(self.s3, self.bucket, &version.key, xml).await;

        //FIXME:: if cleanup fails, we have mongo memory leak
        if let Err(e) = s3_res {
            let reset = doc! {"$set": {"versions": ap.project.versions.clone()}};
            self.gallery_projects
                .find_one_and_update(query, reset, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            return Err(e.into());
        }

        Ok(result)
    }

    pub(crate) async fn get_gallery_project_xml(
        &self,
        vgalp: &ViewGalleryProject,
    ) -> Result<String, UserError> {
        let s3key: api::S3Key = vgalp
            .project
            .versions
            .iter()
            .rev()
            .find(|ver| !ver.deleted)
            .ok_or(UserError::GalleryProjectVersionsEmptyError)?
            .key
            .clone();

        let xml = utils::download(self.s3, self.bucket, &s3key).await?;

        Ok(xml)
    }

    pub(crate) async fn get_gallery_project_xml_version(
        &self,
        vgalp: &ViewGalleryProject,
        index: usize,
    ) -> Result<String, UserError> {
        let s3key: api::S3Key = vgalp.project.versions[index].key.clone();

        let xml = utils::download(self.s3, self.bucket, &s3key).await?;

        Ok(xml)
    }

    pub(crate) async fn remove_project_in_gallery(
        &self,
        dp: &DeleteGalleryProject,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let keys: Vec<api::S3Key> = dp
            .project
            .versions
            .iter()
            .map(|ver| ver.key.clone())
            .collect();

        utils::delete_multiple(self.s3, self.bucket, keys).await?;

        let query = doc! {
          "id": &dp.project.id
        };

        let project = self
            .gallery_projects
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryNotFoundError)?;

        Ok(project)
    }

    pub(crate) async fn remove_project_version_in_gallery(
        &self,
        dgalp: &DeleteGalleryProject,
        index: usize,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let query = doc! {
          "id": dgalp.project.id.clone()
        };

        let update = doc! {
            "$set": {
                format!("gallery_project.versions.{}.deleted", index): true
            }
        };

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let result = self
            .gallery_projects
            .find_one_and_update(query.clone(), update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GalleryProjectVersionNotFound)?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;
    use netsblox_cloud_common::User;

    #[actix_web::test]
    async fn test_create_gallery() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();

                let auth_eu = auth::EditUser::test(user.username.clone());

                let gallery = actions
                    .create_gallery(&auth_eu, "mygallery", api::PublishState::Private)
                    .await
                    .unwrap();

                // Check that it exists in the database
                let query = doc! {"id": gallery.id};
                let metadata = actions.galleries.find_one(query, None).await.unwrap();

                assert!(metadata.is_some(), "Gallery not found in the database");
                let metadata = metadata.unwrap();
                assert_eq!(&metadata.name, "mygallery");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_gallery() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "owner".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let change = api::ChangeGalleryData {
            name: Some("fallery".into()),
            state: None,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();
                let auth_egal = auth::EditGallery::test(&gallery);

                actions.change_gallery(&auth_egal, change).await.unwrap();

                let query = doc! {"id": gallery.id};
                let metadata = actions.galleries.find_one(query, None).await.unwrap();

                assert!(metadata.is_some(), "Gallery not found in the database");
                let metadata = metadata.unwrap();
                assert_eq!(&metadata.name, "fallery", "Gallery not renamed");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_change_gallery_state() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let gallery = Gallery::new(
            "owner".into(),
            "mygallery".into(),
            api::PublishState::Private,
        );
        let change = api::ChangeGalleryData {
            name: None,
            state: Some(api::PublishState::Private),
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();
                let auth_egal = auth::EditGallery::test(&gallery);

                actions.change_gallery(&auth_egal, change).await.unwrap();

                let query = doc! {"id": gallery.id.clone()};
                let metadata = actions.galleries.find_one(query, None).await.unwrap();

                assert!(metadata.is_some(), "Gallery not found in the database");
                let metadata = metadata.unwrap();
                assert_eq!(
                    metadata.state,
                    api::PublishState::Private,
                    "Gallery not renamed"
                );
            })
            .await;
    }
}
