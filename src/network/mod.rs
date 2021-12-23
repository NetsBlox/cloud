pub mod topology;

use crate::app_data::AppData;
use actix::{Actor, Addr, AsyncContext, Handler, StreamHandler};
use actix_web::{delete, get, post};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::Deserialize;
use serde_json::Value;

// TODO: how to handle pyblocks connections?
// TODO: add support for another type of state?

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetClientState {
    pub client_id: String,
    pub role_id: String,
    pub project_id: String,
    pub token: Option<String>, // TODO: token for accessing the project; secret for controlling client
}

#[post("/{client}/state")]
async fn set_client_state(
    data: web::Data<AppData>,
    req: web::Json<SetClientState>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate client secret
    let username = None; // FIXME
    let state = topology::ClientState::new(req.project_id.clone(), req.role_id.clone(), username);
    data.network.do_send(topology::SetClientState {
        id: req.client_id.clone(),
        state,
    });
    // TODO: look up the username
    // TODO: add a client secret (and token) for access control?
    unimplemented!();
}

#[derive(Deserialize)]
enum ChannelType {
    Messages,
    Edits,
}

#[derive(Deserialize)]
struct ConnectClientBody {
    id: String,
    secret: String,
}

#[post("/connect")]
async fn connect_client(
    data: web::Data<AppData>,
    req: HttpRequest,
    stream: web::Payload,
    state: web::Json<ConnectClientBody>,
) -> Result<HttpResponse, Error> {
    // TODO: validate client secret?
    // TODO: ensure ID is unique?
    let handler = WsSession {
        client_id: state.id.clone(),
        topology_addr: data.network.clone(),
    };
    let resp = ws::start(handler, &req, stream);
    resp
}

#[get("/id/{projectID}/occupants/")]
async fn list_occupants() -> Result<HttpResponse, std::io::Error> {
    // TODO: should this go to the network category?
    todo!();
}

#[delete("/id/{projectID}/occupants/{clientID}")]
async fn remove_occupant() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OccupantInvite {
    username: String,
    role_id: String,
}

#[post("/id/{projectID}/occupants/invite")] // TODO: add role ID
async fn invite_occupant(
    invite: web::Json<OccupantInvite>,
) -> Result<HttpResponse, std::io::Error> {
    todo!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(set_client_state).service(connect_client);
}

struct WsSession {
    client_id: String,
    topology_addr: Addr<topology::Topology>,
}

impl WsSession {
    pub fn handle_msg(&self, msg_type: &str, msg: Value) {
        // TODO: handle message from client
        match msg_type {
            "message" => {
                let dst_id = msg["dstId"].clone();
                let addresses = match dst_id {
                    Value::String(address) => vec![address],
                    Value::Array(values) => values
                        .iter()
                        .filter(|v| v.is_string())
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>(),
                    _ => std::vec::Vec::new(),
                };
                self.topology_addr.do_send(topology::SendMessage {
                    addresses,
                    content: msg,
                });
            }
            "client-message" => { // combine this with the above type?
            }
            "user-action" => {
                // TODO: Record
            }
            "project-response" => { // TODO: move this to rest?
            }
            "request-actions" => { // TODO: move this to REST?
            }
            _ => {
                println!("unrecognized message type: {}", msg_type);
            }
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.topology_addr.do_send(topology::AddClient {
            id: self.client_id.clone(),
            addr: addr.recipient(),
        });
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // TODO: wait a little bit?
        self.topology_addr.do_send(topology::RemoveClient {
            id: self.client_id.clone(),
        });
        actix::Running::Stop
    }
}

impl Handler<topology::ClientMessage> for WsSession {
    type Result = ();
    fn handle(&mut self, msg: topology::ClientMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0.to_string());
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap(); // FIXME
                println!("received {} message", v["type"]);
                if let Value::String(msg_type) = &v["type"] {
                    self.handle_msg(&msg_type.to_string(), v);
                } else {
                    println!("Unexpected message type");
                }
            }
            _ => (),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[actix_web::test]
    async fn test_connect_client() {
        // TODO: send a connect request and check that the client has been added to the topology
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_send_msg_room() {
        //let client = Client::new("test".to_string());
        //let msg = json!({"type": "message", "dstId": "project@owner"});
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_send_msg_role() {
        //let client = Client::new("test".to_string());
        //let msg = json!({"type": "message", "dstId": "role1@project@owner"});
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_send_msg_list() {
        //let client = Client::new("test".to_string());
        //let msg = json!({"type": "message", "dstId": ["role1@project@owner"]});
        //client.handle_msg(msg);
        unimplemented!();
    }
}
