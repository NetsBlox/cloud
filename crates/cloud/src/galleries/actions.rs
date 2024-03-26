use mongodb::bson::doc;
use mongodb::options::ReturnDocument;
use mongodb::Collection;

use netsblox_cloud_common::{api, Gallery};

use crate::auth::{self, DeleteGallery, EditGallery};
use crate::errors::{InternalError, UserError};

pub(crate) struct GalleryActions<'a> {
    galleries: &'a Collection<Gallery>,
}

impl<'a> GalleryActions<'a> {
    pub(crate) fn new(galleries: &'a Collection<Gallery>) -> Self {
        Self { galleries }
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

    pub(crate) async fn rename_gallery(
        &self,
        egal: &EditGallery,
        name: &str,
    ) -> Result<Gallery, UserError> {
        let query = doc! {"id": &egal.metadata.id};
        let update = doc! {"$set": {"name": &name}};
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

    pub(crate) async fn change_gallery_state(
        &self,
        egal: &EditGallery,
        state: &api::PublishState,
    ) -> Result<Gallery, UserError> {
        let query = doc! {"id": &egal.metadata.id};

        let update = doc! {"$set": {"state": &state}};
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
}
// break into smaller pieces.
// use with users to decouple
// from other tests.

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

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();
                let auth_egal = auth::EditGallery::test(&gallery.clone().into());

                actions.rename_gallery(&auth_egal, "fallery").await.unwrap();

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

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_galleries(&[gallery.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();
                let auth_egal = auth::EditGallery::test(&gallery.clone().into());

                actions
                    .change_gallery_state(&auth_egal, api::PublishState::Private)
                    .await
                    .unwrap();

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
