pub mod topology;

use std::time::SystemTime;

use crate::app_data::AppData;
use crate::common::api::{
    BrowserClientState, ClientId, ClientState, ClientStateData, OccupantInviteData, ProjectId,
    SaveState,
};
use crate::common::{
    api, api::ExternalClientState, NetworkTraceMetadata, OccupantInvite, SentMessage,
};
use crate::errors::{InternalError, UserError};
use crate::projects::{can_edit_project, ensure_can_edit_project, ensure_can_view_project};
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
use netsblox_cloud_common::ProjectMetadata;
use serde::Deserialize;
use serde_json::{json, Value};
use topology::ClientCommand;

#[post("/{client}/state")] // TODO: add token here (in a header), too?
async fn set_client_state(
    app: web::Data<AppData>,
    path: web::Path<(ClientId,)>,
    body: web::Json<ClientStateData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    // TODO: should we allow users to set the client state for some other user?
    let username = session.get::<String>("username").ok().flatten();
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
            if app_id.as_str().to_lowercase() == topology::DEFAULT_APP_ID {
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
    path: web::Path<(ClientId,)>,
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

    let metadata = match ensure_is_authorized_host(&app, &req, None).await {
        Err(_) => ensure_can_edit_project(&app, &session, None, &project_id).await?,
        _ => app.get_project_metadatum(&project_id).await?,
    };

    let task = app
        .network
        .send(topology::GetRoomState(metadata))
        .await
        .map_err(InternalError::ActixMessageError)?;
    let state = task.run().await.ok_or(UserError::ProjectNotActiveError)?;

    Ok(HttpResponse::Ok().json(state))
}

#[get("/")]
async fn get_rooms(app: web::Data<AppData>, session: Session) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let task = app
        .network
        .send(topology::GetActiveRooms {})
        .await
        .map_err(InternalError::ActixMessageError)?;
    let rooms = task.run().await;

    Ok(HttpResponse::Ok().json(rooms))
}

#[get("/external")]
async fn get_external_clients(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_super_user(&app, &session).await?;

    let task = app
        .network
        .send(topology::GetExternalClients {})
        .await
        .map_err(InternalError::ActixMessageError)?;
    let clients = task.run().await;

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
        invite: invite.clone(),
        project,
    });

    Ok(HttpResponse::Ok().json(invite))
}

#[post("/clients/{clientID}/evict")]
async fn evict_occupant(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(ClientId,)>,
) -> Result<HttpResponse, UserError> {
    let (client_id,) = path.into_inner();

    let metadata = ensure_can_evict_client(&app, &session, &client_id).await?;

    app.network
        .send(topology::EvictOccupant { client_id })
        .await
        .map_err(InternalError::ActixMessageError)?;

    // Fetch the current state of the room
    let room_state = if let Some(metadata) = metadata {
        let task = app
            .network
            .send(topology::GetRoomState(metadata))
            .await
            .map_err(InternalError::ActixMessageError)?;

        task.run().await
    } else {
        None
    };

    Ok(HttpResponse::Ok().json(room_state))
}

