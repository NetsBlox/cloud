pub mod topology;

use std::time::SystemTime;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::{NetworkTraceMetadata, OccupantInvite, SentMessage};
use crate::network::topology::{ClientState, ExternalClientState};
use crate::projects::{ensure_can_edit_project, ensure_can_view_project};
use crate::services::ensure_is_authorized_host;
use crate::users::{ensure_can_edit_user, ensure_is_super_user};
use actix::{Actor, Addr, AsyncContext, Handler, StreamHandler};
use actix_session::Session;
use actix_web::{delete, get, post};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, CloseCode};
use futures::TryStreamExt;
use mongodb::bson::{doc, DateTime};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use netsblox_core::{
    BrowserClientState, ClientID, ClientStateData, OccupantInviteData, ProjectId, SaveState,
};
use serde::Deserialize;
use serde_json::{json, Value};
use topology::ClientCommand;

pub type AppID = String;

#[post("/{client}/state")] // TODO: add token here (in a header), too?
async fn set_client_state(
    app: web::Data<AppData>,
    path: web::Path<(ClientID,)>,
    body: web::Json<ClientStateData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: should we allow users to set the client state for some other user?
    let username = session.get::<String>("username").unwrap();
    let (client_id,) = path.into_inner();
    if !client_id.as_str().starts_with('_') {
        // TODO: move this to the struct parsing
        return Err(UserError::InvalidClientIdError);
    }

    let mut response = None;

    let state = match body.into_inner().state {
        ClientState::External(client_state) => {
            // append the user ID to the address
            let client_id_string = client_id.as_str().to_string();
            let user_id = username.as_ref().unwrap_or(&client_id_string).to_owned();
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
            let metadata = ensure_can_view_project(
                &app,
                &session,
                Some(client_id.clone()),
                &client_state.project_id,
            )
            .await?;

            let query = doc! {
                "id": &metadata.id,
                "saveState": SaveState::CREATED
            };
            let update = doc! {
                "$set": {
                    "saveState": SaveState::TRANSIENT
                },
                "$unset": {
                    "deleteAt": 1
                }
            };
            app.project_metadata
                .update_one(query, update, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            ClientState::Browser(client_state)
        }
    };

    app.network.do_send(topology::SetClientState {
        id: client_id,
        state,
        username,
    });

    Ok(HttpResponse::Ok().body(response.unwrap_or_default()))
}

#[derive(Deserialize)]
enum ChannelType {
    Messages,
    Edits,
}

#[get("/{client}/connect")]
async fn connect_client(
    app: web::Data<AppData>,
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(ClientID,)>,
) -> Result<HttpResponse, UserError> {
    // TODO: validate client secret?
    let (client_id,) = path.into_inner();

    if !client_id.as_str().starts_with('_') {
        return Err(UserError::InvalidClientIdError);
    }

    // close any existing client with the same ID
    app.network
        .send(topology::DisconnectClient {
            client_id: client_id.clone(),
        })
        .await
        .map_err(InternalError::ActixMessageError)?;

    let handler = WsSession {
        client_id,
        topology_addr: app.network.clone(),
    };

    ws::start(handler, &req, stream).map_err(|_err| UserError::InternalError)
}

#[get("/id/{projectID}")]
async fn get_room_state(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    let metadata = match ensure_is_authorized_host(&app, &req).await {
        Err(_) => ensure_can_edit_project(&app, &session, None, &project_id).await?,
        _ => app.get_project_metadatum(&project_id).await?,
    };

    let state = app
        .network
        .send(topology::GetRoomState { metadata })
        .await
        .map_err(InternalError::ActixMessageError)?
        .0
        .ok_or(UserError::ProjectNotActiveError)?;

    Ok(HttpResponse::Ok().json(state))
}

#[get("/")]
async fn get_rooms(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let state = app
        .network
        .send(topology::GetActiveRooms {})
        .await
        .map_err(InternalError::ActixMessageError)?
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
        .map_err(InternalError::ActixMessageError)?
        .0;

    Ok(HttpResponse::Ok().json(clients))
}

#[post("/id/{projectID}/occupants/invite")]
async fn invite_occupant(
    app: web::Data<AppData>,
    body: web::Json<OccupantInviteData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    let project = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    if !project.roles.contains_key(&body.role_id) {
        return Err(UserError::RoleNotFoundError);
    }

    let invite = OccupantInvite::new(project_id, body.into_inner());
    app.occupant_invites
        .insert_one(&invite, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let inviter = session
        .get::<String>("username")
        .map_err(|_err| UserError::PermissionsError)?
        .ok_or(UserError::PermissionsError)?;

    app.network.do_send(topology::SendOccupantInvite {
        inviter,
        invite,
        project,
    });

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
    client_id: &ClientID,
) -> Result<(), UserError> {
    let client_state = app
        .network
        .send(topology::GetClientState {
            client_id: client_id.to_owned(),
        })
        .await
        .map_err(InternalError::ActixMessageError)?
        .0;

    // Client can be evicted by project owners, collaborators
    if let Some(ClientState::Browser(BrowserClientState { project_id, .. })) = client_state {
        let can_edit = ensure_can_edit_project(app, session, None, &project_id).await;
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
        .map_err(InternalError::ActixMessageError)?
        .0;

    match client_username {
        Some(username) => ensure_can_edit_user(app, session, &username).await,
        None => Err(UserError::PermissionsError), // TODO: allow guest to evict self?
    }
}

#[post("/id/{project_id}/trace/")]
async fn start_network_trace(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ProjectId,)>,
) -> Result<HttpResponse, UserError> {
    // TODO: do we need the client ID? Require login?
    let (project_id,) = path.into_inner();
    ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let query = doc! {"id": project_id};
    let new_trace = NetworkTraceMetadata::new();
    let update = doc! {"$push": {"networkTraces": &new_trace}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();
    let metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    app.update_project_cache(metadata);
    Ok(HttpResponse::Ok().body(new_trace.id))
}

#[post("/id/{project_id}/trace/{trace_id}/stop")]
async fn stop_network_trace(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ProjectId, String)>,
) -> Result<HttpResponse, UserError> {
    let (project_id, trace_id) = path.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let trace = metadata
        .network_traces
        .iter()
        .find(|trace| trace.id == trace_id)
        .ok_or(UserError::NetworkTraceNotFoundError)?;
    let query = doc! {"id": project_id};
    let update = doc! {"$pull": {"networkTraces": &trace}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    app.update_project_cache(metadata);
    Ok(HttpResponse::Ok().body("Stopped"))
}

#[get("/id/{project_id}/trace/{trace_id}")]
async fn get_network_trace(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ProjectId, String)>,
) -> Result<HttpResponse, UserError> {
    let (project_id, trace_id) = path.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let trace = metadata
        .network_traces
        .iter()
        .find(|trace| trace.id == trace_id)
        .ok_or(UserError::NetworkTraceNotFoundError)?;

    let start_time = trace.start_time;
    let end_time = trace
        .end_time
        .unwrap_or_else(|| DateTime::from_system_time(SystemTime::now()));

    let query = doc! {
        "projectId": project_id,
        "time": {"$gt": start_time, "$lt": end_time}
    };
    let cursor = app
        .recorded_messages
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let messages: Vec<SentMessage> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(HttpResponse::Ok().json(messages))
}

#[delete("/id/{project_id}/trace/{trace_id}")]
async fn delete_network_trace(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ProjectId, String)>,
) -> Result<HttpResponse, UserError> {
    let (project_id, trace_id) = path.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let trace = metadata
        .network_traces
        .iter()
        .find(|trace| trace.id == trace_id)
        .ok_or(UserError::NetworkTraceNotFoundError)?;

    let query = doc! {"id": &project_id};
    let update = doc! {"$pull": {"networkTraces": &trace}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();
    let metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    // remove all the messages
    let earliest_start_time = metadata
        .network_traces
        .iter()
        .map(|trace| trace.start_time)
        .min()
        .unwrap_or(DateTime::MAX);

    let query = doc! {
        "projectId": project_id,
        "time": {"$lt": earliest_start_time}
    };

    app.recorded_messages
        .delete_many(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    app.update_project_cache(metadata);
    Ok(HttpResponse::Ok().body("Network trace deleted"))
}

#[post("/messages/")]
async fn send_message(
    app: web::Data<AppData>,
    message: web::Json<netsblox_core::SendMessage>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    // TODO: Should this be used to send messages from the CLI?
    ensure_is_authorized_host(&app, &req).await?;

    let message = message.into_inner();
    app.network
        .do_send(topology::SendMessageFromServices { message });

    Ok(HttpResponse::Ok().finish())
}

#[get("/{client}/state")]
async fn get_client_state(
    app: web::Data<AppData>,
    path: web::Path<(ClientID,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    if (ensure_is_authorized_host(&app, &req).await).is_err() {
        ensure_is_super_user(&app, &session).await?
    };

    let (client_id,) = path.into_inner();

    let username = app
        .network
        .send(topology::GetClientUsername {
            client_id: client_id.clone(),
        })
        .await
        .map_err(InternalError::ActixMessageError)?
        .0;

    let state = app
        .network
        .send(topology::GetClientState { client_id })
        .await
        .map_err(InternalError::ActixMessageError)?
        .0;

    Ok(HttpResponse::Ok().json(netsblox_core::ClientInfo { username, state }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(set_client_state)
        .service(get_client_state)
        .service(connect_client)
        .service(get_external_clients)
        .service(get_room_state)
        .service(send_message)
        .service(get_rooms)
        .service(invite_occupant)
        .service(evict_occupant)
        .service(start_network_trace)
        .service(stop_network_trace)
        .service(get_network_trace)
        .service(delete_network_trace);
}

struct WsSession {
    client_id: ClientID,
    topology_addr: Addr<topology::TopologyActor>,
}

impl WsSession {
    pub fn handle_msg(
        &self,
        msg_type: &str,
        mut msg: Value,
        ctx: &mut <WsSession as Actor>::Context,
    ) {
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
                    sender: self.client_id.to_owned(),
                    addresses,
                    content: msg,
                });
            }
            "ide-message" => {
                let recipients = msg["recipients"].clone();
                let addresses = match recipients {
                    Value::Array(values) => values
                        .into_iter()
                        .filter_map(|v| match v {
                            Value::String(v) => Some(ClientID::new(v)),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    _ => vec![],
                };
                msg["sender"] = json!(&self.client_id);

                self.topology_addr.do_send(topology::SendIDEMessage {
                    addresses,
                    content: msg,
                });
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
        println!("stopping! {:?}", self.client_id);
        self.topology_addr.do_send(topology::RemoveClient {
            id: self.client_id.clone(),
        });
        actix::Running::Stop
    }
}

impl Handler<ClientCommand> for WsSession {
    type Result = ();
    fn handle(&mut self, msg: ClientCommand, ctx: &mut Self::Context) {
        match msg {
            ClientCommand::SendMessage(content) => ctx.text(content.to_string()),
            ClientCommand::Close => {
                println!("server side close!");
                ctx.close(None)
            }
        }
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
                    .map(|reason| !matches!(reason.code, CloseCode::Normal | CloseCode::Away))
                    .unwrap_or(true);

                if is_broken {
                    self.topology_addr.do_send(topology::BrokenClient {
                        id: self.client_id.clone(),
                    });
                }
                ctx.close(None);
            }
            _ => (),
        }
    }
}
#[cfg(test)]
mod tests {

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
