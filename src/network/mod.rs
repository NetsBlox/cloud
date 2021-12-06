pub mod topology;
use topology::client::Client;

use actix::prelude::*;
use actix::{Actor, StreamHandler, Addr,AsyncContext,Handler};
use crate::app_data::AppData;
use actix_web_actors::ws;
use actix_web::{web, HttpResponse, HttpRequest, Error};
use actix_web::{post};
use serde::Deserialize;
use serde_json::Value;

// TODO: how to handle pyblocks connections?
//}
// TODO: add a secret along with client ID

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct SetClientState {
    pub client_id: String,
    pub role_id: String,
    pub project_id: String,
    pub token: Option<String>,
}

#[post("/{client}/state")]
async fn set_client_state(data: web::Data<AppData>, req: web::Json<SetClientState>) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate client secret
    let username = None;  // FIXME
    let state = topology::ClientState::new(req.project_id.clone(), req.role_id.clone(), username);
    data.network.do_send(topology::SetClientState{id: req.client_id.clone(), state});
    // TODO: look up the username
    // TODO: add a client secret for access control?
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
    channels: std::vec::Vec<ChannelType>,
}

#[post("/connect")]
async fn connect_client(data: web::Data<AppData>, req: HttpRequest, stream: web::Payload, state: web::Json<ConnectClientBody>) -> Result<HttpResponse, Error> {
    // TODO: ensure ID is unique?
    let handler = WsSession{
        client_id: state.id.clone(),
        topology_addr: data.network.clone(),
    };
    let resp = ws::start(handler, &req, stream);
    //let mut topology = data.network.lock().unwrap();
    //topology.add_client(client);
    resp
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(set_client_state)
        .service(connect_client);
}

struct WsSession {
    client_id: String,
    topology_addr: Addr<topology::Topology>,
}

//impl WsSession {
    //pub fn new(client_id: String) -> WsSession {
        //WsSession{client_id}
    //}
//}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.topology_addr
            .do_send(topology::AddClient {
                id: self.client_id.clone(),
                addr: addr.recipient(),
            });
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // TODO: wait a little bit?
        self.topology_addr
            .do_send(topology::RemoveClient {
                id: self.client_id.clone(),
            });
        actix::Running::Stop
    }

}

impl Handler<topology::ClientMessage> for WsSession {
    type Result = ();
    fn handle(&mut self, msg: topology::ClientMessage, ctx: &mut Self::Context) {
        ctx.text(msg.data.to_string());
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();  // FIXME
                println!("received {} message", v["type"]);
                if let Value::String(msg_type) = &v["type"] {
                    println!("message type is {}", msg_type);
                    // TODO: send a message to the client?
                    //let mut client = self.client.lock().unwrap();
                    //client.handle_msg(msg_type.to_string(), v);
                } else {
                    println!("Unexpected message type");
                }
            },
            _ => (),
        }
    }
}
#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;

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
