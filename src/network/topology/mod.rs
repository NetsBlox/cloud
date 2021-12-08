pub mod client;

use actix::{Actor,Context,Handler};
use actix::prelude::{Message,Recipient};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub addr: Recipient<ClientMessage>,
}

#[derive(Clone)]
pub struct Topology {
    clients: HashMap<String,Client>,
}

impl Topology {
    pub fn new() -> Topology {
        Topology{clients: HashMap::new()}
    }

    pub fn resolve_address(&self, addr: String) -> Vec<&Client> {
        unimplemented!();
    }
}

impl Actor for Topology {
    type Context = Context<Self>;
}

#[derive(Message,Clone)]
#[rtype(result="()")]
pub struct ClientMessage (pub Value);

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

    fn handle(&mut self, msg: SendMessage, ctx: &mut Context<Self>) -> Self::Result {
        let message = ClientMessage(msg.content);
        let recipients = self.resolve_address(msg.address);
        recipients.iter().for_each(|client| {
            client.addr.do_send(message.clone());
        });
        // TODO: resolve the address? Or should it already be resolved?
    }
}
