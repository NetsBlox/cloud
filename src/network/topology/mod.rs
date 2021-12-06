pub mod client;

use actix::{Actor,Context,Handler};
use actix::prelude::{Message,Recipient};
use client::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct Topology {
    clients: std::vec::Vec<Client>,
}

impl Topology {
    pub fn new() -> Topology {
        Topology{clients: std::vec::Vec::new()}
    }

    // TODO: what methods do we need here for sending messages?
    // TODO: how do we need to be able to access the clients?
    pub fn add_client(&mut self, client: Client) {
        self.clients.push(client);
    }
}

impl Actor for Topology {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result="()")]
pub struct ClientMessage {
    pub data: Value,  // TODO: define a trait for converting to a JSON message?
}

#[derive(Message)]
#[rtype(result="()")]
pub struct AddClient {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

#[derive(Message)]
#[rtype(result="()")]
pub struct RemoveClient {
    pub id: String,
}

#[derive(Message)]
#[rtype(result="()")]
pub struct SetClientState {
    pub id: String,
    pub state: ClientState,
}

#[derive(Message)]
#[rtype(result="()")]
pub struct ClientState {
    role_id: String,
    project_id: String,
    username: Option<String>,
}

#[derive(Message)]
#[rtype(result="()")]
pub struct SendMessage {
    pub address: String,
    pub content: Value,
}

impl ClientState {
    pub fn new(project_id: String, role_id: String, username: Option<String>) -> ClientState {
        ClientState{project_id, role_id, username}
    }
}

impl Handler<AddClient> for Topology {
    type Result = ();

    fn handle(&mut self, msg: AddClient, _: &mut Context<Self>) -> Self::Result {
        unimplemented!();
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
        unimplemented!();
    }
}

impl Handler<SendMessage> for Topology {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, _: &mut Context<Self>) -> Self::Result {
        // TODO: resolve the address? Or should it already be resolved?
        unimplemented!();
    }
}
