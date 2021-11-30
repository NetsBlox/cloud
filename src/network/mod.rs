pub mod topology;
use topology::client::Client;

use std::sync::{Arc,Mutex};
use actix::{Actor, StreamHandler};
use crate::app_data::AppData;
use actix_web_actors::ws;
use actix_web::{web, HttpResponse, HttpRequest, Error};
use actix_web::{post};
use serde::Deserialize;
use serde_json::Value;

// TODO: how to handle pyblocks connections?
//}

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct ClientState {
    role_id: String,
    project_id: String,
    owner: String,
    token: Option<String>,
}

#[post("/{client}/state")]
async fn set_client_state(data: web::Data<AppData>, state: web::Json<ClientState>) -> Result<HttpResponse, std::io::Error> {
    // TODO: look up the client
    // TODO: update the client's state
    // TODO: add a client secret for access control
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
    // TODO: pass an Arc<Mutex<Client>> to the ws handler?
    let client = Arc::new(Mutex::new(Client::new(state.id.clone())));
    let handler = ClientMessageHandler{client};
    let resp = ws::start(handler, &req, stream);
    let mut topology = data.network.lock().unwrap();
    topology.add_client(client);
    resp
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(set_client_state)
        .service(connect_client);
}

struct ClientMessageHandler {
    client: Arc<Mutex<Client>>,
}

impl Actor for ClientMessageHandler {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ClientMessageHandler {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap();  // FIXME
                println!("received {} message", v["type"]);
                if let Value::String(msg_type) = &v["type"] {
                    println!("message type is {}", msg_type);
                    let mut client = self.client.lock().unwrap();
                    client.handle_msg(msg_type.to_string(), v);
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
