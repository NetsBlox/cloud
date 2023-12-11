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
    api::{self, SaveState},
    NetworkTraceMetadata, OccupantInvite, ProjectMetadata, SentMessage,
};

use crate::{
    auth,
    errors::{InternalError, UserError},
    utils,
};

use super::topology::{self, TopologyActor};

pub(crate) struct NetworkActions<'a> {
    project_metadata: &'a Collection<ProjectMetadata>,
    occupant_invites: &'a Collection<OccupantInvite>,
    project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    recorded_messages: &'a Collection<SentMessage>,
    network: &'a Addr<TopologyActor>,
}

impl<'a> NetworkActions<'a> {
    pub(crate) fn new(
        project_metadata: &'a Collection<ProjectMetadata>,
        project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
        network: &'a Addr<TopologyActor>,

        occupant_invites: &'a Collection<OccupantInvite>,
        recorded_messages: &'a Collection<SentMessage>,
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

    /// Update a project to ensure it isn't garbage collected due to inactivity
    pub(crate) async fn activate_room(&self, vp: &auth::ViewProject) -> Result<(), UserError> {
        let query = doc! {
            "id": &vp.metadata.id,
            "saveState": SaveState::Created
        };
        let update = doc! {
            "$set": {
                "saveState": SaveState::Transient,
                "updated": DateTime::now(),
            },
            "$unset": {
                "deleteAt": 1
            }
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if let Some(metadata) = metadata {
            utils::update_project_cache(self.project_cache, metadata);
        }

        Ok(())
    }

    pub(crate) async fn start_network_trace(
        &self,
        vp: &auth::ViewProject,
    ) -> Result<api::NetworkTraceMetadata, UserError> {
        let query = doc! {"id": &vp.metadata.id};
        let new_trace = NetworkTraceMetadata::new();
        let update = doc! {
            "$push": {
                "networkTraces": &new_trace
            },
            "$set": {
                "updated": DateTime::now(),
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

        utils::update_project_cache(self.project_cache, metadata);

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
                "networkTraces.$.endTime": end_time,
                "updated": DateTime::now(),
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

        utils::update_project_cache(self.project_cache, metadata);

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
        let end_time = trace.end_time.unwrap_or_else(DateTime::now);

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
        let update = doc! {
            "$pull": {
                "networkTraces": &trace,
            },
            "$set": {
                "updated": DateTime::now(),
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

        let metadata = utils::update_project_cache(self.project_cache, metadata);

        Ok(metadata.into())
    }

    pub(crate) async fn get_client_info(
        &self,
        vc: &auth::ViewClient,
    ) -> Result<api::ClientInfo, UserError> {
        let task = self
            .network
            .send(topology::GetClientInfo(vc.id.clone()))
            .await
            .map_err(InternalError::ActixMessageError)?;
        let info = task.run().await.ok_or(UserError::ClientNotFoundError)?;

        Ok(info)
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::test_utils;

    use super::*;

    #[actix_web::test]
    async fn test_activate_room() {
        let username: String = "username".into();
        let role_id = api::RoleId::new("role_id".into());
        let roles: HashMap<_, _> = [(
            role_id.clone(),
            api::RoleData {
                name: "some role".into(),
                code: "<code/>".into(),
                media: "<media/>".into(),
            },
        )]
        .into_iter()
        .collect();

        let project = test_utils::project::builder()
            .with_name("project")
            .with_owner(username.clone())
            .with_roles(roles)
            .build();

        test_utils::setup()
            .with_projects(&[project])
            .run(|app_data| async move {
                // set the project to Created
                let query = doc! {};
                let update = doc! {
                    "$set": {
                        "saveState": SaveState::Created
                    }
                };
                let options = FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build();

                let metadata = app_data
                    .project_metadata
                    .find_one_and_update(query, update, options)
                    .await
                    .expect("database lookup")
                    .unwrap();

                // Cache the current version
                let vp = auth::ViewProject::test(metadata);
                let actions = app_data.as_network_actions();
                let mut cache = actions.project_cache.write().unwrap();
                cache.put(vp.metadata.id.clone(), vp.metadata.clone());
                drop(cache);

                // Activate room
                actions.activate_room(&vp).await.unwrap();

                // Check that the cache has been updated
                let mut cache = actions.project_cache.write().unwrap();
                let cached = cache.get(&vp.metadata.id);
                assert!(cached.is_some(), "Project not cached after update");

                assert!(matches!(cached.unwrap().save_state, SaveState::Transient));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_get_client_state_not_found() {
        // Return client not found if the client isn't connected
        test_utils::setup()
            .run(|app_data| async move {
                let actions = app_data.as_network_actions();
                let vc = auth::ViewClient::test(api::ClientId::new("_nonexistentClientId".into()));
                let state = actions.get_client_info(&vc).await;
                assert!(matches!(state, Err(UserError::ClientNotFoundError)));
            })
            .await;
    }
}
