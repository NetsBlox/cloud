use futures::future::join_all;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::bson::{doc, DateTime};
pub use netsblox_core::{BrowserClientState, ClientState, ExternalClientState};
use netsblox_core::{ExternalClient, OccupantState, RoleId, RoleState, RoomState};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use crate::app_data::AppData;
use crate::models::{ProjectId, ProjectMetadata, SaveState, SentMessage};
use crate::network::topology::address::ClientAddress;
use crate::network::AppID;
use crate::errors::InternalError;

pub use super::address::DEFAULT_APP_ID;
use super::client::{Client, ClientID, RoleRequest};
use super::{
    AddClient, BrokenClient, ClientCommand, RemoveClient, SendIDEMessage, SendMessage,
    SendOccupantInvite, SendRoomState, SetClientState,
};

#[derive(Clone, Debug)]
struct BrowserAddress {
    role_id: RoleId,
    project_id: ProjectId,
}

impl From<BrowserClientState> for BrowserAddress {
    fn from(state: BrowserClientState) -> BrowserAddress {
        BrowserAddress {
            project_id: state.project_id,
            role_id: state.role_id,
        }
    }
}

impl From<RoomState> for ClientCommand {
    fn from(msg: RoomState) -> ClientCommand {
        let mut value = serde_json::to_value(msg).unwrap();
        let msg = value.as_object_mut().unwrap();
        msg.insert("type".into(), serde_json::to_value("room-roles").unwrap());
        ClientCommand::SendMessage(value)
    }
}

impl From<SendOccupantInvite> for ClientCommand {
    fn from(msg: SendOccupantInvite) -> ClientCommand {
        ClientCommand::SendMessage(json!({
            "type": "room-invitation",
            "projectId": msg.invite.project_id,
            "roleId": msg.invite.role_id,
            "projectName": msg.project.name,
            "inviter": msg.inviter,
        }))
    }
}

struct EvictionNotice;

impl From<EvictionNotice> for ClientCommand {
    fn from(_msg: EvictionNotice) -> ClientCommand {
        ClientCommand::SendMessage(json!({"type": "eviction-notice"}))
    }
}

#[derive(Debug)]
struct ProjectNetwork {
    id: ProjectId,
    roles: HashMap<RoleId, Vec<ClientID>>,
}

impl ProjectNetwork {
    fn new(id: ProjectId) -> ProjectNetwork {
        ProjectNetwork {
            id,
            roles: HashMap::new(),
        }
    }

    fn get_state(
        &self,
        project: ProjectMetadata,
        usernames: &HashMap<ClientID, String>,
    ) -> RoomState {
        let empty = Vec::new();
        let roles: HashMap<RoleId, RoleState> = project
            .roles
            .into_iter()
            .map(|(id, role)| {
                let client_ids = self.roles.get(&id).unwrap_or(&empty);
                let occupants = client_ids
                    .iter()
                    .map(|id| OccupantState {
                        id: id.to_owned(),
                        name: usernames.get(id).unwrap_or(&"guest".to_owned()).to_owned(),
                    })
                    .collect();

                let state = RoleState {
                    name: role.name,
                    occupants,
                };
                (id, state)
            })
            .collect();

        RoomState {
            id: self.id.to_owned(),
            owner: project.owner,
            name: project.name,
            roles,
            collaborators: project.collaborators,
            version: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not get system time")
                .as_secs(),
        }
    }
}

lazy_static! {
    static ref ADDRESS_CACHE: Arc<RwLock<LruCache<ClientAddress, Vec<BrowserAddress>>>> =
        Arc::new(RwLock::new(LruCache::new(500)));
}

pub struct Topology {
    app_data: Option<AppData>,

    clients: HashMap<ClientID, Client>,
    states: HashMap<ClientID, ClientState>,
    usernames: HashMap<ClientID, String>,

    rooms: HashMap<ProjectId, ProjectNetwork>,
    external: HashMap<AppID, HashMap<String, ClientID>>,
}

#[derive(Debug)]
enum ProjectCleanup {
    NONE,
    IMMEDIATELY,
    DELAYED,
}

impl Topology {
    pub fn new() -> Topology {
        Topology {
            clients: HashMap::new(),
            app_data: None,
            rooms: HashMap::new(),
            states: HashMap::new(),
            usernames: HashMap::new(),
            external: HashMap::new(),
        }
    }

    pub fn set_app_data(&mut self, app: AppData) {
        self.app_data = Some(app);
    }

