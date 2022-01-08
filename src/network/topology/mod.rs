mod address;
mod external;

use actix::prelude::{Message, Recipient};
use actix::{Actor, AsyncContext, Context, Handler};
use futures::future::join_all;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::Collection;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use crate::models::ProjectMetadata;
use crate::network::topology::address::ClientAddress;
use crate::network::topology::external::ExternalNetwork;

use self::external::ExternalClientState;

#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

struct ProjectNetwork {
    id: String,
    roles: HashMap<String, Vec<String>>,
}

impl ProjectNetwork {
    fn new(id: String) -> ProjectNetwork {
        ProjectNetwork {
            id,
            roles: HashMap::new(),
        }
    }
}

struct Topology {
    project_metadata: Option<Collection<ProjectMetadata>>,

    clients: HashMap<String, Client>,
    states: HashMap<String, ClientState>, // TODO: Make ClientState an enum?

    rooms: HashMap<String, ProjectNetwork>, // TODO: Make this different?
    external: HashMap<String, ExternalNetwork>,
}

lazy_static! {
    static ref TOPOLOGY: Arc<RwLock<Topology>> = Arc::new(RwLock::new(Topology::new()));
}

impl Topology {
    pub fn new() -> Topology {
        Topology {
            clients: HashMap::new(),
            project_metadata: None,
            rooms: HashMap::new(),
            states: HashMap::new(),
            external: HashMap::new(), // ExternalNetwork::new(),
        }
    }

    fn set_project_metadata(&mut self, project_metadata: Collection<ProjectMetadata>) {
        self.project_metadata = Some(project_metadata);
    }

    async fn get_clients_at(&self, addr: ClientAddress) -> Vec<&Client> {
        let mut client_ids: Vec<&String> = Vec::new();
        let empty = Vec::new();
        for app_id in &addr.app_ids {
            if app_id == "netsblox" {
                let addresses = self.resolve_address(&addr).await;
                let ids = addresses.into_iter().flat_map(|addr| {
                    self.rooms
                        .get(&addr.project_id)
                        .and_then(|room| room.roles.get(&addr.role_id))
                        .unwrap_or(&empty)
                });
                client_ids.extend(ids);
            } else {
                let ids = self
                    .external
                    .get(app_id)
                    .map(|network| network.get_clients_at(&addr))
                    .unwrap_or(Vec::new())
                    .into_iter();
                client_ids.extend(ids);
            }
        }

        client_ids
            .into_iter()
            .filter_map(|id| self.clients.get(id))
            .collect()
    }

    pub async fn resolve_address(&self, addr: &ClientAddress) -> Vec<BrowserAddress> {
        if self.project_metadata.is_none() {
            return Vec::new();
        }

        let project_metadata = self.project_metadata.as_ref().unwrap();

        let mut chunks = addr.address.split('@').rev();
        let project = chunks.next().unwrap();
        let role = chunks.next();

        let query = doc! {"name": project, "owner": &addr.user_id};
        let empty = Vec::new();
        project_metadata
            .find_one(query, None)
            .await
            .unwrap()
            .map(|metadata| {
                let role_names = role.map(|name| vec![name.to_owned()]).unwrap_or_else(|| {
                    metadata
                        .roles
                        .iter()
                        .map(|(_, role)| role.project_name.to_owned())
                        .collect()
                });

                let name2id = metadata
                    .roles
                    .into_iter()
                    .map(|(k, v)| (v.project_name, k))
                    .collect::<HashMap<String, String>>();

                role_names
                    .into_iter()
                    .filter_map(|name| name2id.get(&name))
                    .map(|role_id| BrowserAddress {
                        project_id: metadata.id.to_string(),
                        role_id: role_id.to_owned(),
                    })
                    .collect()
            })
            .unwrap_or(empty)
    }

    pub async fn send_msg(&self, msg: SendMessage) {
        let message = ClientMessage(msg.content);
        println!("received message to send to {:?}", msg.addresses);
        let recipients = join_all(
            msg.addresses
                .iter()
                .filter_map(|addr_str| ClientAddress::from_str(addr_str).ok())
                .map(|address| self.get_clients_at(address)),
        )
        .await
        .into_iter()
        .flatten();

        recipients.for_each(|client| {
            println!("Sending msg to client: {}", client.id);
            client.addr.do_send(message.clone()).unwrap();
        });
    }

    async fn set_client_state(&mut self, msg: SetClientState) {
        println!("Setting client state to {:?}", msg.state);
        self.reset_client_state(&msg.id).await;

        if !self.rooms.contains_key(&msg.state.project_id) {
            self.rooms.insert(
                msg.state.project_id.to_owned(),
                ProjectNetwork::new(msg.state.project_id.to_owned()),
            );
        }
        let room = self.rooms.get_mut(&msg.state.project_id).unwrap();
        if let Some(occupants) = room.roles.get_mut(&msg.state.role_id) {
            occupants.push(msg.id.clone());
        } else {
            room.roles
                .insert(msg.state.role_id.clone(), vec![msg.id.clone()]);
        }
        let project_id = msg.state.project_id.to_owned();
        self.states.insert(msg.id, msg.state);
        self.send_room_state_for(&project_id).await;
    }

