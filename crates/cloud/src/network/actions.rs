use std::sync::{Arc, RwLock};

use actix::Addr;
use futures::TryStreamExt;
use lru::LruCache;
use mongodb::{
    bson::{doc, DateTime},
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Collection,
};
use netsblox_cloud_common::{
    api, NetworkTraceMetadata, OccupantInvite, ProjectMetadata, SentMessage,
};

use crate::{
    auth,
    errors::{InternalError, UserError},
    utils,
};

use super::topology::{self, TopologyActor};

pub(crate) struct NetworkActions {
    project_metadata: Collection<ProjectMetadata>,
    occupant_invites: Collection<OccupantInvite>,
    project_cache: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    recorded_messages: Collection<SentMessage>,
    network: Addr<TopologyActor>,
}

impl NetworkActions {
    pub(crate) fn new(
        project_metadata: Collection<ProjectMetadata>,
        project_cache: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
        network: Addr<TopologyActor>,

        occupant_invites: Collection<OccupantInvite>,
        recorded_messages: Collection<SentMessage>,
    ) -> Self {
        Self {
            project_metadata,
            occupant_invites,
            project_cache,
            recorded_messages,
            network,
        }
    }

    // TODO: can we ensure occupants can view the room state?
    pub(crate) async fn get_room_state(
        &self,
        vp: &auth::ViewProject,
    ) -> Result<api::RoomState, UserError> {
        let task = self
            .network
            .send(topology::GetRoomState(vp.metadata.clone()))
            .await
            .map_err(InternalError::ActixMessageError)?;
        let state = task.run().await.ok_or(UserError::ProjectNotActiveError)?;
        Ok(state)
    }