    async fn get_clients_at(&self, addr: ClientAddress) -> Vec<&Client> {
        let mut client_ids: Vec<&ClientID> = Vec::new();
        let empty = Vec::new();
        for app_id in &addr.app_ids {
            if app_id == DEFAULT_APP_ID {
                let addresses = self.resolve_address(&addr).await;
                let ids = addresses.into_iter().flat_map(|addr| {
                    self.rooms
                        .get(&addr.project_id)
                        .and_then(|room| room.roles.get(&addr.role_id))
                        .unwrap_or(&empty)
                });
                client_ids.extend(ids);
            } else {
                let id = self
                    .external
                    .get(app_id)
                    .map(|network| network.get(&addr.to_app_string()));

                if let Some(id) = id {
                    client_ids.extend(id);
                }
            }
        }

        client_ids
            .into_iter()
            .filter_map(|id| self.clients.get(id))
            .collect()
    }

    async fn get_address_string(&self, state: &ClientState) -> Option<String> {
        match state {
            ClientState::Browser(BrowserClientState {
                role_id,
                project_id,
            }) => {
                if let Some(app) = &self.app_data {
                    let project = app.get_project_metadatum(&project_id).await.unwrap();

                    project
                        .roles
                        .get(role_id)
                        .map(|role| format!("{}@{}@{}", role.name, project.name, project.owner))
                } else {
                    None
                }
            }
            ClientState::External(ExternalClientState { address, app_id }) => {
                Some(format!("{} #{}", address, app_id))
            }
        }
    }

    fn resolve_address_from_cache(&self, addr: &ClientAddress) -> Option<Vec<BrowserAddress>> {
        ADDRESS_CACHE
            .write()
            .unwrap()
            .get(addr)
            .map(|addresses| addresses.to_vec())
    }

    fn cache_address(&self, addr: &ClientAddress, b_addrs: &Vec<BrowserAddress>) {
        ADDRESS_CACHE
            .write()
            .unwrap()
            .put(addr.clone(), b_addrs.clone());
        // TODO: clear cache on room close?
    }

    async fn resolve_address(&self, addr: &ClientAddress) -> Vec<BrowserAddress> {
        if let Some(addresses) = self.resolve_address_from_cache(addr) {
            return addresses;
        }
        let addresses = self.resolve_address_from_db(addr).await;

        if addresses.len() > 0 {
            self.cache_address(addr, &addresses);
        }

        addresses
    }

    async fn resolve_address_from_db(&self, addr: &ClientAddress) -> Vec<BrowserAddress> {
        let project_metadata = match &self.app_data {
            Some(app) => &app.project_metadata,
            None => return Vec::new(),
        };

        let mut chunks = addr.address.split('@').rev();
        let project = chunks.next().unwrap();
        let role = chunks.next();

        let query = doc! {"name": project, "owner": &addr.user_id};
        let empty = Vec::new();
        project_metadata
            .find_one(query, None)
            .await
            .unwrap()
            .map(|metadata| {
                let role_names = role.map(|name| vec![name.to_owned()]).unwrap_or_else(|| {
                    metadata
                        .roles
                        .iter()
                        .map(|(_, role)| role.name.to_owned())
                        .collect()
                });

                let name2id = metadata
                    .roles
                    .into_iter()
                    .map(|(k, v)| (v.name, k))
                    .collect::<HashMap<_, _>>();

                role_names
                    .into_iter()
                    .filter_map(|name| name2id.get(&name))
                    .map(|role_id| BrowserAddress {
                        project_id: metadata.id.to_owned(),
                        role_id: role_id.to_owned(),
                    })
                    .collect()
            })
            .unwrap_or(empty)
    }