async fn ensure_can_evict_client(
    app: &AppData,
    session: &Session,
    client_id: &ClientId,
) -> Result<Option<ProjectMetadata>, UserError> {
    // Client can be evicted by project owners, collaborators
    let metadata = get_project_for_client(app, client_id).await?;

    if let Some(metadata) = metadata.clone() {
        if can_edit_project(app, session, Some(client_id), &metadata).await? {
            return Ok(Some(metadata));
        }
    }

    // or by anyone who can edit the corresponding user
    let task = app
        .network
        .send(topology::GetClientUsername(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;

    let client_username = task.run().await;

    match client_username {
        Some(username) => ensure_can_edit_user(app, session, &username).await,
        None => Err(UserError::PermissionsError), // TODO: allow guest to evict self?
    }?;

    Ok(metadata)
}

async fn get_project_for_client(
    app: &AppData,
    client_id: &ClientId,
) -> Result<Option<ProjectMetadata>, UserError> {
    let task = app
        .network
        .send(topology::GetClientState(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;

    let client_state = task.run().await;
    let project_id = client_state.and_then(|state| match state {
        ClientState::Browser(BrowserClientState { project_id, .. }) => Some(project_id),
        _ => None,
    });

    let metadata = if let Some(id) = project_id {
        Some(app.get_project_metadatum(&id).await?)
    } else {
        None
    };

    Ok(metadata)
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
    Ok(HttpResponse::Ok().json(new_trace))
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

    let query = doc! {
        "id": project_id,
        "networkTraces.id": &trace.id
    };
    let end_time = DateTime::now();
    let update = doc! {
        "$set": {
            "networkTraces.$.endTime": end_time
        }
    };
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    let trace = metadata
        .network_traces
        .iter()
        .find(|t| t.id == trace.id)
        .unwrap() // guaranteed to exist since it was checked in the query
        .clone();

    app.update_project_cache(metadata);
    Ok(HttpResponse::Ok().json(trace))
}

#[get("/id/{project_id}/trace/{trace_id}")]
async fn get_network_trace_metadata(
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

    Ok(HttpResponse::Ok().json(trace))
}

#[get("/id/{project_id}/trace/{trace_id}/messages")]
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
    let end_time = trace.end_time.unwrap_or_else(|| DateTime::now());

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

    app.update_project_cache(metadata.clone());
    Ok(HttpResponse::Ok().json(metadata))
}

#[post("/messages/")]
async fn send_message(
    app: web::Data<AppData>,
    message: web::Json<api::SendMessage>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    // TODO: Should this be used to send messages from the CLI?
    ensure_is_authorized_host(&app, &req, None).await?;

    let message = message.into_inner();
    app.network
        .do_send(topology::SendMessageFromServices { message });

    Ok(HttpResponse::Ok().finish())
}

#[get("/{client}/state")]
async fn get_client_state(
    app: web::Data<AppData>,
    path: web::Path<(ClientId,)>,
    session: Session,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    if ensure_is_authorized_host(&app, &req, None).await.is_err() {
        ensure_is_super_user(&app, &session).await?
    };

    let (client_id,) = path.into_inner();

    let task = app
        .network
        .send(topology::GetClientUsername(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;
    let username = task.run().await;
    let task = app
        .network
        .send(topology::GetClientState(client_id.clone()))
        .await
        .map_err(InternalError::ActixMessageError)?;
    let state = task.run().await;

    Ok(HttpResponse::Ok().json(api::ClientInfo { username, state }))
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
        .service(get_network_trace_metadata)
        .service(delete_network_trace);
}

struct WsSession {
    client_id: ClientId,
    topology_addr: Addr<topology::TopologyActor>,
}

impl WsSession {
    pub fn handle_msg(
        &self,
        msg_type: &str,
        mut msg: Value,
        ctx: &mut <WsSession as Actor>::Context,
    ) {
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
                            Value::String(v) => Some(ClientId::new(v)),
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
            ClientCommand::Close => ctx.close(None),
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Value::String(msg_type) = &v["type"] {
                        self.handle_msg(&msg_type.clone(), v, ctx);
                    }
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
    use std::{collections::HashMap, time::Duration};

    use actix_web::{http, test, App};
    use netsblox_cloud_common::User;

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    #[ignore]
    async fn test_connect_client() {
        // TODO: send a connect request and check that the client has been added to the topology
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_send_msg_room() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": "project@owner"});
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_send_msg_list() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": ["role1@project@owner"]});
        //client.handle_msg(msg);
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_connect_invalid_client_id() {
        //let client = Client::new("test".into());
        //let msg = json!({"type": "message", "dstId": "role1@project@owner"});

        todo!();
    }

    #[actix_web::test]
    async fn test_invite_occupant() {
        let sender: User = api::NewUser {
            username: "sender".to_string(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".to_string(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner("sender".to_string())
            .build();

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let role_id = project.roles.keys().next().unwrap().to_owned();
                let data = api::OccupantInviteData {
                    username: rcvr.username.clone(),
                    role_id,
                };
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&sender.username))
                    .uri(&format!("/id/{}/occupants/invite", &project.id))
                    .set_json(data)
                    .to_request();

                // Ensure that the collaboration invite is returned.
                // This will panic if the response is incorrect so no assert needed.
                let _invite: OccupantInvite = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_network_trace_metadata() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let trace = NetworkTraceMetadata::new();
        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_traces(&[trace.clone()])
            .build();

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}", &project.id, &trace.id))
                    .to_request();

                let metadata: NetworkTraceMetadata = test::call_and_read_body_json(&app, req).await;
                assert_eq!(metadata.id, trace.id);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_network_trace_msgs() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let r1_id = api::RoleId::new("r1".into());
        let r2_id = api::RoleId::new("r2".into());
        let roles: HashMap<_, _> = [
            (
                r1_id.clone(),
                api::RoleData {
                    name: "sender".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
            (
                r2_id.clone(),
                api::RoleData {
                    name: "rcvr".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
        ]
        .into_iter()
        .collect();

        let trace = NetworkTraceMetadata::new();
        let project = test_utils::project::builder()
            .with_name("project".into())
            .with_owner("owner".to_string())
            .with_traces(&[trace.clone()])
            .with_roles(roles)
            .build();

        let s1 = ClientState::Browser(BrowserClientState {
            project_id: project.id.clone(),
            role_id: r1_id,
        });
        let s2 = ClientState::Browser(BrowserClientState {
            project_id: project.id.clone(),
            role_id: r2_id,
        });

        let sender = test_utils::network::Client::new(Some(owner.username.clone()), Some(s1));
        let rcvr = test_utils::network::Client::new(Some(owner.username.clone()), Some(s2));

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .with_clients(&[sender.clone(), rcvr.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                // send messages
                let content = json! ({
                    "type": "message",
                    "msgType": "message",
                    "content": {
                        "msg": "hello!"
                    }
                });
                let messages = (0..10).flat_map(|_i| {
                    [
                        topology::SendMessage {
                            sender: sender.id.clone(),
                            addresses: vec!["rcvr@project@owner".into()],
                            content: content.clone(),
                        },
                        topology::SendMessage {
                            sender: rcvr.id.clone(),
                            addresses: vec!["sender@project@owner".into()],
                            content: content.clone(),
                        },
                    ]
                });

                let messages = messages.collect::<Vec<_>>();
                println!("sending {} messages", messages.len());
                for msg in messages {
                    app_data.network.send(msg).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }

                // wait for the messages to be recorded (up to a limit, ofc)
                let max_end_time = SystemTime::now() + Duration::from_millis(500);
                let mut count = app_data
                    .recorded_messages
                    .count_documents(doc! {}, None)
                    .await
                    .unwrap();

                while count < 20 {
                    // TODO: why is this failing sometimes?
                    assert!(SystemTime::now() < max_end_time);
                    let times = app_data
                        .recorded_messages
                        .find(doc! {}, None)
                        .await
                        .unwrap()
                        .try_collect::<Vec<_>>()
                        .await
                        .unwrap()
                        .into_iter()
                        .map(|msg| msg.time)
                        .collect::<Vec<_>>();

                    dbg!(&times);
                    println!(
                        "count is {} ({}) as of {:?}",
                        &count,
                        times.len(),
                        DateTime::now()
                    );

                    count = app_data
                        .recorded_messages
                        .count_documents(doc! {}, None)
                        .await
                        .unwrap();
                }

                // fetch sent messages
                println!("About to request the messages");
                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}/messages", &project.id, &trace.id))
                    .to_request();

                let messages: Vec<SentMessage> = test::call_and_read_body_json(&app, req).await;
                assert_eq!(messages.len(), 20);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_stop_network_trace() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let r1_id = api::RoleId::new("r1".into());
        let r2_id = api::RoleId::new("r2".into());
        let roles: HashMap<_, _> = [
            (
                r1_id.clone(),
                api::RoleData {
                    name: "sender".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
            (
                r2_id.clone(),
                api::RoleData {
                    name: "rcvr".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
        ]
        .into_iter()
        .collect();

        let trace = NetworkTraceMetadata::new();
        let project = test_utils::project::builder()
            .with_name("project".into())
            .with_owner("owner".to_string())
            .with_traces(&[trace.clone()])
            .with_roles(roles)
            .build();

        let s1 = ClientState::Browser(BrowserClientState {
            project_id: project.id.clone(),
            role_id: r1_id,
        });
        let s2 = ClientState::Browser(BrowserClientState {
            project_id: project.id.clone(),
            role_id: r2_id,
        });

        let sender = test_utils::network::Client::new(Some(owner.username.clone()), Some(s1));
        let rcvr = test_utils::network::Client::new(Some(owner.username.clone()), Some(s2));

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .with_clients(&[sender.clone(), rcvr.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                // send messages
                let content = json! ({
                    "type": "message",
                    "msgType": "message",
                    "content": {
                        "msg": "hello!"
                    }
                });
                app_data
                    .network
                    .send(topology::SendMessage {
                        sender: sender.id.clone(),
                        addresses: vec!["rcvr@project@owner".into()],
                        content: content.clone(),
                    })
                    .await
                    .unwrap();

                // wait for the message to be recorded (up to a limit, ofc)
                let max_end_time = SystemTime::now() + Duration::from_millis(150);
                let mut is_recorded = app_data
                    .recorded_messages
                    .find_one(doc! {}, None)
                    .await
                    .unwrap()
                    .is_some();

                while !is_recorded {
                    assert!(SystemTime::now() < max_end_time);
                    tokio::time::sleep(Duration::from_millis(10)).await;

                    is_recorded = app_data
                        .recorded_messages
                        .find_one(doc! {}, None)
                        .await
                        .unwrap()
                        .is_some();
                }

                // stop recording messages
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}/stop", &project.id, &trace.id))
                    .to_request();

                let metadata: NetworkTraceMetadata = test::call_and_read_body_json(&app, req).await;
                assert_eq!(metadata.id, trace.id);

                // send another message
                app_data
                    .network
                    .send(topology::SendMessage {
                        sender: rcvr.id.clone(),
                        addresses: vec!["sender@project@owner".into()],
                        content: content.clone(),
                    })
                    .await
                    .unwrap();

                // fetch sent messages
                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}/messages", &project.id, &trace.id))
                    .to_request();

                let mut messages: Vec<SentMessage> = test::call_and_read_body_json(&app, req).await;
                assert_eq!(messages.len(), 1);

                // Check that it is the first message
                let msg = messages.pop().unwrap();
                assert_eq!(msg.source, sender.state.unwrap());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_stop_network_trace_404() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let r1_id = api::RoleId::new("r1".into());
        let r2_id = api::RoleId::new("r2".into());
        let roles: HashMap<_, _> = [
            (
                r1_id.clone(),
                api::RoleData {
                    name: "sender".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
            (
                r2_id.clone(),
                api::RoleData {
                    name: "rcvr".into(),
                    code: "<code/>".into(),
                    media: "<media/>".into(),
                },
            ),
        ]
        .into_iter()
        .collect();

        let trace = NetworkTraceMetadata::new();
        let project = test_utils::project::builder()
            .with_name("project".into())
            .with_owner("owner".to_string())
            .with_roles(roles)
            .build();

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                // stop recording messages
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}/stop", &project.id, &trace.id))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_network_trace() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let trace = NetworkTraceMetadata::new();
        let project = test_utils::project::builder()
            .with_owner("owner".to_string())
            .with_traces(&[trace.clone()])
            .build();

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!("/id/{}/trace/{}", &project.id, &trace.id))
                    .to_request();

                let metadata: ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                assert!(metadata.network_traces.is_empty());
                // check the network trace has been removed from the project metadata
                let project = app_data.get_project_metadatum(&project.id).await.unwrap();
                assert!(project.network_traces.is_empty());
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_evict_occupant_project_owner() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_evict_occupant_project_collaborator() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_evict_occupant_group_owner() {
        todo!();
    }
}
