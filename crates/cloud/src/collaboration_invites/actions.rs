use std::sync::{Arc, RwLock};

use actix::Addr;
use futures::TryStreamExt;
use lru::LruCache;
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Collection,
};
use netsblox_cloud_common::{
    api::{self, InvitationState},
    CollaborationInvite, ProjectMetadata,
};

use crate::{
    auth,
    errors::{InternalError, UserError},
    network::{self, topology::TopologyActor},
    utils,
};

pub(crate) struct CollaborationInviteActions {
    collab_invites: Collection<CollaborationInvite>,

    project_metadata: Collection<ProjectMetadata>,
    project_cache: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    network: Addr<TopologyActor>,
}

impl CollaborationInviteActions {
    pub(crate) fn new(
        collab_invites: Collection<CollaborationInvite>,

        project_metadata: Collection<ProjectMetadata>,
        project_cache: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
        network: Addr<TopologyActor>,
    ) -> Self {
        Self {
            collab_invites,
            project_metadata,
            project_cache,
            network,
        }
    }
    pub(crate) async fn list_invites(
        &self,
        eu: &auth::ViewUser,
    ) -> Result<Vec<api::CollaborationInvite>, UserError> {
        let query = doc! {"receiver": &eu.username};
        let cursor = self
            .collab_invites
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let invites: Vec<api::CollaborationInvite> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|invite| invite.into())
            .collect();

        Ok(invites)
    }

    // TODO: should I restrict sending the invite based on target?
    // TODO: only allow the owner to send collaboration invites?
    pub(crate) async fn send_invite(
        &self,
        ep: &auth::InviteCollaborator,
        target: &str,
    ) -> Result<api::CollaborationInvite, UserError> {
        let sender = ep.project.owner.to_owned();
        let invitation =
            CollaborationInvite::new(sender.clone(), target.to_owned(), ep.project.id.clone());

        let query = doc! {
            "sender": &sender,
            "receiver": &target,
            "projectId": &invitation.project_id
        };
        let update = doc! {
            "$setOnInsert": &invitation
        };
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let result = self
            .collab_invites
            .update_one(query, update, Some(options))
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Err(UserError::InviteAlreadyExistsError)
        } else {
            // notify the recipient of the new invitation
            let invitation: api::CollaborationInvite = invitation.into();
            self.network
                .send(network::topology::CollabInviteChangeMsg::new(
                    network::topology::ChangeType::Add,
                    invitation.clone(),
                ))
                .await
                .map_err(InternalError::ActixMessageError)?;

            Ok(invitation)
        }
    }

    pub(crate) async fn respond(
        &self,
        ri: &auth::RespondToCollabInvite,
        state: InvitationState,
    ) -> Result<InvitationState, UserError> {
        let query = doc! {"id": &ri.invite.id};
        let invitation = self
            .collab_invites
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::InviteNotFoundError)?;

        // Update the project
        if matches!(state, InvitationState::Accepted) {
            let query = doc! {"id": &ri.invite.project_id};
            let update = doc! {"$addToSet": {"collaborators": &ri.invite.receiver}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();

            let updated_metadata = self
                .project_metadata
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::ProjectNotFoundError)?;

            utils::on_room_changed(&self.network, &self.project_cache, updated_metadata);
        }
        // Update the project
        let invitation: api::CollaborationInvite = invitation.into();
        self.network
            .send(network::topology::CollabInviteChangeMsg::new(
                network::topology::ChangeType::Remove,
                invitation,
            ))
            .await
            .map_err(InternalError::ActixMessageError)?;

        Ok(state)
    }
}
