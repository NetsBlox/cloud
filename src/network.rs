use actix::{Actor, StreamHandler};
use actix_web_actors::ws;
use actix_web::{web, HttpResponse, HttpRequest, Error};
use actix_web::{post};
use serde::Deserialize;
use serde_json::Value;

// Functionality:
//   - 
#[derive(Deserialize)]
struct ClientState {
    role: String,
    project: String,
    owner: String,
    token: Option<String>,
}

#[post("/{client}/state")]
async fn set_client_state(state: web::Json<ClientState>) -> Result<HttpResponse, std::io::Error> {
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
async fn connect_client(req: HttpRequest, stream: web::Payload, state: web::Json<ConnectClientBody>) -> Result<HttpResponse, Error> {
    let resp = ws::start(Client::new(state.id.clone()), &req, stream);
    resp
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(set_client_state);
}

// Websocket support
struct Client {
    id: String,
}

impl Client {
    pub fn new(id: String) -> Client {
        Client{id}
    }

    fn handle_msg(&self, msg_type: String, msg: Value, ctx: &mut <Client as Actor>::Context) {
        match msg_type.as_str() {
            "message" => {
            },
            "client-message" => {  // combine this with the above type?
            },
            "user-action" => {
            },
            "project-response" => {  // TODO: move this to rest?
            },
            "request-actions" => {  // TODO: move this to REST?
            },
            _ => {
                println!("unrecognized message type: {}", msg_type);
            }
        }
    }
}

impl Actor for Client {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Client {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                println!("text: {}", text);
                let v: Value = serde_json::from_str(&text).unwrap();  // FIXME
                println!("received {} message", v["type"]);
                if let Value::String(msg_type) = &v["type"] {
                    println!("message type is {}", msg_type);
                    self.handle_msg(msg_type.to_string(), v, ctx);
                } else {
                    println!("Unexpected message type");
                }
            },
            _ => (),
        }
    }
}
