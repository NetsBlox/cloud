use crate::auth;
use crate::common::api;
use crate::common::api::{
    AppId, BrowserClientState, ClientState, ExternalClient, OccupantState, RoleId, RoleState,
    RoomState,
};
use futures::future::join_all;
use log::warn;
use lru::LruCache;
use mongodb::bson::{doc, DateTime};
use netsblox_cloud_common::SentMessage;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use crate::app_data::AppData;
use crate::common::api::{ProjectId, SaveState};
use crate::common::ProjectMetadata;
use crate::errors::InternalError;
use crate::network::topology::address::ClientAddress;

pub use super::address::DEFAULT_APP_ID;
use super::client::{Client, ClientId, RoleRequest};
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
        let mut value = serde_json::to_value(msg).unwrap(); // safe to unwrap since RoomState is serializable
        let msg = value.as_object_mut().unwrap(); // safe to unwrap since RoomState is serialized as a JSON object
        msg.insert(
            "type".into(),
            serde_json::to_value("room-roles").unwrap(), // save to unwrap since it is just a string
        );
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
    roles: HashMap<RoleId, Vec<ClientId>>,
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
        usernames: &HashMap<ClientId, String>,
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

        let version = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|dur| dur.as_secs())
            .map_err(|err| {
                log::error!("Unable to compute unix timestamp: {}", &err);
                err
            })
            .unwrap_or_default();

        RoomState {
            id: self.id.to_owned(),
            owner: project.owner,
            name: project.name,
            roles,
            collaborators: project.collaborators,
            version,
        }
    }
}

pub(crate) struct Topology {
    app_data: Option<AppData>,

    clients: HashMap<ClientId, Client>,
    states: HashMap<ClientId, ClientState>,
    usernames: HashMap<ClientId, String>,

    rooms: HashMap<ProjectId, ProjectNetwork>,
    external: HashMap<AppId, HashMap<String, ClientId>>,

    address_cache: Arc<RwLock<LruCache<ClientAddress, Vec<BrowserAddress>>>>,
    cache_size: NonZeroUsize,
}

#[derive(Debug)]
enum ProjectCleanup {
    None,
    Immediately,
    Delayed,
}

impl Topology {
    pub fn new(cache_size: NonZeroUsize) -> Topology {
        Topology {
            clients: HashMap::new(),
            app_data: None,
            rooms: HashMap::new(),
            states: HashMap::new(),
            usernames: HashMap::new(),
            external: HashMap::new(),

            address_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            cache_size,
        }
    }

    pub fn set_app_data(&mut self, app: AppData) {
        self.app_data = Some(app);
    }

