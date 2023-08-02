use actix::Addr;
use netsblox_cloud_common::api;

use crate::{
    auth,
    errors::{InternalError, UserError},
};

use super::routes::topology;

pub(crate) struct NetworkActions {
    network: Addr<TopologyActor>,
}

impl NetworkActions {
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
}
