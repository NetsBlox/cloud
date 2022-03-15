pub mod topology;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::OccupantInvite;
use crate::network::topology::{ClientState, ExternalClientState};
use crate::projects::{ensure_can_edit_project, ensure_can_edit_project_id};
use crate::users::{ensure_can_edit_user, ensure_is_super_user};
use actix::{Actor, Addr, AsyncContext, Handler, StreamHandler};
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, CloseCode};
use mongodb::bson::doc;
use netsblox_core::{BrowserClientState, ClientID, ClientStateData, OccupantInviteReq, ProjectId};
use serde::Deserialize;
use serde_json::Value;

pub type AppID = String;

#[post("/{client}/state")] // TODO: add token here (in a header), too?
async fn set_client_state(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    body: web::Json<ClientStateData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: should we allow users to set the client state for some other user?
    let username = session.get::<String>("username").unwrap();
    let (client_id,) = path.into_inner();
    if !client_id.starts_with('_') {
        return Err(UserError::InvalidClientIdError);
    }

    // TODO: Check that the user can set the state to the given value
    // User needs to either be able to edit the project or use a token
    // In other words, there are 2 things that need to be verified:
    //   - the request can edit the client (ie, secret/signed token or something)
    //   - the user can join the project. May need a token if invited as occupant

    let mut response = None;

    let state = match body.into_inner().state {
        ClientState::External(client_state) => {
            // append the user ID to the address

            let user_id = username.as_ref().unwrap_or_else(|| &client_id).to_owned();
            let address = format!("{}@{}", client_state.address, user_id);
            let app_id = client_state.app_id;
            if app_id.to_lowercase() == topology::DEFAULT_APP_ID {
                // TODO: make AppID a type
                return Err(UserError::InvalidAppIdError);
            }

            response = Some(address.clone());
            ClientState::External(ExternalClientState { address, app_id })
        }
        ClientState::Browser(client_state) => {
            let query = doc! {"id": &client_state.project_id};
            let metadata = app
                .project_metadata
                .find_one(query, None)
                .await
                .map_err(|_err| InternalError::DatabaseConnectionError)?
                .ok_or_else(|| UserError::ProjectNotFoundError)?;

            ensure_can_edit_project(&app, &session, Some(client_id.clone()), &metadata).await?;
            ClientState::Browser(client_state)
        }
    };

    app.network.do_send(topology::SetClientState {
        id: client_id,
        state,
        username,
    });

    Ok(HttpResponse::Ok().body(response.unwrap_or_else(String::new)))
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

#[post("/id/{projectID}/occupants/invite")]
async fn invite_occupant(
    app: web::Data<AppData>,
    body: web::Json<OccupantInviteReq>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    ensure_can_edit_project_id(&app, &session, None, &project_id).await?;

    let invite = OccupantInvite::new(project_id, body.into_inner());
    app.occupant_invites
        .insert_one(invite, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?;

    Ok(HttpResponse::Ok().body("Invitation sent!"))
}

#[post("/clients/{clientID}/evict")]
async fn evict_occupant(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ClientID,)>,
) -> Result<HttpResponse, UserError> {
    let (client_id,) = path.into_inner();

    ensure_can_evict_client(&app, &session, &client_id).await?;

    app.network.do_send(topology::EvictOccupant { client_id });

    Ok(HttpResponse::Ok().body("Evicted!"))
}

async fn ensure_can_evict_client(
    app: &AppData,
    session: &Session,
    client_id: &str,
) -> Result<(), UserError> {
    let client_state = app
        .network
        .send(topology::GetClientState {
            client_id: client_id.to_owned(),
        })
        .await
        .map_err(|_err| UserError::InternalError)?
        .0;

    // Client can be evicted by project owners, collaborators
    if let Some(ClientState::Browser(BrowserClientState { project_id, .. })) = client_state {
        let can_edit = ensure_can_edit_project_id(app, session, None, &project_id).await;
        if can_edit.is_ok() {
            return Ok(());
        }
    }

    // or by anyone who can edit the corresponding user
    let client_username = app
        .network
        .send(topology::GetClientUsername {
            client_id: client_id.to_owned(),
        })
        .await
        .map_err(|_err| UserError::InternalError)?
        .0;

    match client_username {
        Some(username) => ensure_can_edit_user(app, session, &username).await,
        None => Err(UserError::PermissionsError), // TODO: allow guest to evict self?
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(set_client_state)
        .service(connect_client)
        .service(get_external_clients)
        .service(get_room_state)
        .service(get_rooms)
        .service(invite_occupant)
        .service(evict_occupant);
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
                        .into_iter()
                        .filter_map(|v| match v {
                            Value::String(v) => Some(v),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    Value::String(value) => vec![value],
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