    pub async fn send_msg(&self, msg: SendMessage) {
        let message = ClientCommand::SendMessage(msg.content.clone());
        let recipients = join_all(
            msg.addresses
                .iter()
                .filter_map(|addr_str| ClientAddress::from_str(addr_str).ok())
                .map(|address| self.get_clients_at(address)), // TODO: Get the project for these clients?
        )
        .await
        .into_iter()
        .flatten();

        recipients.clone().for_each(|client| {
            client.addr.do_send(message.clone()).unwrap();
        });

        // maybe record the message
        if let Some(app) = &self.app_data {
            let project_ids: Vec<_> = recipients
                .clone()
                .map(|client| &client.id)
                .chain(std::iter::once(&msg.sender))
                .filter_map(|client_id| match self.get_client_state(client_id) {
                    Some(ClientState::Browser(BrowserClientState { project_id, .. })) => {
                        Some(project_id.to_owned())
                    }
                    _ => None,
                })
                .collect();

            let projects = app.get_project_metadata(&project_ids).await.unwrap();
            let recording_ids = projects
                .iter()
                .filter(|metadata| {
                    metadata
                        .network_traces
                        .iter()
                        .find(|trace| trace.end_time == None)
                        .is_some()
                })
                .map(|metadata| metadata.id.to_owned());

            let source = self.get_client_state(&msg.sender).unwrap();
            let recipients = recipients
                .filter_map(|client| self.get_client_state(&client.id))
                .map(|state| state.to_owned())
                .collect::<Vec<_>>();

            // TODO: record the actual recipients
            let messages: Vec<_> = recording_ids
                .into_iter()
                .map(|project_id| {
                    SentMessage::new(
                        project_id,
                        source.to_owned(),
                        recipients.clone(),
                        msg.content.clone(),
                    )
                })
                .collect();

            if messages.len() > 0 {
                app.recorded_messages
                    .insert_many(messages, None)
                    .await
                    .unwrap();
            }
        }
    }

    fn has_client(&self, id: &str) -> bool {
        self.clients.contains_key(id)
    }

    pub fn disconnect_client(&self, id: &str) {
        if let Some(client) = self.clients.get(id) {
            client.addr.do_send(ClientCommand::Close).unwrap();
        }
    }

    pub async fn set_client_state(&mut self, msg: SetClientState) {
        if !self.has_client(&msg.id) {
            return;
        }

        let new_project_id = match msg.state {
            ClientState::Browser(ref state) => Some(state.project_id.clone()),
            _ => None,
        };
        self.reset_client_state(&msg.id, new_project_id).await;
        if let Some(username) = msg.username {
            self.usernames.insert(msg.id.clone(), username);
        }

        match &msg.state {
            ClientState::Browser(state) => {
                if !self.rooms.contains_key(&state.project_id) {
                    self.rooms.insert(
                        state.project_id.to_owned(),
                        ProjectNetwork::new(state.project_id.to_owned()),
                    );
                }
                let room = self.rooms.get_mut(&state.project_id).unwrap();
                if let Some(occupants) = room.roles.get_mut(&state.role_id) {
                    occupants.push(msg.id.clone());
                } else {
                    room.roles
                        .insert(state.role_id.clone(), vec![msg.id.clone()]);
                }
                let project_id = state.project_id.to_owned();
                self.send_room_state_for(&project_id).await;
            }
            ClientState::External(state) => {
                let app_net = self
                    .external
                    .entry(state.app_id.to_lowercase())
                    .or_insert_with(HashMap::new);

                app_net.insert(state.address.to_owned(), msg.id.to_owned());
            }
        }
        self.states.insert(msg.id, msg.state);
    }

    pub fn add_client(&mut self, msg: AddClient) {
        let client = Client::new(msg.id.clone(), msg.addr);
        self.clients.insert(msg.id, client);
    }

    pub async fn set_broken_client(&mut self, msg: BrokenClient) {
        if let Some(app) = &self.app_data {
            if let Some(ClientState::Browser(state)) = self.states.get(&msg.id) {
                let query = doc! {
                    "id": &state.project_id,
                    "saveState": SaveState::TRANSIENT
                };
                let update = doc! {"$set": {"saveState": SaveState::BROKEN}};
                let result = app
                    .project_metadata
                    .update_one(query, update, None)
                    .await
                    .unwrap();
            }
        }
        // TODO: Record a list of broken clients for the project?
    }

    pub async fn remove_client(&mut self, msg: RemoveClient) {
        self.clients.remove(&msg.id);
        self.reset_client_state(&msg.id, None).await;
    }

