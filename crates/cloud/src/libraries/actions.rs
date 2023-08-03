use futures::TryStreamExt;
use mongodb::{bson::doc, options::FindOptions, Collection};
use netsblox_cloud_common::{
    api::{self, PublishState},
    Library,
};

use crate::errors::{InternalError, UserError};

pub(crate) struct LibraryActions {
    libraries: Collection<Library>,
}

impl LibraryActions {
    pub(crate) async fn list_community_libraries(&self) -> Result<Vec<api::Library>, UserError> {
        let options = FindOptions::builder().sort(doc! {"name": 1}).build();
        let public_filter = doc! {"state": PublishState::Public};
        let cursor = self
            .libraries
            .find(public_filter, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let libraries = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(libraries)
    }
}