    async fn get_clients_at(&self, addr: ClientAddress) -> Vec<&Client> {
        let mut client_ids: Vec<&ClientId> = Vec::new();
        let empty = Vec::new();
        for app_id_str in &addr.app_ids {
            if app_id_str == DEFAULT_APP_ID {
                let addresses = self.resolve_address(&addr).await;
                let ids = addresses.into_iter().flat_map(|addr| {
                    self.rooms
                        .get(&addr.project_id)
                        .and_then(|room| room.roles.get(&addr.role_id))
                        .unwrap_or(&empty)
                });
                client_ids.extend(ids);
            } else {
                let app_id = AppId::new(app_id_str);
                let id = self
                    .external
                    .get(&app_id)
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

    fn resolve_address_from_cache(&self, addr: &ClientAddress) -> Option<Vec<BrowserAddress>> {
        self.address_cache
            .write()
            .map_err(|err| {
                log::error!("Unable to acquire mutex for address cache to resolve address");
                err
            })
            .ok()
            .and_then(|mut cache| cache.get(addr).map(|addresses| addresses.to_vec()))
    }

    fn cache_address(&self, addr: &ClientAddress, b_addrs: &[BrowserAddress]) {
        if let Err(err) = self
            .address_cache
            .write()
            .map(|mut cache| cache.put(addr.clone(), b_addrs.to_vec()))
        {
            log::error!(
                "Unable to acquire mutex for address cache to add address: {}",
                err
            );
        }
    }

    async fn resolve_address(&self, addr: &ClientAddress) -> Vec<BrowserAddress> {
        if let Some(addresses) = self.resolve_address_from_cache(addr) {
            return addresses;
        }
        let addresses = self.resolve_address_from_db(addr).await;

        if !addresses.is_empty() {
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
        let project = chunks.next().unwrap(); // safe to unwrap: we know there is at least one chunk
        let role = chunks.next();

        let query = doc! {"name": project, "owner": &addr.user_id};
        project_metadata
            .find_one(query, None)
            .await
            .map_err(|err| {
                warn!("Unable to resolve address: {:?}", err);
                InternalError::DatabaseConnectionError(err)
            })
            .ok()
            .flatten()
            .map(|metadata| {
                let role_names = role.map(|name| vec![name.to_owned()]).unwrap_or_else(|| {
                    metadata
                        .roles
                        .values()
                        .map(|role| role.name.to_owned())
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
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub async fn send_msg(&self, msg: SendMessage) {
        if let Some(app) = &self.app_data {
            let message = ClientCommand::SendMessage(msg.content.clone());
            let recipients: Vec<_> = join_all(
                msg.addresses
                    .iter()
                    .filter_map(|addr_str| ClientAddress::from_str(addr_str).ok())
                    .map(|address| self.get_clients_at(address)),
            )
            .await
            .into_iter()
            .flatten()
            .collect();

            // check if the message is allowed
            // Since the likelihood of malicious projects being able to send a meaningful
            // message (ie, that has the correct message type and is listened to by the
            // target) is quite low, we will allow all messages to be sent for now.
            //let recipients = self.allowed_recipients(app, &msg.sender, recipients).await;

            recipients.iter().for_each(|client| {
                if let Err(err) = client.addr.do_send(message.clone()) {
                    log::error!("Unable to send message to client: {}", err);
                }
            });

            // maybe record the message
            let project_ids: HashSet<_> = recipients
                .iter()
                .map(|client| &client.id)
                .chain(std::iter::once(&msg.sender))
                .filter_map(|client_id| match self.get_client_state(client_id) {
                    Some(ClientState::Browser(BrowserClientState { project_id, .. })) => {
                        Some(project_id.to_owned())
                    }
                    _ => None,
                })
                .collect();

            let projects = app
                .get_project_metadata(project_ids.iter())
                .await
                .unwrap_or_default();

            let recording_ids = projects
                .iter()
                .filter(|metadata| {
                    metadata
                        .network_traces
                        .iter()
                        .any(|trace| trace.end_time.is_none())
                })
                .map(|metadata| metadata.id.to_owned());

            let messages = self
                .get_client_state(&msg.sender)
                .map(|source| {
                    let recipients = recipients
                        .into_iter()
                        .filter_map(|client| self.get_client_state(&client.id))
                        .map(|state| state.to_owned())
                        .collect::<Vec<_>>();

                    // TODO: record the actual recipients. In other words, not just
                    // the role that it was sent to but the actual user who was occupying
                    // the role
                    recording_ids
                        .into_iter()
                        .map(|project_id| {
                            SentMessage::new(
                                project_id,
                                source.to_owned(),
                                recipients.clone(),
                                msg.content.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if !messages.is_empty() {
                let res = app.recorded_messages.insert_many(&messages, None).await;
                if let Err(err) = res {
                    warn!("Failed to record sent message: {}", err);
                }
            }

            app.metrics.record_msg_sent();
        }
    }

    /// Get the allowed recipients of a message. If the recipient is a
    /// member of a group, ensure that the sender can message that group.
    #[allow(dead_code)] // This is temporarily disabled until msg filtering is fleshed out further
    async fn allowed_recipients<'a>(
        &self,
        app: &AppData,
        sender: &ClientId,
        recipients: impl Iterator<Item = &'a Client> + Clone,
    ) -> Vec<&'a Client> {
        let sender = self.usernames.get(sender);
        let recipient_names: HashSet<_> = recipients
            .clone()
            .filter_map(|rcp| self.usernames.get(&rcp.id))
            .cloned()
            .collect();
        let members = app.keep_members(recipient_names).await.unwrap_or_default();

        let deny_list: HashSet<_> = if let Some(sender) = sender {
            if app.is_admin(sender).await {
                // allow messages from admins
                std::iter::empty().collect()
            } else {
                // message only allowed from group member/owner
                join_all(members.into_iter().map(|member| async {
                    // TODO: do we need this?
                    let friends = app.get_friends(&member).await.unwrap_or_default();
                    (member, friends.contains(sender))
                }))
                .await
                .into_iter()
                .filter_map(|(rec_name, is_friend)| if !is_friend { Some(rec_name) } else { None })
                .collect()
            }
        } else {
            // messages to any group member will be blocked
            members.into_iter().collect()
        };

        recipients
            .into_iter()
            .filter(|rec| {
                self.usernames
                    .get(&rec.id)
                    .map(|username| !deny_list.contains(username))
                    .unwrap_or(true)
            })
            .collect()
    }

    fn has_client(&self, id: &ClientId) -> bool {
        self.clients.contains_key(id)
    }

    pub fn disconnect_client(&self, id: &ClientId) {
        if let Some(client) = self.clients.get(id) {
            if let Err(err) = client.addr.do_send(ClientCommand::Close) {
                log::error!("Unable to send close command to client: {}", err);
            }
        }
    }

    pub fn set_client_username(&mut self, client_id: &ClientId, username: Option<String>) {
        self.usernames.remove(client_id);
        if let Some(username) = username {
            self.usernames.insert(client_id.clone(), username);
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
        self.set_client_username(&msg.id, msg.username);

        match &msg.state {
            ClientState::Browser(state) => {
                let room = self
                    .rooms
                    .entry(state.project_id.clone())
                    .or_insert(ProjectNetwork::new(state.project_id.to_owned()));

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
                    .entry(state.app_id.to_owned())
                    .or_insert_with(HashMap::new);

                app_net.insert(state.address.to_owned(), msg.id.to_owned());
            }
        }
        self.states.insert(msg.id, msg.state);
    }

    pub fn add_client(&mut self, msg: AddClient) {
        let client = Client::new(msg.id.clone(), msg.addr);
        self.clients.insert(msg.id, client);
        let app_data = &self.app_data;
        if let Some(app_data) = app_data {
            app_data
                .metrics
                .record_connected_clients(self.clients.len());
        }
    }

    pub async fn set_broken_client(&mut self, msg: BrokenClient) -> Result<(), InternalError> {
        if let Some(app) = &self.app_data {
            if let Some(ClientState::Browser(state)) = self.states.get(&msg.id) {
                let query = doc! {
                    "id": &state.project_id,
                    "saveState": SaveState::Transient
                };
                let update = doc! {"$set": {"saveState": SaveState::Broken}};
                app.project_metadata
                    .update_one(query, update, None)
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;
            }
        }

        Ok(())
        // TODO: Record a list of broken clients for the project?
    }

    pub async fn remove_client(&mut self, msg: RemoveClient) {
        self.clients.remove(&msg.id);
        self.reset_client_state(&msg.id, None).await;

        let app_data = &self.app_data;
        if let Some(app_data) = app_data {
            app_data
                .metrics
                .record_connected_clients(self.clients.len());
        }
    }

    async fn reset_client_state(
        &mut self,
        id: &ClientId,
        new_project_id: Option<ProjectId>,
    ) -> Option<ClientState> {
        self.usernames.remove(id);
        let state = self.states.remove(id);
        match &state {
            Some(ClientState::Browser(state)) => {
                let room = self.rooms.get_mut(&state.project_id);
                let mut empty: Vec<_> = Vec::new();
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
                            if let Err(error) = self.remove_room(&state.project_id).await {
                                warn!(
                                    "Unable to remove project {}: {:?}",
                                    &state.project_id, error
                                );
                            }
                            update_needed = false;
                        } else {
                            // remove the role
                            self.rooms
                                .get_mut(&state.project_id)
                                .and_then(|room| room.roles.remove(&state.role_id));
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
        state
    }

    async fn remove_room(&mut self, project_id: &ProjectId) -> Result<(), InternalError> {
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
                .map_err(InternalError::DatabaseConnectionError)?
                .map(|md| match md.save_state {
                    SaveState::Created => unreachable!(), // Cannot reach here since this is triggered when the last user leaves
                    SaveState::Transient => ProjectCleanup::Immediately,
                    SaveState::Broken => ProjectCleanup::Delayed,
                    SaveState::Saved => ProjectCleanup::None,
                })
                .unwrap_or(ProjectCleanup::None);

            match cleanup {
                ProjectCleanup::Immediately => {
                    let actions = app.as_project_actions();
                    let system_auth = auth::try_manage_system(self);
                    let dp =
                        auth::DeleteProject::from_manage_system(&system_auth, project_id.clone());

                    if let Err(err) = actions.delete_project(&dp).await {
                        log::error!("Unable to delete project {}: {}", project_id, &err);
                    }
                }
                ProjectCleanup::Delayed => {
                    let ten_minutes = Duration::new(10 * 60, 0);
                    let delete_at = SystemTime::now() + ten_minutes;
                    let update = doc! {"$set": {
                        "deleteAt": DateTime::from_system_time(delete_at)}
                    };

                    // FIXME: this should call delete_project since it:
                    //   - can leave data on s3 if deleted by MongoDB
                    //   - won't invalidate the cache
                    // We need to remove the index from app data
                    app.project_metadata
                        .update_one(query, update, None)
                        .await
                        .map_err(InternalError::DatabaseConnectionError)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn send_room_state_for(&mut self, project_id: &ProjectId) {
        if let Some(app) = &self.app_data {
            let query = doc! {"id": project_id};
            if let Some(project) = app
                .project_metadata
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)
                .ok()
                .flatten()
            {
                self.send_room_state(SendRoomState { project });
            }
        }
    }

    pub fn send_room_state(&mut self, msg: SendRoomState) {
        // The room changed so the address cache may contain stale data
        // (ie, the room or role may have been renamed - or the occupancy changed)
        self.invalidate_cached_addresses(&msg.project);

        if let Some(room) = self.rooms.get(&msg.project.id) {
            let clients = room
                .roles
                .values()
                .flatten()
                .filter_map(|id| self.clients.get(id));

            let room_state = room.get_state(msg.project, &self.usernames);
            clients.for_each(|client| {
                if let Err(err) = client.addr.do_send(room_state.clone().into()) {
                    log::error!("Unable to send room state to client: {}", err);
                }
            });
        }
    }

    fn reset_address_cache(&mut self) {
        self.address_cache = Arc::new(RwLock::new(LruCache::new(self.cache_size)));
    }

    /// Invalidate the cached addresses for the given project as it (or the
    // occupancy) has changed.
    fn invalidate_cached_addresses(&mut self, project: &ProjectMetadata) {
        // reset the whole cache if mutex is poisoned
        if self.address_cache.is_poisoned() {
            self.reset_address_cache();
        }

        let invalidate_result = self.address_cache.write().map(|mut address_cache| {
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
        });

        if let Err(err) = invalidate_result {
            log::error!("Unable to invalidate address cache: {}", err);
        }
    }

    pub fn get_role_request(&self, state: BrowserClientState) -> Option<RoleRequest> {
        self.rooms
            .get(&state.project_id)
            .and_then(|room| room.roles.get(&state.role_id))
            .and_then(|client_ids| client_ids.first())
            .and_then(|id| self.clients.get(id))
            .map(|client| RoleRequest::new(client.addr.clone()))
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

    pub async fn evict_client(&mut self, id: ClientId) -> Option<ClientState> {
        let username = self.usernames.remove(&id);
        let state = self.reset_client_state(&id, None).await;
        self.clients
            .get(&id)
            .map(|client| client.addr.do_send(EvictionNotice.into()));

        if let Some(username) = username {
            if self.usernames.get(&id).is_none() {
                self.usernames.insert(id, username);
            }
        }

        state
    }

    pub fn get_client_state(&self, id: &ClientId) -> Option<&ClientState> {
        self.states.get(id)
    }

    pub fn get_client_username(&self, id: &ClientId) -> Option<&String> {
        self.usernames.get(id)
    }

    /// Get info about a client. Returns None if no client connected.
    pub(crate) fn get_client_info(&self, id: &ClientId) -> Option<api::ClientInfo> {
        self.has_client(id).then(|| api::ClientInfo {
            username: self.get_client_username(id).cloned(),
            state: self.get_client_state(id).cloned(),
        })
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
            if let Err(err) = client.addr.do_send(msg.clone().into()) {
                warn!("Unable to send invite to client: {}", err);
            }
        });
    }

    pub async fn send_msg_from_services(&self, msg: api::SendMessage) {
        let recipients = match msg.target {
            api::SendMessageTarget::Address { address } => {
                if let Ok(address) = ClientAddress::from_str(&address) {
                    self.get_clients_at(address).await
                } else {
                    Vec::new()
                }
            }
            api::SendMessageTarget::Client { state, client_id } => {
                let current_state = self.states.get(&client_id);
                let has_state = match state {
                    Some(state) => current_state.map(|s| s == &state).unwrap_or(false),
                    None => true,
                };

                let mut clients = Vec::new();
                if let Some(client) = self.clients.get(&client_id) {
                    if has_state {
                        clients.push(client);
                    }
                }
                clients
            }
            api::SendMessageTarget::Role {
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
                .unwrap_or_default(),
            api::SendMessageTarget::Room { project_id } => self
                .rooms
                .get(&project_id)
                .map(|room| {
                    let client_ids = room.roles.values().flatten();
                    client_ids
                        .filter_map(|id| self.clients.get(id))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(Vec::new),
        };

        let message = ClientCommand::SendMessage(msg.content);
        recipients.iter().for_each(|client| {
            if let Err(err) = client.addr.do_send(message.clone()) {
                log::error!("Unable to send message to client: {}", err);
            }
        });
    }

    pub fn send_ide_msg(&self, msg: SendIDEMessage) {
        let recipients = msg
            .addresses
            .iter()
            .filter_map(|client_id| self.clients.get(client_id));

        let message = ClientCommand::SendMessage(msg.content);
        recipients.for_each(|client| {
            if let Err(err) = client.addr.do_send(message.clone()) {
                log::error!("Unable to send IDE message to client: {}", err);
            }
        });
    }

    pub fn send_to_user(&self, msg: Value, username: &str) {
        let recipients = self
            .usernames
            .iter()
            .filter_map(|(client_id, name)| {
                if name == username {
                    Some(client_id)
                } else {
                    None
                }
            })
            .filter_map(|client_id| self.clients.get(client_id));

        let message = ClientCommand::SendMessage(msg);
        recipients.for_each(|client| {
            if let Err(err) = client.addr.do_send(message.clone()) {
                log::error!("Unable to send message to user: {}", err);
            }
        });
    }

    pub fn send_to_room(&self, msg: Value, id: &ProjectId) {
        let recipients = self
            .rooms
            .get(id)
            .map(|room| {
                room.roles
                    .values()
                    .flatten()
                    .filter_map(|id| self.clients.get(id))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let message = ClientCommand::SendMessage(msg);
        recipients.into_iter().for_each(|client| {
            if let Err(err) = client.addr.do_send(message.clone()) {
                log::error!(
                    "Unable to send message to client (sending to room): {}",
                    err
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use netsblox_cloud_common::{
        api::{self, AppId, ClientId, ClientState, ExternalClientState},
        Group, User,
    };
    use serde_json::json;

    use crate::{
        network::topology::{SendMessage, SetStorage},
        test_utils,
    };

    use super::Topology;

    #[actix_web::test]
    #[ignore]
    async fn test_remove_client_clear_state() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_remove_client_clear_external_state() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_allowed_recipients_for_member() {
        let outsider: User = api::NewUser {
            username: "outsider".to_string(),
            email: "outsider@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let outsider_id = ClientId::new("_outsider".into());
        // FIXME: this will be used once the test is finished
        let _outsider_state = ClientState::External(ExternalClientState {
            address: String::from("OutsiderAddress"),
            app_id: AppId::new("TestApp"),
        });

        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let owner_id = ClientId::new("_owner".into());
        let group = Group::new(owner.username.clone(), "some_group".into());
        let member: User = api::NewUser {
            username: "member".to_string(),
            email: "member@netsblox.org".into(),
            password: None,
            group_id: Some(group.id),
            role: None,
        }
        .into();
        let member_id = ClientId::new("_member".into());

        test_utils::setup()
            .with_users(&[owner, member.clone(), outsider.clone()])
            .run(|app_data| async move {
                let topology = Topology::new(NonZeroUsize::new(10).unwrap());
                // topology.set_app_data(app_data);

                // TODO: mock the clients?
                // TODO: how to create a client
                // let addr = Addr::recipient();
                // let owner_client = Client::new(owner_id, addr);

                let recipients = vec![]; // TODO: create `Client`s in the list of recipients
                let recipients = topology
                    .allowed_recipients(&app_data, &outsider_id, recipients.iter())
                    .await;

                assert!(
                    !recipients.iter().any(|rec| rec.id == member_id),
                    "Member was allowed recipient for outsider"
                );
                assert!(
                    recipients.iter().any(|rec| rec.id == owner_id),
                    "Messages blocked from outsider to group owner"
                );
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_filter_blocked_users() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_filter_group_msgs() {
        todo!();
    }

    #[actix_web::test]
    async fn test_send_msg_no_state() {
        let client = test_utils::network::Client::new(None, None);

        test_utils::setup()
            .with_clients(&[client.clone()])
            .run(|app_data| async move {
                app_data
                    .network
                    .send(SetStorage {
                        app_data: app_data.clone(),
                    })
                    .await
                    .unwrap();

                app_data
                    .network
                    .send(SendMessage {
                        sender: client.id.clone(),
                        addresses: Vec::new(),
                        content: json!({}),
                    })
                    .await
                    .unwrap();

                // Ensure we can send a second msg and the thread hasn't crashed
                app_data
                    .network
                    .send(SendMessage {
                        sender: client.id,
                        addresses: Vec::new(),
                        content: json!({}),
                    })
                    .await
                    .unwrap();
            })
            .await;
    }

    // TODO: Add test for broken connections?
}