    async fn reset_client_state(&mut self, id: &str, new_project_id: Option<ProjectId>) {
        self.usernames.remove(id);
        match self.states.remove(id) {
            Some(ClientState::Browser(state)) => {
                let room = self.rooms.get_mut(&state.project_id);
                let mut empty: Vec<String> = Vec::new();
                let mut update_needed = room.is_some();

                if let Some(room) = room {
                    let occupants = room.roles.get_mut(&state.role_id).unwrap_or(&mut empty);
                    if let Some(pos) = occupants.iter().position(|item| item == id) {
                        occupants.swap_remove(pos);
                    }

                    if occupants.is_empty() {
                        let role_count = room.roles.len();
                        let is_leaving_project = new_project_id
                            .map(|id| id != state.project_id)
                            .unwrap_or(true);
                        let remove_room = role_count == 1 && is_leaving_project;
                        if remove_room {
                            self.remove_room(&state.project_id).await;
                            update_needed = false;
                        } else {
                            // remove the role
                            let room = self.rooms.get_mut(&state.project_id).unwrap();
                            room.roles.remove(&state.role_id);
                        }
                    }
                }

                if update_needed {
                    self.send_room_state_for(&state.project_id).await;
                }
            }
            Some(ClientState::External(state)) => {
                let remove_entry = self
                    .external
                    .get_mut(&state.app_id)
                    .map(|network| {
                        network.remove(&state.address);
                        network.keys().len() == 0
                    })
                    .unwrap_or(false);

                if remove_entry {
                    self.external.remove(&state.app_id);
                }
            }
            None => {}
        }
    }

