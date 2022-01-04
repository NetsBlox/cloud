pub mod client;

use actix::prelude::{Message, Recipient};
use actix::{Actor, AsyncContext, Context, Handler};
use futures::future::join_all;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::Collection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::models::ProjectMetadata;

#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

struct ProjectNetwork {
    roles: HashMap<String, Vec<String>>,
}

struct Topology {
    project_metadata: Option<Collection<ProjectMetadata>>,
    clients: HashMap<String, Client>,
    rooms: HashMap<String, ProjectNetwork>,
    states: HashMap<String, ClientState>,
}

// TODO: wrap this in a mutex or something?
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
        }
    }

    fn set_project_metadata(&mut self, project_metadata: Collection<ProjectMetadata>) {
        self.project_metadata = Some(project_metadata);
    }

    async fn get_clients_at(&self, addr: &str) -> Vec<&Client> {
        // TODO: Add support for third party clients
        // How should they be addressed?
        if let Some(project_metadata) = &self.project_metadata {
            let addresses = ClientAddress::parse(&project_metadata, addr).await;
            let empty = Vec::new();
            let clients: Vec<&Client> = addresses
                .into_iter()
                .flat_map(|addr| {
                    self.rooms
                        .get(&addr.project_id)
                        .and_then(|room| room.roles.get(&addr.role_id))
                        .unwrap_or(&empty)
                })
                .filter_map(|id| self.clients.get(id))
                .collect();

            return clients;
        } else {
            return Vec::new();
        }
    }

    pub async fn send_msg(&self, msg: SendMessage) {
        let message = ClientMessage(msg.content);
        println!("received message to send to {:?}", msg.addresses);
        let recipients = join_all(
            msg.addresses
                .iter()
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

    fn set_client_state(&mut self, msg: SetClientState) {
        if let Some(state) = self.states.remove(&msg.id) {
            self.remove_client_from(&msg.id, &state);
        }

        println!("Setting client state to {:?}", msg.state);
        if !self.rooms.contains_key(&msg.state.project_id) {
            self.rooms.insert(
                msg.state.project_id.to_owned(),
                ProjectNetwork {
                    roles: HashMap::new(),
                },
            );
        }
        let room = self.rooms.get_mut(&msg.state.project_id).unwrap();
        if let Some(occupants) = room.roles.get_mut(&msg.state.role_id) {
            occupants.push(msg.id.clone());
        } else {
            room.roles
                .insert(msg.state.role_id.clone(), vec![msg.id.clone()]);
        }
        self.states.insert(msg.id, msg.state);
    }

    fn add_client(&mut self, msg: AddClient) {
        let client = Client {
            id: msg.id.clone(),
            addr: msg.addr,
        };
        self.clients.insert(msg.id, client);
    }

    fn remove_client(&mut self, msg: RemoveClient) {
        println!("remove client");
        self.clients.remove(&msg.id);

        if let Some(state) = self.states.remove(&msg.id) {
            self.remove_client_from(&msg.id, &state);
        }
    }

    fn remove_client_from(&mut self, id: &str, state: &ClientState) {
        let mut empty: Vec<String> = Vec::new();
        let occupants_and_role_count = self
            .states
            .remove(id)
            .and_then(|state| self.rooms.get_mut(&state.project_id))
            .map(|room| {
                (
                    room.roles.len().clone(),
                    room.roles.get_mut(&state.role_id).unwrap_or(&mut empty),
                )
            });

        if let Some((mut role_count, occupants)) = occupants_and_role_count {
            if let Some(pos) = occupants.iter().position(|item| item == id) {
                occupants.swap_remove(pos);
                role_count -= 1;
            }
            if occupants.len() == 0 {
                if role_count == 0 {
                    // remove the room
                    self.rooms.remove(&state.project_id);
                } else {
                    // remove the role
                    let room = self.rooms.get_mut(&state.role_id).unwrap();
                    room.roles.remove(&state.role_id);
                }
            }
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
    username: Option<String>,
}

pub struct ClientAddress {
    project_id: String,
    role_id: String,
}

impl ClientAddress {
    pub async fn parse(project_metadata: &Collection<ProjectMetadata>, addr: &str) -> Vec<Self> {
        let mut chunks = addr.split('@').rev();
        let owner = chunks.next().unwrap(); // FIXME: Better feedback for devs
        let project = chunks.next().unwrap();
        let role = chunks.next();
        let mut states = vec![];
        let query = doc! {"name": project, "owner": owner};
        if let Some(metadata) = project_metadata.find_one(query, None).await.unwrap() {
            let project_id = metadata._id.to_string();
            match role {
                Some(role_name) => {
                    let name2id = metadata
                        .roles
                        .into_iter()
                        .map(|(k, v)| (v.project_name, k))
                        .collect::<HashMap<String, String>>();

                    match name2id.get(role_name) {
                        Some(role_id) => {
                            let state = ClientAddress {
                                role_id: role_id.to_owned(),
                                project_id,
                            };
                            states.push(state);
                        }
                        None => {
                            todo!(); // TODO: Log an error
                        }
                    }
                }
                None => metadata
                    .roles
                    .into_keys()
                    .map(|role_id| ClientAddress {
                        role_id,
                        project_id: project_id.clone(),
                    })
                    .for_each(|state| states.push(state)),
            }
        }
        states
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage {
    pub addresses: Vec<String>,
    pub content: Value,
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

    fn handle(&mut self, msg: RemoveClient, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.remove_client(msg);
    }
}

impl Handler<SetClientState> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetClientState, _: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.set_client_state(msg);
    }
}

impl Handler<SendMessage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Context<Self>) -> Self::Result {
        let fut = async {
            let topology = TOPOLOGY.read().unwrap();
            topology.send_msg(msg).await;
        };
        let fut = actix::fut::wrap_future(fut); // Darn this won't work...
        ctx.spawn(fut);
    }
}
impl Handler<SetStorage> for TopologyActor {
    type Result = ();

    fn handle(&mut self, msg: SetStorage, ctx: &mut Context<Self>) -> Self::Result {
        let mut topology = TOPOLOGY.write().unwrap();
        topology.set_project_metadata(msg.project_metadata);
    }
}
