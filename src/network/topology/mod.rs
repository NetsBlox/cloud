pub mod client;

use actix::prelude::{Message, Recipient};
use actix::{Actor, AsyncContext, Context, Handler};
use futures::future::join_all;
use mongodb::bson::doc;
use mongodb::Collection;
use serde_json::Value;
use std::collections::HashMap;

use crate::models::ProjectMetadata;

#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

struct ProjectNetwork {
    roles: HashMap<String, Vec<String>>,
}

pub struct Topology {
    //clients: HashMap<String, Client>,
    project_metadata: Collection<ProjectMetadata>,
    clients: HashMap<String, Client>,
    rooms: HashMap<String, ProjectNetwork>,
}

impl Topology {
    pub fn new(project_metadata: Collection<ProjectMetadata>) -> Topology {
        Topology {
            //clients: HashMap::new(),
            clients: HashMap::new(),
            project_metadata,
            rooms: HashMap::new(),
        }
    }

    pub async fn get_clients_at(&self, addr: &str) -> Vec<&Client> {
        // TODO: Add support for third party clients
        // How should they be addressed?
        let addresses = ClientAddress::parse(&self.project_metadata, addr).await;
        let empty = Vec::new();
        let clients = addresses
            .into_iter()
            .flat_map(|addr| {
                self.rooms
                    .get(&addr.project_id)
                    .and_then(|room| room.roles.get(&addr.role_id))
                    .unwrap_or(&empty)
            })
            .map(|id| self.clients.get(id))
            .filter(|client| client.is_some())
            .map(|client| client.unwrap())
            .collect();

        return clients;
    }

    pub async fn route_msg(&self, msg: SendMessage) {
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
}

impl Actor for Topology {
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
pub struct RemoveClient {
    pub id: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetClientState {
    pub id: String,
    pub state: ClientState,
}

#[derive(Message)]
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

impl Handler<AddClient> for Topology {
    type Result = ();

    fn handle(&mut self, msg: AddClient, _: &mut Context<Self>) -> Self::Result {
        let client = Client {
            id: msg.id,
            addr: msg.addr,
        };
        self.clients.insert(msg.id, client);
    }
}

impl Handler<RemoveClient> for Topology {
    type Result = ();

    fn handle(&mut self, msg: RemoveClient, _: &mut Context<Self>) -> Self::Result {
        unimplemented!();
    }
}

impl Handler<SetClientState> for Topology {
    type Result = ();

    fn handle(&mut self, msg: SetClientState, _: &mut Context<Self>) -> Self::Result {
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
            occupants.push(msg.id);
        } else {
            room.roles.insert(msg.state.role_id, vec![msg.id]);
        }
    }
}

impl Handler<SendMessage> for Topology {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, ctx: &mut Context<Self>) -> Self::Result {
        //let fut = async move { self.route_msg(msg).await };
        let fut = self.route_msg(msg);
        let fut = actix::fut::wrap_future(fut); // Darn this won't work...
        ctx.spawn(fut);
        //ctx.spawn(fut);
        // TODO: resolve the address? Or should it already be resolved?
    }
}
