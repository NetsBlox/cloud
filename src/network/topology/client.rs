use actix::prelude::{Message, Recipient};
use actix_web::rt::time::sleep;
use lazy_static::lazy_static;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};
use uuid::Uuid;

use super::{BrowserClientState, ClientMessage};
use crate::{errors::InternalError, models::RoleData};

lazy_static! {
    pub static ref RESPONSE_BUFFER: Arc<RwLock<HashMap<Uuid, RoleDataResponseState>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

pub type ClientID = String; // TODO: use this everywhere

#[derive(Clone)]
struct RoleRequestMessage(pub Uuid);

impl From<RoleRequestMessage> for ClientMessage {
    fn from(msg: RoleRequestMessage) -> ClientMessage {
        ClientMessage(json!({"type": "role-data-request", "id": msg.0.to_string()}))
    }
}

#[derive(Debug, Clone)]
pub enum RoleDataResponseState {
    Pending, // TODO: add a token and the client state?
    Data(RoleData),
}

#[derive(Clone, Debug)]
pub struct Client {
    pub id: ClientID,
    pub addr: Recipient<ClientMessage>,
}

impl Client {
    pub fn new(id: ClientID, addr: Recipient<ClientMessage>) -> Self {
        Client { id, addr }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RoleRequest {
    addr: Recipient<ClientMessage>,
    state: BrowserClientState,
}

impl RoleRequest {
    pub fn new(addr: Recipient<ClientMessage>, state: BrowserClientState) -> Self {
        // TODO: add support for sending to multiple clients?
        RoleRequest {
            addr,
            state: state.clone(),
        }
    }

    fn initialize_response(&self, id: Uuid) {
        let mut responses = RESPONSE_BUFFER.write().unwrap();
        responses.insert(id, RoleDataResponseState::Pending);
    }

    fn has_response(&self, id: &Uuid) -> bool {
        let responses = RESPONSE_BUFFER.read().unwrap();
        match responses.get(&id) {
            Some(RoleDataResponseState::Data(_)) => true,
            _ => false,
        }
    }

    fn retrieve(&self, id: &Uuid) -> Option<RoleData> {
        let mut responses = RESPONSE_BUFFER.write().unwrap();
        responses.remove(&id).and_then(|state| match state {
            RoleDataResponseState::Data(role) => Some(role),
            _ => None,
        })
    }

    pub async fn send(self) -> Result<RoleData, InternalError> {
        let id = Uuid::new_v4();
        self.initialize_response(id.clone());

        // send the message
        let message = RoleRequestMessage(id);
        self.addr.do_send(message.into()).unwrap();

        // poll the inbox
        let max_wait_ms = Duration::from_millis(5000);
        let start_time = SystemTime::now();

        while start_time.elapsed().unwrap() < max_wait_ms {
            sleep(Duration::from_millis(250)).await;
            if self.has_response(&id) {
                break;
            }
        }

        // TODO: what if the requested project_id, role_id are not what we receive (race condition)
        self.retrieve(&id)
            .ok_or_else(|| InternalError::TimeoutError)
    }
}
