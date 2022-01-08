mod address;
mod external;
mod network;

use crate::models::ProjectMetadata;
use actix::prelude::{Message, Recipient};
use actix::{Actor, AsyncContext, Context, Handler};
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::Collection;
use serde_json::Value;
use std::sync::{Arc, RwLock};

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
