use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use mongodb::{Collection, Cursor};
use netsblox_cloud_common::api::{ProjectId, UpdateGalleryProjectData};

use crate::auth::{self, AddProject, DeleteGallery, EditGallery, ViewGallery};
use crate::errors::{InternalError, UserError};
use crate::utils;

use aws_sdk_s3 as s3;
use netsblox_cloud_common::{api, Bucket, Gallery, GalleryProjectMetadata};

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
        let gallery = Gallery::new(eu.username.clone(), name.into(), state.clone());
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

    pub(crate) async fn change_gallery(&self, egal: &EditGallery) -> Result<Gallery, UserError> {
        let query = doc! {"id": &egal.metadata.id};

        let mut update = doc! {"$set": {}};

        if let Some(n) = egal.change.name.clone() {
            update.get_document_mut("$set").unwrap().insert("name", n);
        }
        if let Some(s) = egal.change.state.clone() {
            update.get_document_mut("$set").unwrap().insert("state", s);
        }

        let fin_update = update;

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let gallery = self
            .galleries
            .find_one_and_update(query, fin_update, options)
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

    // for galleries, galleries/<gallery ID>/<project ID>/<version index>.xml
    // WARN:
    // WARN: what should happen if the client exceeds 9999 versions?
    fn get_s3key(ap: &AddProject, gal_proj: &GalleryProjectMetadata) -> api::S3Key {
        let ver_index = gal_proj.versions.len() + 1;
        let path = format!("{}/{}/{:04}.xml", ap.metadata.id, gal_proj.id, ver_index);

        api::S3Key::new(path)
    }

    pub(crate) async fn add_project(
        &self,
        ap: &AddProject,
    ) -> Result<GalleryProjectMetadata, UserError> {
        let mut gal_project = GalleryProjectMetadata::new(
            &ap.metadata,
            ap.project.owner.clone(),
            ap.project.name.clone(),
            ap.project.thumbnail.clone(),
        );

        let key: api::S3Key = GalleryActions::get_s3key(&ap, &gal_project);
        gal_project.versions.push(Some(key.clone()));

        let query = doc! {
            "gallery_id": gal_project.gallery_id.clone(),
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

        let s3_res =
            utils::upload(self.s3, self.bucket, &key, ap.project.project_xml.clone()).await;

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

    // diesel orm
    /// returns project in gallery
    pub(crate) async fn get_all_projects(
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

    pub(crate) async fn get_gallery_project_xml(
        &self,
        vgal: &ViewGallery,
        project_id: &ProjectId,
    ) -> Result<String, UserError> {
        let mut project = self.get_gallery_project(vgal, project_id).await?;
        // This cleans up deleted versions
        project.versions.retain(|ver| ver.is_some());

        let s3key = project
            .versions
            .last()
            .ok_or(UserError::GalleryNotFoundError)? // this resolves the .last() option
            .clone()
            .ok_or(UserError::GalleryNotFoundError)?;

        let xml = utils::download(self.s3, self.bucket, &s3key).await?; // this resolves the tombstone option

        Ok(xml)
    }

    pub(crate) async fn add_version(
        &self,
        ap: &AddProject,
    ) -> Result<GalleryProjectMetadata, UserError> {
        unimplemented!()
    }

    // for galleries, galleries/<gallery ID>/<project ID>/<Role ID>/<version index>.xml
    /// update project to s3
    pub(crate) fn update_project_in_gallery(
        &self,
        egal: &EditGallery,
        upd_proj: UpdateGalleryProjectData,
    ) -> Result<GalleryProjectMetadata, UserError> {
        unimplemented!()
    }

    pub(crate) async fn remove_project_in_gallery(
        &self,
        egal: &DeleteGallery,
        proj_id: &ProjectId,
    ) -> Result<Gallery, UserError> {
        unimplemented!()
    }

    pub(crate) fn remove_project_version_in_gallery(
        &self,
        egal: &EditGallery,
    ) -> Result<Gallery, UserError> {
        unimplemented!()
    }

    pub(crate) fn change_project_version_in_gallery(
        &self,
        egal: &EditGallery,
    ) -> Result<Gallery, UserError> {
        unimplemented!();
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
                let auth_egal = auth::EditGallery::test(&gallery, &change);

                actions.change_gallery(&auth_egal).await.unwrap();

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
                let auth_egal = auth::EditGallery::test(&gallery, &change);

                actions.change_gallery(&auth_egal).await.unwrap();

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
