mod address;
mod client;
mod external;
mod network;

use crate::models::{ProjectMetadata, RoleData};
use actix::prelude::*;
use actix::{Actor, AsyncContext, Context, Handler, MessageResult};
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::Collection;
use netsblox_core::{ExternalClient, ProjectId};
use serde_json::Value;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use self::client::{RoleDataResponseState, RoleRequest, RESPONSE_BUFFER};
use self::network::Topology;
pub use self::network::{BrowserClientState, ClientState, ExternalClientState, DEFAULT_APP_ID};

lazy_static! {
    static ref TOPOLOGY: Arc<RwLock<Topology>> = Arc::new(RwLock::new(Topology::new()));
}

pub struct TopologyActor {}

impl Actor for TopologyActor {
    type Context = Context<Self>;
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ClientMessage(pub Value);

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddClient {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetStorage {
    pub project_metadata: Collection<ProjectMetadata>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendRoomState {
    pub project: ProjectMetadata,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BrokenClient {
    pub id: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RemoveClient {
    pub id: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetClientState {
    pub id: String,
    pub state: ClientState,
    pub username: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage {
    pub addresses: Vec<String>,
    pub content: Value,
}

// impl ClientState {
//     pub fn new(project_id: String, role_id: String, username: Option<String>) -> ClientState {
//         ClientState {
//             project_id,
//             role_id,
//             username,
//         }
//     }
// }

impl Handler<AddClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: AddClient, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.add_client(msg);
    }
}

impl Handler<BrokenClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: BrokenClient, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().unwrap();
            topology.set_broken_client(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<RemoveClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: RemoveClient, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().unwrap();
            topology.remove_client(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SetClientState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetClientState, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().unwrap();
            topology.set_client_state(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SendMessage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().unwrap();
            topology.send_msg(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}
impl Handler<SetStorage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetStorage, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.set_project_metadata(msg.project_metadata);
    }
}

impl Handler<SendRoomState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendRoomState, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        topology.send_room_state(msg);
    }
}

#[derive(Message)]
#[rtype(result = "GetRoleRequestResult")]
pub struct GetRoleRequest {
    pub state: BrowserClientState,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetRoleRequestResult(pub Option<RoleRequest>);

impl Handler<GetRoleRequest> for TopologyActor {
    type Result = MessageResult<GetRoleRequest>;

    fn handle(&mut self, msg: GetRoleRequest, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetRoleRequestResult(topology.get_role_request(msg.state)))
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RoleDataResponse {
    pub id: Uuid,
    pub data: RoleData,
}

impl Handler<RoleDataResponse> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: RoleDataResponse, _: &mut Context<Self>) -> Self::Result {
        let mut responses = RESPONSE_BUFFER.write().unwrap();
        responses.insert(msg.id, RoleDataResponseState::Data(msg.data));
    }
}

#[derive(Message)]
#[rtype(result = "GetActiveRoomsResult")]
pub struct GetActiveRooms;

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetActiveRoomsResult(pub Vec<ProjectId>);

impl Handler<GetActiveRooms> for TopologyActor {
    type Result = MessageResult<GetActiveRooms>;

    fn handle(&mut self, _: GetActiveRooms, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetActiveRoomsResult(topology.get_active_rooms()))
    }
}

#[derive(Message)]
#[rtype(result = "Vec<String>")]
pub struct GetOnlineUsers {
    pub usernames: Vec<String>,
}

impl Handler<GetOnlineUsers> for TopologyActor {
    type Result = Vec<String>;

    fn handle(&mut self, msg: GetOnlineUsers, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        topology.get_online_users(msg.usernames)
    }
}

#[derive(Message)]
#[rtype(result = "GetExternalClientsResult")]
pub struct GetExternalClients;

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetExternalClientsResult(pub Vec<ExternalClient>);

impl Handler<GetExternalClients> for TopologyActor {
    type Result = MessageResult<GetExternalClients>;

    fn handle(&mut self, _: GetExternalClients, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetExternalClientsResult(topology.get_external_clients()))
    }
}
