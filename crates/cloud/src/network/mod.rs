pub mod topology;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::network::topology::{ClientState, ExternalClientState};
use crate::projects::ensure_can_edit_project;
use crate::users::ensure_is_super_user;
use actix::{Actor, Addr, AsyncContext, Handler, StreamHandler};
use actix_session::Session;
use actix_web::{delete, get, post};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, CloseCode};
use mongodb::bson::doc;
use netsblox_core::{ClientStateData, ProjectId};
use serde::Deserialize;
use serde_json::Value;

pub type AppID = String;

#[post("/{client}/state")] // TODO: add token here, too
async fn set_client_state(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    body: web::Json<ClientStateData>,
    session: Session,
) -> HttpResponse {
    let username = session.get::<String>("username").unwrap();
    let (client_id,) = path.into_inner();
    if !client_id.starts_with('_') {
        return HttpResponse::BadRequest().body("Invalid client ID.");
    }

    // TODO: Check that the user can set the state to the given value
    // User needs to either be able to edit the project or use a token
    // In other words, there are 2 things that need to be verified:
    //   - the request can edit the client (ie, secret/signed token or something)
    //   - the user can join the project. May need a token if invited as occupant

    let mut response = None;
    let mut state = body.into_inner().state;
    if let ClientState::External(client_state) = state {
        let user_id = username.as_ref().unwrap_or_else(|| &client_id).to_owned();
        let address = format!("{}@{}", client_state.address, user_id);
        let app_id = client_state.app_id;
        if app_id.to_lowercase() == topology::DEFAULT_APP_ID {
            // TODO: make AppID a type
            return HttpResponse::BadRequest().body("Invalid App ID.");
        }

        response = Some(address.clone());
        state = ClientState::External(ExternalClientState { address, app_id });
    };

    println!("setting state {:?} {:?}", &state, &username);
    app.network.do_send(topology::SetClientState {
        id: client_id,
        state,
        username,
    });

    HttpResponse::Ok().body(response.unwrap_or_else(String::new))
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

#[get("/{client}/connect")]
async fn connect_client(
    data: web::Data<AppData>,
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(String,)>,
    //body: web::Json<ConnectClientBody>,
) -> Result<HttpResponse, Error> {
    // TODO: validate client secret?
    // TODO: ensure ID is unique?
    let (client_id,) = path.into_inner();
    let handler = WsSession {
        client_id,
        topology_addr: data.network.clone(),
    };
    ws::start(handler, &req, stream)
}

#[get("/id/{projectID}")]
async fn get_room_state(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    let state = app
        .network
        .send(topology::GetRoomState { metadata })
        .await
        .map_err(|_err| UserError::InternalError)?
        .0
        .ok_or_else(|| UserError::ProjectNotActiveError)?;

    Ok(HttpResponse::Ok().json(state))
}

#[get("/")]
async fn get_rooms(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let state = app
        .network
        .send(topology::GetActiveRooms {})
        .await
        .map_err(|_err| UserError::InternalError)?
        .0;

    Ok(HttpResponse::Ok().json(state))
}

#[get("/external")]
async fn get_external_clients(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let clients = app
        .network
        .send(topology::GetExternalClients {})
        .await
        .map_err(|_err| UserError::InternalError)?
        .0;

    Ok(HttpResponse::Ok().json(clients))
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
    token: String, // TODO
}

#[post("/id/{projectID}/occupants/invite")] // TODO: add role ID
async fn invite_occupant(
    invite: web::Json<OccupantInvite>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: generate a token for the user?
    // TODO: these are probably fine to be transient
    todo!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(set_client_state)
        .service(connect_client)
        .service(get_external_clients)
        .service(get_room_state)
        .service(get_rooms);
}

struct WsSession {
    client_id: String,
    topology_addr: Addr<topology::TopologyActor>,
}

impl WsSession {
    pub fn handle_msg(&self, msg_type: &str, msg: Value, ctx: &mut <WsSession as Actor>::Context) {
        if msg_type != "ping" {
            println!("received {} message", msg_type);
        }
        match msg_type {
            "message" => {
                let dst_id = msg["dstId"].clone();
                let addresses = match dst_id {
                    Value::Array(values) => values
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                        .collect::<Vec<_>>(),
                    _ => vec![],
                };
                println!("Sending message to {:?}", addresses);
                self.topology_addr.do_send(topology::SendMessage {
                    addresses,
                    content: msg,
                });
            }
            "client-message" => { // combine this with the above type?
            }
            "user-action" => {
                // TODO: Record: Can we get rid of these?
            }
            "request-actions" => { // TODO: move this to REST?
            }
            "ping" => ctx.text("{\"type\": \"pong\"}"),
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
                if let Value::String(msg_type) = &v["type"] {
                    self.handle_msg(&msg_type.clone(), v, ctx);
                } else {
                    println!("Unexpected message type");
                }
            }
            Ok(ws::Message::Close(reason_opt)) => {
                println!("Closing! Reason: {:?}", &reason_opt);
                let is_broken = reason_opt
                    .map(|reason| match reason.code {
                        CloseCode::Normal | CloseCode::Away => false,
                        _ => true,
                    })
                    .unwrap_or(true);

                if is_broken {
                    self.topology_addr.do_send(topology::BrokenClient {
                        id: self.client_id.clone(),
                    });
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
        todo!();
    }

    #[actix_web::test]
    async fn test_send_msg_room() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": "project@owner"});
        todo!();
    }

    #[actix_web::test]
    async fn test_send_msg_list() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": ["role1@project@owner"]});
        //client.handle_msg(msg);
        todo!();
    }

    #[actix_web::test]
    async fn test_connect_invalid_client_id() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": "role1@project@owner"});

        todo!();
    }
}
