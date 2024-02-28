use mongodb::bson::doc;
use mongodb::Collection;
use netsblox_cloud_common::{api, Gallery};

use crate::errors::UserError;

pub(crate) struct GalleryActions<'a> {
    galleries: &'a Collection<Gallery>,
}

impl<'a> GalleryActions<'a> {
    pub(crate) fn new(galleries: &'a Collection<Gallery>) -> Self {
        Self { galleries }
    }

    /// Create a gallery for a given user
    pub(crate) async fn create_gallery(
        &self,
        // TODO: this function will need some arguments
        //_ml: &auth::ModerateGalleries,
        //owner: &str,
        //name: &str,
        //state: api::PublishState,
    ) -> Result<api::Gallery, UserError> {
        todo!("need to implement this!");
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;
    use actix_web::test;

    #[actix_web::test]
    async fn test_create_gallery() {
        test_utils::setup()
            //.with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_gallery_actions();

                // TODO: call the endpoint to create the gallery
                actions.create_gallery().await.unwrap();

                // Check that it exists in the database
                let query = doc! {};
                let metadata = actions.galleries.find_one(query, None).await.unwrap();

                assert!(metadata.is_some(), "Gallery not found in the database");
                let metadata = metadata.unwrap();
                assert_eq!(&metadata.name, "mygallery");
            })
            .await;
    }
}
