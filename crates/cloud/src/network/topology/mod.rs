mod address;
mod client;
mod network;

use crate::app_data::AppData;
use crate::models::{OccupantInvite, ProjectMetadata, RoleData};
use actix::prelude::*;
use actix::{Actor, AsyncContext, Context, Handler};
use lazy_static::lazy_static;
use log::warn;
use mongodb::bson::doc;
use netsblox_core::{ClientID, ExternalClient, ProjectId, RoomState};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
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

    fn handle(&mut self, msg: AddClient, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().await;
            topology.add_client(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<BrokenClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: BrokenClient, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().await;
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
            let mut topology = TOPOLOGY.write().await;
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
            let mut topology = TOPOLOGY.write().await;
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
            let topology = TOPOLOGY.read().await;
            topology.send_msg(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SetStorage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetStorage, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().await;
            topology.set_app_data(msg.app_data);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SendRoomState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendRoomState, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().await;
            topology.send_room_state(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
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
#[rtype(result = "()")]
pub struct EvictOccupant {
    pub client_id: ClientID,
}

impl Handler<EvictOccupant> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: EvictOccupant, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let mut topology = TOPOLOGY.write().await;
            topology.evict_client(msg.client_id).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DisconnectClient {
    pub client_id: ClientID,
}

impl Handler<DisconnectClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: DisconnectClient, ctx: &mut Context<Self>) -> Self::Result {
        let client_id = msg.client_id;
        let fut = async move {
            let topology = TOPOLOGY.read().await;
            topology.disconnect_client(&client_id);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
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

    fn handle(&mut self, msg: SendOccupantInvite, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().await;
            topology.send_occupant_invite(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
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
            let topology = TOPOLOGY.read().await;
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

    fn handle(&mut self, msg: SendIDEMessage, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().await;
            topology.send_ide_msg(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

// methods for querying information from the topology
// TODO: There might be a better name for this. maybe make this an async builder?
pub(crate) async fn get_role_request(state: BrowserClientState) -> Option<RoleRequest> {
    let topology = TOPOLOGY.read().await;
    topology.get_role_request(state)
}

pub(crate) async fn get_active_rooms() -> Vec<ProjectId> {
    let topology = TOPOLOGY.read().await;
    topology.get_active_rooms()
}

pub(crate) async fn get_online_users(filter_names: Option<Vec<String>>) -> Vec<String> {
    let topology = TOPOLOGY.read().await;

    topology.get_online_users(filter_names)
}

pub(crate) async fn get_client_username(client_id: &ClientID) -> Option<String> {
    let topology = TOPOLOGY.read().await;
    topology
        .get_client_username(client_id)
        .map(|state| state.to_owned())
}

pub(crate) async fn get_client_state(client_id: &ClientID) -> Option<ClientState> {
    let topology = TOPOLOGY.read().await;
    topology.get_client_state(client_id).cloned()
}

pub(crate) async fn get_room_state(project: ProjectMetadata) -> Option<RoomState> {
    let topology = TOPOLOGY.read().await;
    topology.get_room_state(project)
}

pub(crate) async fn get_external_clients() -> Vec<ExternalClient> {
    let topology = TOPOLOGY.read().await;
    topology.get_external_clients()
}