    fn add_client(&mut self, msg: AddClient) {
        let client = Client {
            id: msg.id.clone(),
            addr: msg.addr,
        };
        self.clients.insert(msg.id, client);
    }

    async fn remove_client(&mut self, msg: RemoveClient) {
        println!("remove client");
        self.clients.remove(&msg.id);
        self.reset_client_state(&msg.id).await;
    }

    async fn reset_client_state(&mut self, id: &str) {
        let mut empty: Vec<String> = Vec::new();

        let state = self.states.remove(id);
        if state.is_none() {
            return;
        }

        let state = state.unwrap();
        let room = self.rooms.get_mut(&state.project_id);

        let mut update_needed = room.is_some();

        if let Some(room) = room {
            let occupants = room.roles.get_mut(&state.role_id).unwrap_or(&mut empty);
            if let Some(pos) = occupants.iter().position(|item| item == id) {
                occupants.swap_remove(pos);
            }

            if occupants.len() == 0 {
                let role_count = room.roles.len().clone();
                if role_count == 1 {
                    // remove the room
                    self.rooms.remove(&state.project_id);
                    // TODO: Should we remove the entry from the database?
                    update_needed = false;
                } else {
                    // remove the role
                    let room = self.rooms.get_mut(&state.role_id).unwrap();
                    room.roles.remove(&state.role_id);
                }
            }
        }

        if update_needed {
            self.send_room_state_for(&state.project_id).await;
        }
    }

    // FIXME: it might be nice not to query the database on *every* occupant invite/move/etc
    async fn send_room_state_for(&self, project_id: &str) {
        if let Some(project_metadata) = &self.project_metadata {
            let id = ObjectId::parse_str(&project_id).expect("Invalid project ID.");
            let query = doc! {"id": id};
            if let Some(project) = project_metadata.find_one(query, None).await.unwrap() {
                self.send_room_state(SendRoomState { project });
            }
        }
    }

    fn send_room_state(&self, msg: SendRoomState) {
        let id = msg.project.id.to_string();
        if let Some(room) = self.rooms.get(&id) {
            let clients = room
                .roles
                .values()
                .flatten()
                .filter_map(|id| self.clients.get(id));

            let room_state = RoomStateMessage::new(msg.project, room);
            println!("Sending room update: {}", room_state.name);
            clients.for_each(|client| {
                client.addr.do_send(room_state.clone().into());
            });
        }
    }
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
pub struct RemoveClient {
    pub id: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetClientState {
    pub id: String,
    pub state: ClientState,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct ClientState {
    role_id: String,
    project_id: String,
    username: Option<String>, // TODO: do I need this?
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SetExternalClientState {
    pub id: String,
    pub state: ExternalClientState,
}

struct BrowserAddress {
    role_id: String,
    project_id: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage {
    pub addresses: Vec<String>,
    pub content: Value,
}

#[derive(Message, Serialize, Clone)]
#[rtype(result = "()")]
struct RoomStateMessage {
    id: String,
    owner: String,
    name: String,
    roles: HashMap<String, RoleState>,
    collaborators: Vec<String>,
    version: u64,
}

impl From<RoomStateMessage> for ClientMessage {
    fn from(msg: RoomStateMessage) -> ClientMessage {
        let mut value = serde_json::to_value(msg).unwrap();
        let msg = value.as_object_mut().unwrap();
        msg.insert(
            "type".to_string(),
            serde_json::to_value("room-roles").unwrap(),
        );
        ClientMessage(value)
    }
}
#[derive(Message, Serialize, Clone)]
#[rtype(result = "()")]
struct RoleState {
    name: String,
    occupants: Vec<OccupantState>,
}

#[derive(Message, Serialize, Clone)]
#[rtype(result = "()")]
struct OccupantState {
    id: String,
    name: String,
}

impl RoomStateMessage {
    fn new(project: ProjectMetadata, room: &ProjectNetwork) -> RoomStateMessage {
        let empty = Vec::new();
        let roles: HashMap<String, RoleState> = project
            .roles
            .into_iter()
            .map(|(id, role)| {
                let client_ids = room.roles.get(&id).unwrap_or(&empty);
                // TODO: get the names...
                let occupants = client_ids
                    .into_iter()
                    .map(|id| OccupantState {
                        id: id.to_owned(),
                        name: "guest".to_owned(),
                    })
                    .collect();

                (
                    id,
                    RoleState {
                        name: role.project_name,
                        occupants,
                    },
                )
            })
            .collect();

        RoomStateMessage {
            id: room.id.to_owned(),
            owner: project.owner,
            name: project.name,
            roles,
            collaborators: project.collaborators,
            version: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not get system time")
                .as_secs(),
        }
    }
}

impl ClientState {
    pub fn new(project_id: String, role_id: String, username: Option<String>) -> ClientState {
        ClientState {
            project_id,
            role_id,
            username,
        }
    }
}

impl Handler<AddClient> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: AddClient, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.add_client(msg);
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

impl Handler<SetExternalClientState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetExternalClientState, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        // TODO:  b
        if let Some(network) = topology.external.get_mut(&msg.state.app_id) {
            network.set_client_state(&msg.id, msg.state);
        }
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
