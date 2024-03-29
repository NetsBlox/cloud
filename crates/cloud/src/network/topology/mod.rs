mod address;
mod client;
pub(crate) mod network;

use crate::app_data::AppData;
use crate::common::api::{ClientId, ExternalClient, ProjectId, RoleData, RoomState};
use crate::common::{api, OccupantInvite, ProjectMetadata};
use actix::dev::OneshotSender;
use actix::prelude::*;
use actix::{Actor, AsyncContext, Context, Handler};
use log::warn;
use netsblox_cloud_common::api::CollaborationInvite;
use serde::Serialize;
use serde_json::Value;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use tokio::sync::RwLock;
use uuid::Uuid;

use self::client::{RoleDataResponseState, RoleRequest, RESPONSE_BUFFER};
use self::network::Topology;
pub use self::network::DEFAULT_APP_ID;
use crate::common::api::{BrowserClientState, ClientState};

pub struct TopologyActor {
    network: Arc<RwLock<Topology>>,
    tx: Option<OneshotSender<TopologyPanic>>,
}

pub struct TopologyPanic;
impl TopologyActor {
    pub(crate) fn new(cache_size: NonZeroUsize, tx: Option<OneshotSender<TopologyPanic>>) -> Self {
        let network = Arc::new(RwLock::new(Topology::new(cache_size)));
        Self { network, tx }
    }
}

impl Actor for TopologyActor {
    type Context = Context<Self>;
}

