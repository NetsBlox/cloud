use std::sync::{Arc, RwLock};

use actix::Addr;
use actix_session::Session;
use lru::LruCache;
use mongodb::{
    bson::{doc, DateTime},
    options::ReturnDocument,
    Collection,
};
use netsblox_cloud_common::{
    api::{self, ClientId},
    BannedAccount, ProjectMetadata,
};

use crate::{
    app_data::metrics,
    errors::{InternalError, UserError},
    network::topology::{self, TopologyActor},
    utils,
};

/// This is a helper file containing logic reused across multiple resources.
/// Unlike `utils`, this is expected to be stateful and contains its own
/// references to collections, etc.

pub(crate) struct LoginHelper<'a> {
    network: &'a Addr<TopologyActor>,
    metrics: &'a metrics::Metrics,
    project_metadata: &'a Collection<ProjectMetadata>,
    project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,

    banned_accounts: &'a Collection<BannedAccount>,
}

impl<'a> LoginHelper<'a> {
    pub(crate) fn new(
        network: &'a Addr<TopologyActor>,
        metrics: &'a metrics::Metrics,
        project_metadata: &'a Collection<ProjectMetadata>,
        project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
        banned_accounts: &'a Collection<BannedAccount>,
    ) -> Self {
        Self {
            network,
            metrics,
            project_metadata,
            project_cache,
            banned_accounts,
        }
    }

    /// Login as the given user for the current session
    pub(crate) async fn login(
        &self,
        session: Session,
        user: &api::User,
        client_id: Option<ClientId>,
    ) -> Result<(), UserError> {
        // TODO: make sure the user isn't banned
        let query = doc! {"$or": [
            {"username": &user.username},
            {"email": &user.email},
        ]};

        if self
            .banned_accounts
            .find_one(query.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .is_some()
        {
            return Err(UserError::BannedUserError);
        }

        // update ownership, if applicable
        if let Some(client_id) = client_id {
            self.update_ownership(&client_id, &user.username).await?;
            self.network.do_send(topology::SetClientUsername {
                id: client_id,
                username: Some(user.username.clone()),
            });
        }
        self.metrics.record_login();

        session.insert("username", &user.username).unwrap();

        Ok(())
    }

    async fn update_ownership(
        &self,
        client_id: &api::ClientId,
        username: &str,
    ) -> Result<(), UserError> {
        // Update ownership of current project
        if !client_id.as_str().starts_with('_') {
            return Err(UserError::InvalidClientIdError);
        }

        let query = doc! {"owner": client_id.as_str()};
        if let Some(metadata) = self
            .project_metadata
            .find_one(query.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            // No project will be found for non-NetsBlox clients such as PyBlox
            let name = utils::get_valid_project_name(
                self.project_metadata,
                username,
                &metadata.name.as_str(),
            )
            .await?;
            let update = doc! {
                "$set": {
                    "owner": username,
                    "name": name,
                    "updated": DateTime::now(),
                }
            };
            let options = mongodb::options::FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();
            let new_metadata = self
                .project_metadata
                .find_one_and_update(query, update, Some(options))
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::ProjectNotFoundError)?;

            utils::on_room_changed(self.network, self.project_cache, new_metadata);
        }
        Ok(())
    }
}
