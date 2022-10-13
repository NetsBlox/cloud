mod address;
mod client;
mod network;

use crate::app_data::AppData;
use crate::models::{OccupantInvite, ProjectMetadata, RoleData};
use actix::prelude::*;
use actix::{Actor, AsyncContext, Context, Handler, MessageResult};
use lazy_static::lazy_static;
use log::warn;
use mongodb::bson::doc;
use netsblox_core::{ClientID, ExternalClient, ProjectId, RoomState};
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
pub enum ClientCommand {
    SendMessage(Value),
    Close,
}

impl From<netsblox_core::SendMessage> for ClientCommand {
    fn from(msg: netsblox_core::SendMessage) -> ClientCommand {
        ClientCommand::SendMessage(msg.content)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddClient {
    pub id: ClientID,
    pub addr: Recipient<ClientCommand>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetStorage {
    pub app_data: AppData,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendRoomState {
    pub project: ProjectMetadata,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BrokenClient {
    pub id: ClientID,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RemoveClient {
    pub id: ClientID,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetClientState {
    pub id: ClientID,
    pub state: ClientState,
    pub username: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage {
    // TODO: include sender username
    pub sender: ClientID,
    pub addresses: Vec<String>,
    pub content: Value,
}

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
            if let Err(error) = topology.set_broken_client(msg).await {
                warn!("Unable to record broken client: {:?}", error);
            }
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
        // TODO: check if the message should be recorded
        // TODO: should we first check what clients are going to receive it?
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
        topology.set_app_data(msg.app_data);
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
    pub usernames: Option<Vec<String>>,
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

#[derive(Message)]
#[rtype(result = "GetRoomStateResult")]
pub struct GetRoomState {
    pub metadata: ProjectMetadata,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetRoomStateResult(pub Option<RoomState>);

impl Handler<GetRoomState> for TopologyActor {
    type Result = MessageResult<GetRoomState>;

    fn handle(&mut self, msg: GetRoomState, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetRoomStateResult(topology.get_room_state(msg.metadata)))
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EvictOccupant {
    pub client_id: ClientID,
}

impl Handler<EvictOccupant> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: EvictOccupant, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().unwrap();
            topology.evict_client(msg.client_id).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message)]
#[rtype(result = "GetClientStateResult")]
pub struct GetClientState {
    pub client_id: ClientID,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetClientStateResult(pub Option<ClientState>);

impl Handler<GetClientState> for TopologyActor {
    type Result = MessageResult<GetClientState>;

    fn handle(&mut self, msg: GetClientState, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetClientStateResult(
            topology
                .get_client_state(&msg.client_id)
                .map(|state| state.clone()),
        ))
    }
}

#[derive(Message)]
#[rtype(result = "GetClientUsernameResult")]
pub struct GetClientUsername {
    pub client_id: ClientID,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetClientUsernameResult(pub Option<String>);

impl Handler<GetClientUsername> for TopologyActor {
    type Result = MessageResult<GetClientUsername>;

    fn handle(&mut self, msg: GetClientUsername, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        MessageResult(GetClientUsernameResult(
            topology
                .get_client_username(&msg.client_id)
                .map(|state| state.to_owned()),
        ))
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DisconnectClient {
    pub client_id: ClientID,
}

impl Handler<DisconnectClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: DisconnectClient, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        topology.disconnect_client(&msg.client_id);
    }
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SendOccupantInvite {
    pub inviter: String,
    pub invite: OccupantInvite,
    pub project: ProjectMetadata,
}

impl Handler<SendOccupantInvite> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendOccupantInvite, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        topology.send_occupant_invite(msg);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessageFromServices {
    pub message: netsblox_core::SendMessage,
}

impl Handler<SendMessageFromServices> for TopologyActor {
    type Result = ();

    fn handle(
        &mut self,
        send_msg_req: SendMessageFromServices,
        ctx: &mut Context<Self>,
    ) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().unwrap();
            topology.send_msg_from_services(send_msg_req.message).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SendIDEMessage {
    pub addresses: Vec<ClientID>,
    pub content: Value,
}

impl Handler<SendIDEMessage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendIDEMessage, _: &mut Context<Self>) -> Self::Result {
        let topology = TOPOLOGY.read().unwrap();
        topology.send_ide_msg(msg);
    }
}