impl Drop for TopologyActor {
    fn drop(&mut self) {
        if thread::panicking() {
            if let Some(tx) = self.tx.take() {
                log::warn!("Panic in topology detected. Notifying main thread.");
                let _ = tx.send(TopologyPanic);
            }
        }
    }
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub enum ClientCommand {
    SendMessage(Value),
    Close,
}

impl From<api::SendMessage> for ClientCommand {
    fn from(msg: api::SendMessage) -> ClientCommand {
        ClientCommand::SendMessage(msg.content)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddClient {
    pub id: ClientId,
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
    pub id: ClientId,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RemoveClient {
    pub id: ClientId,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SetClientState {
    pub id: ClientId,
    pub state: ClientState,
    pub username: Option<String>,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct SetClientUsername {
    pub id: ClientId,
    pub username: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage {
    pub sender: ClientId,
    pub addresses: Vec<String>,
    pub content: Value,
}

impl Handler<AddClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: AddClient, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.add_client(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<BrokenClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: BrokenClient, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
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
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.remove_client(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SetClientState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetClientState, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.set_client_state(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SetClientUsername> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetClientUsername, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.set_client_username(&msg.id, msg.username);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}
impl Handler<SendMessage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            topology.send_msg(msg).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SetStorage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetStorage, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.set_app_data(msg.app_data);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

impl Handler<SendRoomState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendRoomState, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
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
    pub client_id: ClientId,
}

impl Handler<EvictOccupant> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: EvictOccupant, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let mut topology = network.write().await;
            topology.evict_client(msg.client_id).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DisconnectClient {
    pub client_id: ClientId,
}

impl Handler<DisconnectClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: DisconnectClient, ctx: &mut Context<Self>) -> Self::Result {
        let client_id = msg.client_id;
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            topology.disconnect_client(&client_id);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Serialize)]
pub(crate) enum ChangeType {
    Add,
    Remove,
}

/// A notification that the collaboration invitations have changed.
/// Either a new invitation was sent or revoked/answered/etc
#[derive(Message, Serialize)]
#[rtype(result = "()")]
pub struct CollabInviteChangeMsg {
    r#type: &'static str,
    change: ChangeType,
    content: CollaborationInvite,
}

impl CollabInviteChangeMsg {
    pub(crate) fn new(change: ChangeType, content: CollaborationInvite) -> Self {
        let r#type = "collaboration-invitation";

        Self {
            r#type,
            change,
            content,
        }
    }
}

impl Handler<CollabInviteChangeMsg> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: CollabInviteChangeMsg, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            let receiver = msg.content.receiver.clone();
            let json = serde_json::to_value(msg).unwrap(); // we created the message so it should be infallible
            topology.send_to_user(json, &receiver);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

/// A notification that the friend requests have changed.
/// Either a new invitation was sent or revoked/answered/etc
#[derive(Message, Serialize)]
#[rtype(result = "()")]
pub struct FriendRequestChangeMsg {
    r#type: &'static str,
    change: ChangeType,
    content: api::FriendInvite,
}

impl FriendRequestChangeMsg {
    pub(crate) fn new(change: ChangeType, content: api::FriendInvite) -> Self {
        let r#type = "friend-request";

        Self {
            r#type,
            change,
            content,
        }
    }
}

impl Handler<FriendRequestChangeMsg> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: FriendRequestChangeMsg, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            let receiver = msg.content.recipient.clone();
            let json = serde_json::to_value(msg).unwrap(); // we created the message so it should be infallible
            topology.send_to_user(json, &receiver);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

/// A notification that the given project has been deleted.
#[derive(Message, Serialize)]
#[rtype(result = "()")]
pub struct ProjectDeleted {
    r#type: &'static str,
    project: ProjectMetadata,
}

impl ProjectDeleted {
    pub(crate) fn new(project: ProjectMetadata) -> Self {
        let r#type = "project-deleted";

        Self { r#type, project }
    }
}

impl Handler<ProjectDeleted> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: ProjectDeleted, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            let project_id = msg.project.id.clone();
            let json = serde_json::to_value(msg).unwrap(); // we created the message so it should be infallible
            topology.send_to_room(json, &project_id);
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
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            topology.send_occupant_invite(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessageFromServices {
    pub message: api::SendMessage,
}

impl Handler<SendMessageFromServices> for TopologyActor {
    type Result = ();

    fn handle(
        &mut self,
        send_msg_req: SendMessageFromServices,
        ctx: &mut Context<Self>,
    ) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            topology.send_msg_from_services(send_msg_req.message).await;
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SendIDEMessage {
    pub addresses: Vec<ClientId>,
    pub content: Value,
}

impl Handler<SendIDEMessage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendIDEMessage, ctx: &mut Context<Self>) -> Self::Result {
        let network = self.network.clone();
        let fut = async move {
            let topology = network.read().await;
            topology.send_ide_msg(msg);
        };
        let fut = actix::fut::wrap_future(fut);
        ctx.spawn(fut);
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetRoleRequestTask")]
pub struct GetRoleRequest {
    pub(crate) state: BrowserClientState,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetRoleRequestTask {
    network: Arc<RwLock<Topology>>,
    state: BrowserClientState,
}

impl GetRoleRequestTask {
    pub(crate) async fn run(self) -> Option<RoleRequest> {
        let topology = self.network.read().await;
        topology.get_role_request(self.state)
    }
}

impl Handler<GetRoleRequest> for TopologyActor {
    type Result = MessageResult<GetRoleRequest>;

    fn handle(&mut self, msg: GetRoleRequest, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetRoleRequestTask {
            network: self.network.clone(),
            state: msg.state,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetActiveRoomsTask")]
pub struct GetActiveRooms;

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetActiveRoomsTask {
    network: Arc<RwLock<Topology>>,
}

impl GetActiveRoomsTask {
    pub(crate) async fn run(&self) -> Vec<ProjectId> {
        let topology = self.network.read().await;
        topology.get_active_rooms()
    }
}

impl Handler<GetActiveRooms> for TopologyActor {
    type Result = MessageResult<GetActiveRooms>;

    fn handle(&mut self, _msg: GetActiveRooms, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetActiveRoomsTask {
            network: self.network.clone(),
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetOnlineUsersTask")]
pub(crate) struct GetOnlineUsers(pub Option<Vec<String>>);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetOnlineUsersTask {
    network: Arc<RwLock<Topology>>,
    allow_names: Option<Vec<String>>,
}

impl GetOnlineUsersTask {
    pub(crate) async fn run(self) -> Vec<String> {
        let topology = self.network.read().await;
        topology.get_online_users(self.allow_names)
    }
}

impl Handler<GetOnlineUsers> for TopologyActor {
    type Result = MessageResult<GetOnlineUsers>;

    fn handle(&mut self, msg: GetOnlineUsers, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetOnlineUsersTask {
            network: self.network.clone(),
            allow_names: msg.0,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetClientUsernameTask")]
pub(crate) struct GetClientUsername(pub ClientId);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetClientUsernameTask {
    network: Arc<RwLock<Topology>>,
    client_id: ClientId,
}

impl GetClientUsernameTask {
    pub(crate) async fn run(self) -> Option<String> {
        let topology = self.network.read().await;
        topology
            .get_client_username(&self.client_id)
            .map(|state| state.to_owned())
    }
}

impl Handler<GetClientUsername> for TopologyActor {
    type Result = MessageResult<GetClientUsername>;

    fn handle(&mut self, msg: GetClientUsername, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetClientUsernameTask {
            network: self.network.clone(),
            client_id: msg.0,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetClientStateTask")]
pub(crate) struct GetClientState(pub ClientId);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetClientStateTask {
    network: Arc<RwLock<Topology>>,
    client_id: ClientId,
}

impl GetClientStateTask {
    pub(crate) async fn run(self) -> Option<ClientState> {
        let topology = self.network.read().await;
        topology.get_client_state(&self.client_id).cloned()
    }
}

impl Handler<GetClientState> for TopologyActor {
    type Result = MessageResult<GetClientState>;

    fn handle(&mut self, msg: GetClientState, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetClientStateTask {
            network: self.network.clone(),
            client_id: msg.0,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetClientInfoTask")]
pub(crate) struct GetClientInfo(pub ClientId);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetClientInfoTask {
    network: Arc<RwLock<Topology>>,
    client_id: ClientId,
}

impl GetClientInfoTask {
    pub(crate) async fn run(self) -> Option<api::ClientInfo> {
        let topology = self.network.read().await;
        topology.get_client_info(&self.client_id)
    }
}

impl Handler<GetClientInfo> for TopologyActor {
    type Result = MessageResult<GetClientInfo>;

    fn handle(&mut self, msg: GetClientInfo, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetClientInfoTask {
            network: self.network.clone(),
            client_id: msg.0,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetRoomStateTask")]
pub(crate) struct GetRoomState(pub ProjectMetadata);

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetRoomStateTask {
    network: Arc<RwLock<Topology>>,
    project: ProjectMetadata,
}

impl GetRoomStateTask {
    pub(crate) async fn run(self) -> Option<RoomState> {
        let topology = self.network.read().await;
        topology.get_room_state(self.project)
    }
}

impl Handler<GetRoomState> for TopologyActor {
    type Result = MessageResult<GetRoomState>;

    fn handle(&mut self, msg: GetRoomState, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetRoomStateTask {
            network: self.network.clone(),
            project: msg.0,
        })
    }
}

#[derive(Message, Clone)]
#[rtype(result = "GetExternalClientsTask")]
pub(crate) struct GetExternalClients;

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct GetExternalClientsTask {
    network: Arc<RwLock<Topology>>,
}

impl GetExternalClientsTask {
    pub(crate) async fn run(self) -> Vec<ExternalClient> {
        let topology = self.network.read().await;
        topology.get_external_clients()
    }
}

impl Handler<GetExternalClients> for TopologyActor {
    type Result = MessageResult<GetExternalClients>;

    fn handle(&mut self, _msg: GetExternalClients, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(GetExternalClientsTask {
            network: self.network.clone(),
        })
    }
}