    async fn remove_room(&mut self, project_id: &ProjectId) -> Result<(), InternalError>{
        // Set the entry to be removed. After how long?
        //   - If the room has only one role, it can be deleted immediately
        //     - the client may need to be updated
        //   - if multiple roles and there is a broken connection:
        //     - delete after an amount of time with no activity - maybe 10 minutes?
        self.rooms.remove(project_id);
        if let Some(app) = &self.app_data {
            // If it has no broken connections, delete it!
            let query = doc! {"id": &project_id};
            let cleanup = app
                .project_metadata
                .find_one(query.clone(), None)
                .await
                .unwrap()
                .map(|md| match md.save_state {
                    SaveState::CREATED => unreachable!(),
                    SaveState::TRANSIENT => ProjectCleanup::IMMEDIATELY,
                    SaveState::BROKEN => ProjectCleanup::DELAYED,
                    SaveState::SAVED => ProjectCleanup::NONE,
                })
                .unwrap_or(ProjectCleanup::NONE);

            match cleanup {
                ProjectCleanup::IMMEDIATELY => {
                    app.project_metadata.delete_one(query, None).await
                        .map_err(|err| InternalError::DatabaseConnectionError(err))?;
                }
                ProjectCleanup::DELAYED => {
                    let ten_minutes = Duration::new(10 * 60, 0);
                    let delete_at = DateTime::from_system_time(
                        SystemTime::now().checked_add(ten_minutes).unwrap(),
                    );
                    let update = doc! {"$set": {"deleteAt": delete_at}};
                    app.project_metadata.update_one(query, update, None).await
                        .map_err(|err| InternalError::DatabaseConnectionError(err))?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn send_room_state_for(&mut self, project_id: &ProjectId) {
        if let Some(app) = &self.app_data {
            let query = doc! {"id": project_id};
            if let Some(project) = app.project_metadata.find_one(query, None).await.unwrap() {
                self.send_room_state(SendRoomState { project });
            }
        }
    }

    pub fn send_room_state(&self, msg: SendRoomState) {
        self.invalidate_cached_addresses(&msg.project);

        if let Some(room) = self.rooms.get(&msg.project.id) {
            let clients = room
                .roles
                .values()
                .flatten()
                .filter_map(|id| self.clients.get(id));

            let room_state = room.get_state(msg.project, &self.usernames);
            clients.for_each(|client| {
                let _ = client.addr.do_send(room_state.clone().into()); // TODO: handle error?
            });
        }
    }

    fn invalidate_cached_addresses(&self, project: &ProjectMetadata) {
        let mut address_cache = ADDRESS_CACHE.write().unwrap();
        let invalid_addrs: Vec<_> = address_cache
            .iter()
            .filter_map(|(client_addr, browser_addrs)| {
                browser_addrs
                    .iter()
                    .find(|addr| addr.project_id == project.id)
                    .map(|_| client_addr.clone())
            })
            .collect();

        invalid_addrs.into_iter().for_each(|addr| {
            address_cache.pop(&addr);
        });
    }

    pub fn get_role_request(&self, state: BrowserClientState) -> Option<RoleRequest> {
        self.rooms
            .get(&state.project_id)
            .and_then(|room| room.roles.get(&state.role_id))
            .and_then(|client_ids| client_ids.first())
            .and_then(|id| self.clients.get(id))
            .map(|client| RoleRequest::new(client.addr.clone(), state.clone()))
    }

    pub fn get_active_rooms(&self) -> Vec<ProjectId> {
        self.rooms.keys().map(|k| k.to_owned()).collect::<Vec<_>>()
    }

    pub fn get_external_clients(&self) -> Vec<ExternalClient> {
        self.states
            .iter()
            .filter_map(|(id, state)| match state {
                ClientState::External(state) => Some(ExternalClient {
                    username: self.usernames.get(id).map(|name| name.to_owned()),
                    address: state.address.to_owned(),
                    app_id: state.app_id.to_owned(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>()
    }

    /// Get a list of online users from a list of usernames. If no usernames are provided,
    /// all online users will be returned
    pub fn get_online_users(&self, from_names: Option<Vec<String>>) -> Vec<String> {
        let online = self.usernames.values().collect::<HashSet<_>>();
        match from_names {
            Some(usernames) => usernames
                .into_iter()
                .filter(|username| online.contains(&username))
                .collect(),
            None => online
                .into_iter()
                .map(|username| username.to_owned())
                .collect(),
        }
    }

    pub fn get_room_state(&self, metadata: ProjectMetadata) -> Option<RoomState> {
        self.rooms
            .get(&metadata.id)
            .map(|room| room.get_state(metadata, &self.usernames))
    }

    pub async fn evict_client(&mut self, id: ClientID) {
        let username = self.usernames.remove(&id);
        self.reset_client_state(&id, None).await;
        self.clients
            .get(&id)
            .map(|client| client.addr.do_send(EvictionNotice.into()));

        if let Some(username) = username {
            if self.usernames.get(&id).is_none() {
                self.usernames.insert(id, username);
            }
        }
    }

    pub fn get_client_state(&self, id: &ClientID) -> Option<&ClientState> {
        self.states.get(id)
    }

    pub fn get_client_username(&self, id: &ClientID) -> Option<&String> {
        self.usernames.get(id)
    }

    pub fn send_occupant_invite(&self, msg: SendOccupantInvite) {
        let clients = self.usernames.iter().filter_map(|(client_id, username)| {
            if username == &msg.invite.username {
                self.clients.get(client_id)
            } else {
                None
            }
        });

        clients.for_each(|client| {
            client.addr.do_send(msg.clone().into());
        });
    }

    pub async fn send_msg_from_services(&self, msg: netsblox_core::SendMessage) {
        let recipients = match msg.target {
            netsblox_core::SendMessageTarget::Address { address } => {
                if let Ok(address) = ClientAddress::from_str(&address) {
                    self.get_clients_at(address).await
                } else {
                    Vec::new()
                }
            }
            netsblox_core::SendMessageTarget::Client {
                project_id,
                role_id,
                client_id,
            } => {
                let state = self.states.get(&client_id);
                let has_state = match state {
                    Some(ClientState::Browser(BrowserClientState {
                        role_id: role,
                        project_id: project,
                    })) => &role_id == role && &project_id == project,
                    _ => false,
                };

                let mut clients = Vec::new();
                if let Some(client) = self.clients.get(&client_id) {
                    if has_state {
                        clients.push(client);
                    }
                }
                clients
            }
            netsblox_core::SendMessageTarget::Role {
                project_id,
                role_id,
            } => self
                .rooms
                .get(&project_id)
                .and_then(|room| {
                    room.roles.get(&role_id).map(|ids| {
                        ids.iter()
                            .filter_map(|id| self.clients.get(id))
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_else(|| Vec::new()),
            netsblox_core::SendMessageTarget::Room { project_id } => self
                .rooms
                .get(&project_id)
                .map(|room| {
                    let client_ids = room.roles.values().flatten();
                    client_ids
                        .filter_map(|id| self.clients.get(id))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| Vec::new()),
        };

        println!("Sending to {count} clients", count = recipients.len());
        let message = ClientCommand::SendMessage(msg.content);
        recipients.iter().for_each(|client| {
            client.addr.do_send(message.clone()).unwrap();
        });
    }

    pub fn send_ide_msg(&self, msg: SendIDEMessage) {
        let recipients = msg
            .addresses
            .iter()
            .filter_map(|client_id| self.clients.get(client_id));

        let message = ClientCommand::SendMessage(msg.content);
        recipients.for_each(|client| {
            client.addr.do_send(message.clone()).unwrap();
        });
    }
}

#[cfg(test)]
mod tests {
    #[actix_web::test]
    async fn test_remove_client_clear_state() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_client_clear_external_state() {
        todo!();
    }

    #[actix_web::test]
    async fn test_filter_blocked_users() {
        todo!();
    }

    #[actix_web::test]
    async fn test_filter_group_msgs() {
        todo!();
    }
    // TODO: Add test for broken connections?
}