    pub(crate) async fn start_network_trace(
        &self,
        vp: &auth::ViewProject,
    ) -> Result<api::NetworkTraceMetadata, UserError> {
        let query = doc! {"id": &vp.metadata.id};
        let new_trace = NetworkTraceMetadata::new();
        let update = doc! {"$push": {"networkTraces": &new_trace}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        utils::update_project_cache(&self.project_cache, metadata);

        Ok(new_trace.into())
    }

    pub(crate) async fn stop_network_trace(
        &self,
        vp: &auth::ViewProject,
        trace_id: &str,
    ) -> Result<api::NetworkTraceMetadata, UserError> {
        let trace = vp
            .metadata
            .network_traces
            .iter()
            .find(|trace| trace.id == trace_id)
            .ok_or(UserError::NetworkTraceNotFoundError)?;

        let query = doc! {
            "id": &vp.metadata.id,
            "networkTraces.id": &trace.id
        };
        let end_time = DateTime::now();
        let update = doc! {
            "$set": {
                "networkTraces.$.endTime": end_time
            }
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        let trace = metadata
            .network_traces
            .iter()
            .find(|t| t.id == trace.id)
            .unwrap() // guaranteed to exist since it was checked in the query
            .clone();

        utils::update_project_cache(&self.project_cache, metadata);

        Ok(trace.into())
    }

    pub(crate) fn get_network_trace_metadata(
        &self,
        vp: &auth::ViewProject,
        trace_id: &str,
    ) -> Result<api::NetworkTraceMetadata, UserError> {
        let trace = vp
            .metadata
            .network_traces
            .iter()
            .find(|trace| trace.id == trace_id)
            .ok_or(UserError::NetworkTraceNotFoundError)?
            .to_owned();

        Ok(trace.into())
    }

    pub(crate) async fn get_network_trace(
        &self,
        vp: &auth::ViewProject,
        trace_id: &str,
    ) -> Result<Vec<api::SentMessage>, UserError> {
        let trace = vp
            .metadata
            .network_traces
            .iter()
            .find(|trace| trace.id == trace_id)
            .ok_or(UserError::NetworkTraceNotFoundError)?;

        let start_time = trace.start_time;
        let end_time = trace.end_time.unwrap_or_else(|| DateTime::now());

        let query = doc! {
            "projectId": &vp.metadata.id,
            "time": {"$gt": start_time, "$lt": end_time}
        };
        let cursor = self
            .recorded_messages
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let messages: Vec<api::SentMessage> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|msg| msg.into())
            .collect();

        Ok(messages)
    }

    pub(crate) async fn delete_network_trace(
        &self,
        vp: &auth::ViewProject,
        trace_id: &str,
    ) -> Result<api::ProjectMetadata, UserError> {
        let trace = vp
            .metadata
            .network_traces
            .iter()
            .find(|trace| trace.id == trace_id)
            .ok_or(UserError::NetworkTraceNotFoundError)?;

        let query = doc! {"id": &vp.metadata.id};
        let update = doc! {"$pull": {"networkTraces": &trace}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();
        let metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        // remove all the messages
        let earliest_start_time = metadata
            .network_traces
            .iter()
            .map(|trace| trace.start_time)
            .min()
            .unwrap_or(DateTime::MAX);

        let query = doc! {
            "projectId": &vp.metadata.id,
            "time": {"$lt": earliest_start_time}
        };

        self.recorded_messages
            .delete_many(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        utils::update_project_cache(&self.project_cache, metadata.clone());

        Ok(metadata.into())
    }

    pub(crate) async fn get_client_state(
        &self,
        vc: &auth::ViewClient,
    ) -> Result<api::ClientInfo, UserError> {
        let task = self
            .network
            .send(topology::GetClientUsername(vc.id.clone()))
            .await
            .map_err(InternalError::ActixMessageError)?;
        let username = task.run().await;
        let task = self
            .network
            .send(topology::GetClientState(vc.id.clone()))
            .await
            .map_err(InternalError::ActixMessageError)?;
        let state = task.run().await;
        Ok(api::ClientInfo { username, state })
    }

    pub(crate) async fn invite_occupant(
        &self,
        ep: &auth::EditProject,
        link: &auth::InviteLink,
        role_id: &api::RoleId,
    ) -> Result<api::OccupantInvite, UserError> {
        if !ep.metadata.roles.contains_key(role_id) {
            return Err(UserError::RoleNotFoundError);
        }

        let invite = OccupantInvite::new(
            link.target.to_owned(),
            ep.metadata.id.to_owned(),
            role_id.to_owned(),
        );
        self.occupant_invites
            .insert_one(&invite, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        self.network.do_send(topology::SendOccupantInvite {
            inviter: link.source.to_owned(),
            invite: invite.clone(),
            project: ep.metadata.clone(),
        });

        Ok(invite.into())
    }

    pub(crate) async fn evict_occupant(
        &self,
        ep: &auth::EvictClient,
    ) -> Result<Option<api::RoomState>, UserError> {
        self.network
            .send(topology::EvictOccupant {
                client_id: ep.id.clone(),
            })
            .await
            .map_err(InternalError::ActixMessageError)?;

        // Fetch the current state of the room
        let room_state = if let Some(metadata) = ep.project.clone() {
            let task = self
                .network
                .send(topology::GetRoomState(metadata))
                .await
                .map_err(InternalError::ActixMessageError)?;

            task.run().await
        } else {
            None
        };

        Ok(room_state)
    }
    pub(crate) async fn list_rooms(
        &self,
        _lr: &auth::ListActiveRooms,
    ) -> Result<Vec<api::ProjectId>, UserError> {
        let task = self
            .network
            .send(topology::GetActiveRooms {})
            .await
            .map_err(InternalError::ActixMessageError)?;
        let rooms = task.run().await;

        Ok(rooms)
    }

    pub(crate) async fn list_external_clients(
        &self,
        _lc: &auth::ListClients,
    ) -> Result<Vec<api::ExternalClient>, UserError> {
        let task = self
            .network
            .send(topology::GetExternalClients {})
            .await
            .map_err(InternalError::ActixMessageError)?;
        let clients = task.run().await;
        Ok(clients)
    }

    pub(crate) fn send_message(&self, sm: &auth::SendMessage) {
        self.network.do_send(topology::SendMessageFromServices {
            message: sm.msg.clone(),
        });
    }
}
