pub(crate) mod metrics;

use crate::common::api::{
    oauth, LibraryMetadata, NewUser, ProjectId, PublishState, RoleId, UserRole,
};
use crate::{network, projects};
//pub use self::
use actix_web::rt::time;
use futures::future::join_all;
use futures::join;
use lazy_static::lazy_static;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use log::{error, info, warn};
use lru::LruCache;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOptions, IndexOptions, ReturnDocument};
use netsblox_cloud_common::api::{FriendInvite, FriendLinkState, GroupId};
use rusoto_core::credential::StaticProvider;
use rusoto_core::Region;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::RwLock as AsyncRwLock;
use uuid::Uuid;

use crate::common::api::{RoleData, SaveState};
use crate::common::{
    AuthorizedServiceHost, BannedAccount, CollaborationInvite, FriendLink, Group, Library,
    OAuthClient, OAuthToken, Project, ProjectMetadata, SetPasswordToken, User,
};
use crate::common::{OccupantInvite, RoleMetadata, SentMessage};
use crate::config::Settings;
use crate::errors::{InternalError, UserError};
use crate::libraries;
use crate::network::topology::{self, SetStorage, TopologyActor};
use actix::{Actor, Addr};
use futures::TryStreamExt;
use mongodb::{Client, Collection, IndexModel};
use rusoto_s3::{
    CreateBucketRequest, DeleteObjectRequest, GetObjectRequest, PutObjectOutput, PutObjectRequest,
    S3Client, S3,
};

// This is lazy_static to ensure it is shared between threads
// TODO: it would be nice to be able to configure the cache size from the settings
// TODO: move the cache to something that isn't shared btwn tests...
lazy_static! {
    static ref MEMBERSHIP_CACHE: Arc<AsyncRwLock<LruCache<String, bool>>> =
        Arc::new(AsyncRwLock::new(LruCache::new(1000)));
}
lazy_static! {
    static ref ADMIN_CACHE: Arc<AsyncRwLock<LruCache<String, bool>>> =
        Arc::new(AsyncRwLock::new(LruCache::new(1000)));
}

lazy_static! {
    static ref PROJECT_CACHE: Arc<RwLock<LruCache<ProjectId, ProjectMetadata>>> =
        Arc::new(RwLock::new(LruCache::new(500)));
}
lazy_static! {
    static ref FRIEND_CACHE: Arc<RwLock<LruCache<String, Vec<String>>>> =
        Arc::new(RwLock::new(LruCache::new(1000)));
}

#[derive(Clone)]
pub struct AppData {
    bucket: String,
    tor_exit_nodes: Collection<TorNode>,
    s3: S3Client,
    pub(crate) settings: Settings,
    pub(crate) network: Addr<TopologyActor>,
    pub(crate) groups: Collection<Group>,
    pub(crate) users: Collection<User>,
    pub(crate) banned_accounts: Collection<BannedAccount>,
    friends: Collection<FriendLink>,
    pub(crate) project_metadata: Collection<ProjectMetadata>,
    pub(crate) library_metadata: Collection<LibraryMetadata>,
    pub(crate) libraries: Collection<Library>,
    pub(crate) authorized_services: Collection<AuthorizedServiceHost>,

    pub(crate) password_tokens: Collection<SetPasswordToken>,
    pub(crate) recorded_messages: Collection<SentMessage>,
    pub(crate) collab_invites: Collection<CollaborationInvite>,
    pub(crate) occupant_invites: Collection<OccupantInvite>,

    pub(crate) oauth_clients: Collection<OAuthClient>,
    pub(crate) oauth_tokens: Collection<OAuthToken>,
    pub(crate) oauth_codes: Collection<oauth::Code>,

    pub(crate) metrics: metrics::Metrics,
    mailer: SmtpTransport,
    sender: Mailbox,
}

impl AppData {
    pub fn new(
        client: Client,
        settings: Settings,
        network: Option<Addr<TopologyActor>>,
        prefix: Option<&str>,
    ) -> AppData {
        // Blob storage
        let region = Region::Custom {
            name: settings.s3.region_name.clone(),
            endpoint: settings.s3.endpoint.clone(),
        };
        let s3 = S3Client::new_with(
            rusoto_core::request::HttpClient::new().expect("Failed to create HTTP client"),
            StaticProvider::new(
                settings.s3.credentials.access_key.clone(),
                settings.s3.credentials.secret_key.clone(),
                None,
                None,
            ),
            region,
        );

        // Email
        let credentials = Credentials::new(
            settings.email.smtp.username.clone(),
            settings.email.smtp.password.clone(),
        );
        let mailer = SmtpTransport::relay(&settings.email.smtp.host)
            .expect("Unable to connect to SMTP host.")
            .credentials(credentials)
            .build();
        let sender = settings
            .email
            .sender
            .parse()
            .expect("Invalid sender email address.");

        // Database collections
        let db = client.database(&settings.database.name);
        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let password_tokens =
            db.collection::<SetPasswordToken>(&(prefix.to_owned() + "passwordTokens"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        let banned_accounts =
            db.collection::<BannedAccount>(&(prefix.to_owned() + "bannedAccounts"));
        let project_metadata = db.collection::<ProjectMetadata>(&(prefix.to_owned() + "projects"));
        let library_metadata = db.collection::<LibraryMetadata>(&(prefix.to_owned() + "libraries"));
        let libraries = db.collection::<Library>(&(prefix.to_owned() + "libraries"));
        let authorized_services =
            db.collection::<AuthorizedServiceHost>(&(prefix.to_owned() + "authorizedServices"));
        let collab_invites =
            db.collection::<CollaborationInvite>(&(prefix.to_owned() + "collaborationInvitations"));
        let occupant_invites =
            db.collection::<OccupantInvite>(&(prefix.to_owned() + "occupantInvites"));
        let friends = db.collection::<FriendLink>(&(prefix.to_owned() + "friends"));
        let recorded_messages =
            db.collection::<SentMessage>(&(prefix.to_owned() + "recordedMessages"));
        let network = network.unwrap_or_else(|| TopologyActor::new().start());
        let oauth_clients = db.collection::<OAuthClient>(&(prefix.to_owned() + "oauthClients"));
        let oauth_tokens = db.collection::<OAuthToken>(&(prefix.to_owned() + "oauthToken"));
        let oauth_codes = db.collection::<oauth::Code>(&(prefix.to_owned() + "oauthCode"));
        let tor_exit_nodes = db.collection::<TorNode>(&(prefix.to_owned() + "torExitNodes"));
        let bucket = settings.s3.bucket.clone();

        AppData {
            settings,
            network,
            s3,
            bucket,
            groups,
            users,
            banned_accounts,
            project_metadata,
            library_metadata,
            libraries,
            authorized_services,

            collab_invites,
            occupant_invites,
            password_tokens,
            friends,

            mailer,
            sender,

            oauth_clients,
            oauth_tokens,
            oauth_codes,

            metrics: metrics::Metrics::new(),

            tor_exit_nodes,
            recorded_messages,
        }
    }

    pub async fn initialize(&self) -> Result<(), InternalError> {
        // Create the s3 bucket
        let bucket = &self.settings.s3.bucket;
        let request = CreateBucketRequest {
            bucket: bucket.clone(),
            ..Default::default()
        };

        // FIXME: check if bucket exists or invalid bucket name
        if self.s3.create_bucket(request).await.is_err() {
            info!("Using existing s3 bucket.")
        };

        // Add database indexes
        let index_opts = IndexOptions::builder()
            .expire_after(Duration::from_secs(60 * 60))
            .build();
        let occupant_invite_indexes = vec![
            IndexModel::builder()
                .keys(doc! {"createdAt": 1})
                .options(index_opts)
                .build(),
            IndexModel::builder()
                .keys(doc! {"project_id": 1, "role_id": 1})
                .build(),
        ];
        self.occupant_invites
            .create_indexes(occupant_invite_indexes, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let index_opts = IndexOptions::builder()
            .expire_after(Duration::from_secs(60 * 60))
            .build();
        let token_index = IndexModel::builder()
            .keys(doc! {"createdAt": 1})
            .options(index_opts)
            .build();
        self.password_tokens
            .create_index(token_index, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        self.project_metadata
            .create_indexes(
                vec![
                    IndexModel::builder().keys(doc! {"id": 1}).build(),
                    // delete broken projects after a delay
                    IndexModel::builder()
                        .keys(doc! {"deleteAt": 1})
                        .options(
                            IndexOptions::builder()
                                .expire_after(Duration::from_secs(10))
                                .sparse(true)
                                .build(),
                        )
                        .build(),
                    // delete transient projects after 1 week
                    IndexModel::builder()
                        .keys(doc! { "originTime": 1})
                        .options(
                            IndexOptions::builder()
                                .expire_after(Duration::from_secs(60 * 60 * 24 * 7))
                                .partial_filter_expression(doc! {"saveState": SaveState::Transient})
                                .background(true)
                                .build(),
                        )
                        .build(),
                ],
                None,
            )
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        self.tor_exit_nodes
            .create_index(IndexModel::builder().keys(doc! {"addr": 1}).build(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        self.network.do_send(SetStorage {
            app_data: self.clone(),
        });

        if !self.settings.security.allow_tor_login {
            self.start_update_interval();
        }

        if let Some(admin) = self.settings.admin.as_ref() {
            let user: User = NewUser {
                username: admin.username.to_owned(),
                password: Some(admin.password.to_owned()),
                email: admin.email.to_owned(),
                group_id: None,
                role: Some(UserRole::Admin),
            }
            .into();

            let query = doc! {"username": &user.username};
            let update = doc! {"$setOnInsert": &user};
            let options = mongodb::options::FindOneAndUpdateOptions::builder()
                .upsert(true)
                .build();

            self.users
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
        }

        Ok(())
    }

    fn start_update_interval(&self) {
        let tor_exit_nodes = self.tor_exit_nodes.clone();
        actix_web::rt::spawn(async move {
            let one_day = Duration::from_secs(60 * 60 * 24);
            let mut interval = time::interval(one_day);
            loop {
                if let Err(error) = update_tor_nodes(&tor_exit_nodes).await {
                    warn!("Unable to update Tor nodes: {:?}", error);
                }
                interval.tick().await;
            }
        });
    }

    pub async fn get_project_metadatum(
        &self,
        id: &ProjectId,
    ) -> Result<ProjectMetadata, UserError> {
        match self.get_cached_project(id) {
            Some(project) => Ok(project),
            None => self.get_project_and_cache(id).await,
        }
    }

    pub fn update_project_cache(&self, metadata: ProjectMetadata) {
        let mut cache = PROJECT_CACHE.write().unwrap();
        cache.put(metadata.id.clone(), metadata);
    }

    fn get_cached_project_metadata<'a>(
        &self,
        ids: impl Iterator<Item = &'a ProjectId>,
    ) -> (Vec<ProjectMetadata>, Vec<&'a ProjectId>) {
        let mut results = Vec::new();
        let mut missing_projects = Vec::new();
        let mut cache = PROJECT_CACHE.write().unwrap();
        for id in ids {
            match cache.get(id) {
                Some(project_metadata) => results.push(project_metadata.clone()),
                None => missing_projects.push(id),
            }
        }
        (results, missing_projects)
    }

    pub async fn get_project_metadata<'a>(
        &self,
        ids: impl Iterator<Item = &'a ProjectId>,
    ) -> Result<Vec<ProjectMetadata>, UserError> {
        let (mut results, missing_projects) = self.get_cached_project_metadata(ids);

        if !missing_projects.is_empty() {
            let docs: Vec<Document> = missing_projects.iter().map(|id| doc! {"id": id}).collect();
            let query = doc! {"$or": docs};
            let cursor = self
                .project_metadata
                .find(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            let projects: Vec<_> = cursor
                .try_collect::<Vec<_>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            let mut cache = PROJECT_CACHE.write().unwrap();
            projects.iter().for_each(|project| {
                cache.put(project.id.clone(), project.clone());
            });
            results.extend(projects);
        }

        Ok(results)
    }

    fn get_cached_project(&self, id: &ProjectId) -> Option<ProjectMetadata> {
        let mut cache = PROJECT_CACHE.write().unwrap();
        cache.get(id).map(|md| md.to_owned())
    }

    async fn get_project_and_cache(&self, id: &ProjectId) -> Result<ProjectMetadata, UserError> {
        let metadata = self
            .project_metadata
            .find_one(doc! {"id": id}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        let mut cache = PROJECT_CACHE.write().unwrap();
        cache.put(id.clone(), metadata);
        Ok(cache.get(id).unwrap().clone())
    }

    /// Get a unique project name for the given user and preferred name.
    pub async fn get_valid_project_name(
        &self,
        owner: &str,
        basename: &str,
    ) -> Result<String, UserError> {
        projects::ensure_valid_name(basename)?;

        let query = doc! {"owner": &owner};
        let cursor = self
            .project_metadata
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let project_names = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .iter()
            .map(|md| md.name.to_owned())
            .collect();

        Ok(get_unique_name(project_names, basename))
    }

    pub async fn import_project(
        &self,
        owner: &str,
        name: &str,
        roles: &mut HashMap<RoleId, RoleData>,
        save_state: Option<SaveState>,
    ) -> Result<ProjectMetadata, UserError> {
        let unique_name = self.get_valid_project_name(owner, name).await?;

        // Prepare the roles (ensure >=1 exists; upload them)
        if roles.is_empty() {
            roles.insert(
                RoleId::new(Uuid::new_v4().to_string()),
                RoleData {
                    name: "myRole".to_owned(),
                    code: "".to_owned(),
                    media: "".to_owned(),
                },
            );
        };

        let role_mds: Vec<RoleMetadata> = join_all(
            roles
                .values()
                .map(|role| self.upload_role(owner, &unique_name, role)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<RoleMetadata>, _>>()?;

        let roles: HashMap<RoleId, RoleMetadata> =
            roles.keys().cloned().zip(role_mds.into_iter()).collect();

        let save_state = save_state.unwrap_or(SaveState::Created);
        let metadata = ProjectMetadata::new(owner, &unique_name, roles, save_state);
        self.project_metadata
            .insert_one(metadata.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(metadata)
    }

    async fn upload_role(
        &self,
        owner: &str,
        project_name: &str,
        role: &RoleData,
    ) -> Result<RoleMetadata, UserError> {
        let is_guest = owner.starts_with('_');
        let top_level = if is_guest { "guests" } else { "users" };
        let basepath = format!("{}/{}/{}/{}", top_level, owner, project_name, &role.name);
        let src_path = format!("{}/code.xml", &basepath);
        let media_path = format!("{}/media.xml", &basepath);

        self.upload(&media_path, role.media.to_owned()).await?;
        self.upload(&src_path, role.code.to_owned()).await?;

        Ok(RoleMetadata {
            name: role.name.to_owned(),
            code: src_path,
            media: media_path,
            updated: DateTime::now(),
        })
    }

    async fn delete(&self, key: String) -> Result<(), UserError> {
        let request = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            key,
            ..Default::default()
        };

        self.s3
            .delete_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)?;

        Ok(())
    }

    async fn upload(&self, key: &str, body: String) -> Result<PutObjectOutput, InternalError> {
        let request = PutObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            body: Some(String::into_bytes(body).into()),
            ..Default::default()
        };
        self.s3.put_object(request).await.map_err(|err| {
            warn!("Unable to upload to s3: {}", err);
            InternalError::S3Error
        })
    }

    async fn download(&self, key: &str) -> Result<String, InternalError> {
        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            key: String::from(key),
            ..Default::default()
        };

        let output = self
            .s3
            .get_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)?;
        let byte_str = output
            .body
            .unwrap()
            .map_ok(|b| b.to_vec())
            .try_concat()
            .await
            .map_err(|_err| InternalError::S3ContentError)?;

        String::from_utf8(byte_str).map_err(|_err| InternalError::S3ContentError)
    }

    pub async fn fetch_project(&self, metadata: &ProjectMetadata) -> Result<Project, UserError> {
        let (keys, values): (Vec<_>, Vec<_>) = metadata.roles.clone().into_iter().unzip();
        // TODO: make fetch_role fallible
        let role_data = join_all(values.iter().map(|v| self.fetch_role(v))).await;

        let roles = keys
            .into_iter()
            .zip(role_data)
            .filter_map(|(k, data)| data.map(|d| (k, d)).ok())
            .collect::<HashMap<RoleId, _>>();

        Ok(Project {
            id: metadata.id.to_owned(),
            name: metadata.name.to_owned(),
            owner: metadata.owner.to_owned(),
            updated: metadata.updated.to_owned(),
            state: metadata.state.to_owned(),
            collaborators: metadata.collaborators.to_owned(),
            origin_time: metadata.origin_time,
            save_state: metadata.save_state.to_owned(),
            roles,
        })
    }

    pub async fn delete_project(
        &self,
        metadata: ProjectMetadata,
    ) -> Result<ProjectMetadata, UserError> {
        let query = doc! {"id": &metadata.id};
        let metadata = self
            .project_metadata
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        let paths = metadata
            .roles
            .clone()
            .into_values()
            .flat_map(|role| vec![role.code, role.media]);

        join_all(paths.map(move |path| self.delete(path)))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        // TODO: send update to any current occupants
        Ok(metadata)
    }

    pub async fn fetch_role(&self, metadata: &RoleMetadata) -> Result<RoleData, InternalError> {
        let (code, media) = join!(
            self.download(&metadata.code),
            self.download(&metadata.media),
        );
        Ok(RoleData {
            name: metadata.name.to_owned(),
            code: code?,
            media: media?,
        })
    }

    /// Send updated room state and update project cache when room structure is changed or renamed
    pub fn on_room_changed(&self, updated_project: ProjectMetadata) {
        self.network.do_send(topology::SendRoomState {
            project: updated_project.clone(),
        });

        self.update_project_cache(updated_project);
    }

    pub async fn save_role(
        &self,
        metadata: &ProjectMetadata,
        role_id: &RoleId,
        role: RoleData,
    ) -> Result<ProjectMetadata, UserError> {
        let role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role)
            .await?;

        // check if the (public) project needs to be re-approved
        let state = match metadata.state {
            PublishState::Public => {
                let needs_approval = libraries::is_approval_required(&role.code);
                if needs_approval {
                    PublishState::PendingApproval
                } else {
                    PublishState::Public
                }
            }
            _ => metadata.state.clone(),
        };

        let query = doc! {"id": &metadata.id};
        let update = doc! {
            "$set": {
                &format!("roles.{}", role_id): role_md,
                "saveState": SaveState::Saved,
                "state": state,
            }
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());

        Ok(updated_metadata)
    }

    pub async fn create_role(
        &self,
        metadata: ProjectMetadata,
        role_data: RoleData,
    ) -> Result<ProjectMetadata, UserError> {
        let mut role_md = self
            .upload_role(&metadata.owner, &metadata.name, &role_data)
            .await?;

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let role_names = metadata
            .roles
            .into_values()
            .map(|r| r.name)
            .collect::<Vec<_>>();
        let role_name = get_unique_name(role_names, &role_md.name);
        role_md.name = role_name;

        let role_id = Uuid::new_v4();
        let query = doc! {"id": metadata.id};
        let update = doc! {"$set": {&format!("roles.{}", role_id): role_md}};
        let updated_metadata = self
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        self.on_room_changed(updated_metadata.clone());
        Ok(updated_metadata)
    }

    // Membership queries (cached)
    pub async fn keep_members(&self, usernames: HashSet<String>) -> Result<Vec<String>, UserError> {
        let cache = MEMBERSHIP_CACHE.write().await;
        let unknown_users: Vec<_> = usernames
            .iter()
            .filter(|name| !cache.contains(*name))
            .collect();
        drop(cache); // don't hold the lock over the upcoming async boundary

        let unknown_count = unknown_users.len() as i64;
        if unknown_count > 0 {
            let opts = FindOptions::builder().limit(Some(unknown_count)).build();
            let query = doc! {"$or":
                unknown_users.into_iter().map(|name| doc!{"username": name}).collect::<Vec<_>>()
            };
            let users = self
                .users
                .find(query, opts)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            let mut cache = MEMBERSHIP_CACHE.write().await;
            users.into_iter().for_each(|usr| {
                cache.put(usr.username, usr.group_id.is_some());
            });
        }

        let mut cache = MEMBERSHIP_CACHE.write().await;
        let members: Vec<_> = usernames
            .into_iter()
            .filter(|name| {
                // Although unlikely, it's possible that the entries
                // have been invalidated from the cache while looking up
                // the unknown users. In this case, we will be conservative
                // and just assume they are members.
                let is_member = cache.get(name).unwrap_or(&true);
                *is_member
            })
            .collect();

        Ok(members)
    }

    // Cached admin-checking
    pub async fn is_admin(&self, username: &str) -> bool {
        let cache = ADMIN_CACHE.write().await;
        let needs_lookup = !cache.contains(username);
        drop(cache); // don't hold the lock during the database query

        if needs_lookup {
            let query = doc! {"username": &username};
            let is_admin = self
                .users
                .find_one(query, None)
                .await
                .map_err(|err| {
                    error!("Database error: {:?}", err);
                    InternalError::DatabaseConnectionError(err)
                })
                .ok()
                .flatten()
                .map(|user| matches!(user.role, UserRole::Admin))
                .unwrap_or(false);

            let mut cache = ADMIN_CACHE.write().await;
            cache.put(username.to_owned(), is_admin);
        }

        let mut cache = ADMIN_CACHE.write().await;
        cache
            .get(username)
            .map(|is_admin| is_admin.to_owned())
            .unwrap_or(false)
    }

    // Friend-related features
    fn get_cached_friends(&self, username: &str) -> Option<Vec<String>> {
        let mut cache = FRIEND_CACHE.write().unwrap();
        cache.get(username).map(|friends| friends.to_owned())
    }

    async fn lookup_friends(&self, username: &str) -> Result<Vec<String>, UserError> {
        let query = doc! {"username": &username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let is_universal_friend = matches!(user.role, UserRole::Admin);

        let friend_names: Vec<_> = if is_universal_friend {
            self.users
                .find(doc! {}, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .try_collect::<Vec<User>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .into_iter()
                .map(|user| user.username)
                .filter(|name| name != username)
                .collect()
        } else if let Some(group_id) = user.group_id {
            // get owner + all members
            let query = doc! {"id": &group_id};
            let group = self
                .groups
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::GroupNotFoundError)?;
            let members = self.lookup_members(std::iter::once(&group_id)).await?;

            std::iter::once(group.owner)
                .chain(members.into_iter().map(|user| user.username))
                .collect()
        } else {
            // look up:
            //   - members of any group we own
            //   - accepted friend requests/links
            let query = doc! {"owner": &username};
            let groups = self
                .groups
                .find(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            let group_ids = groups.into_iter().map(|group| group.id);
            let members = self.lookup_members(group_ids).await?;

            let query = doc! {"$or": [
                {"sender": &username, "state": FriendLinkState::Approved},
                {"recipient": &username, "state": FriendLinkState::Approved}
            ]};
            let cursor = self
                .friends
                .find(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            let links = cursor
                .try_collect::<Vec<_>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            links
                .into_iter()
                .map(|l| {
                    if l.sender == username {
                        l.recipient
                    } else {
                        l.sender
                    }
                })
                .chain(members.into_iter().map(|user| user.username))
                .collect()
        };

        Ok(friend_names)
    }

    async fn lookup_members<T>(
        &self,
        group_ids: impl Iterator<Item = T>,
    ) -> Result<Vec<User>, UserError>
    where
        T: Borrow<GroupId>,
    {
        let member_queries: Vec<_> = group_ids.map(|id| doc! {"groupId": id.borrow()}).collect();
        if !member_queries.is_empty() {
            let query = doc! {"$or": member_queries};

            let members = self
                .users
                .find(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            Ok(members)
        } else {
            Ok(Vec::new())
        }
    }

    /// Invalidate the relevant cached values when a user is added or removed
    /// from a group
    pub async fn group_members_updated(&self, group_id: &GroupId) {
        if let Ok(members) = self.lookup_members(std::iter::once(group_id)).await {
            let mut cache = FRIEND_CACHE.write().unwrap();
            members.into_iter().for_each(|user| {
                cache.pop(&user.username);
            });
        } else {
            error!("Error occurred while retrieving members for {}", group_id);
        }
    }

    pub async fn get_friends(&self, username: &str) -> Result<Vec<String>, UserError> {
        let friend_names = if let Some(names) = self.get_cached_friends(username) {
            names
        } else {
            let names = self.lookup_friends(username).await?;
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.put(username.to_owned(), names.clone());
            names
        };
        Ok(friend_names)
    }

    pub async fn unfriend(&self, owner: &str, friend: &str) -> Result<FriendLink, UserError> {
        let query = doc! {
            "$or": [
                {"sender": &owner, "recipient": &friend, "state": FriendLinkState::Approved},
                {"sender": &friend, "recipient": &owner, "state": FriendLinkState::Approved}
            ]
        };
        let link = self
            .friends
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::FriendNotFoundError)?;

        // invalidate friend cache
        let mut cache = FRIEND_CACHE.write().unwrap();
        cache.pop(owner);
        cache.pop(friend);
        Ok(link)
    }

    pub async fn block_user(&self, owner: &str, other_user: &str) -> Result<FriendLink, UserError> {
        let query = doc! {
            "$or": [
                {"sender": &owner, "recipient": &other_user},
                {"sender": &other_user, "recipient": &owner}
            ]
        };
        let link = FriendLink::new(
            owner.to_owned(),
            other_user.to_owned(),
            Some(FriendLinkState::Blocked),
        );
        let update = doc! {
            "$set": {
                "state": &link.state,
                "updatedAt": &link.updated_at,
            },
            "$setOnInsert": {
                "createdAt": &link.created_at,
            },
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .upsert(true)
            .build();

        let original = self
            .friends
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // invalidate friend cache
        if let Some(mut original) = original {
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.pop(owner);
            cache.pop(other_user);

            original.state = link.state;
            original.updated_at = link.updated_at;

            Ok(original)
        } else {
            Ok(link)
        }
    }

    pub async fn unblock_user(&self, owner: &str, other_user: &str) -> Result<(), UserError> {
        let query = doc! {
            "sender": &owner,
            "recipient": &other_user,
            "state": FriendLinkState::Blocked,
        };
        self.friends
            .delete_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // No need to invalidate cache since it only caches the list of friend names
        Ok(())
    }

    pub async fn list_invites(&self, owner: &str) -> Result<Vec<FriendInvite>, UserError> {
        let query = doc! {"recipient": &owner, "state": FriendLinkState::Pending}; // TODO: ensure they are still pending
        let cursor = self
            .friends
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let invites: Vec<FriendInvite> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|link| link.into())
            .collect();

        Ok(invites)
    }

    pub async fn send_invite(
        &self,
        owner: &str,
        recipient: &str,
    ) -> Result<FriendLinkState, UserError> {
        let query = doc! {
            "sender": &recipient,
            "recipient": &owner,
            "state": FriendLinkState::Pending
        };

        let update = doc! {"$set": {"state": FriendLinkState::Approved}};
        let approved_existing = self
            .friends
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .modified_count
            > 0;

        let state = if approved_existing {
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.pop(owner);
            cache.pop(recipient);

            // TODO: send msg about removing the existing invite

            FriendLinkState::Approved
        } else {
            let query = doc! {
                "$or": [
                    {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::Blocked},
                    {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::Blocked},
                    {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::Approved},
                    {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::Approved},
                ]
            };

            let link = FriendLink::new(owner.to_owned(), recipient.to_owned(), None);
            let update = doc! {"$setOnInsert": &link};
            let options = FindOneAndUpdateOptions::builder().upsert(true).build();
            let result = self
                .friends
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            if let Some(link) = result {
                // user is already blocked or approved
                link.state
            } else {
                // new friend link
                let request: FriendInvite = link.into();
                self.network
                    .send(network::topology::FriendRequestChangeMsg::new(
                        network::topology::ChangeType::Add,
                        request.clone(),
                    ))
                    .await
                    .map_err(InternalError::ActixMessageError)?;

                FriendLinkState::Pending
            }
        };

        Ok(state)
    }

    pub async fn respond_to_request(
        &self,
        recipient: &str,
        sender: &str,
        resp: FriendLinkState,
    ) -> Result<FriendLink, UserError> {
        let query = doc! {
          "recipient": &recipient,
          "sender": &sender,
          "state": FriendLinkState::Pending
        };
        let update = doc! {"$set": {"state": &resp}};

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let link = self
            .friends
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::InviteNotFoundError)?;

        let friend_list_changed = matches!(resp, FriendLinkState::Approved);
        if friend_list_changed {
            // invalidate cache
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.pop(sender);
            cache.pop(recipient);
        }

        let request: FriendInvite = link.clone().into();
        self.network
            .send(network::topology::FriendRequestChangeMsg::new(
                network::topology::ChangeType::Remove,
                request.clone(),
            ))
            .await
            .map_err(InternalError::ActixMessageError)?;

        Ok(link)
    }

    // Tor-related restrictions
    pub async fn ensure_not_tor_ip(&self, ip_addr: &IpAddr) -> Result<(), UserError> {
        let ip_addr = ip_addr.to_string();
        let query = doc! {"addr": &ip_addr};
        let node = self
            .tor_exit_nodes
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if node.is_some() {
            Err(UserError::TorAddressError)
        } else if is_opera_vpn(&ip_addr) {
            Err(UserError::OperaVPNError)
        } else {
            Ok(())
        }
    }

    pub async fn send_email(
        &self,
        to_email: &str,
        subject: &str,
        body: MultiPart,
    ) -> Result<(), UserError> {
        let email = Message::builder()
            .from(self.sender.clone())
            .to(Mailbox::new(
                None,
                to_email
                    .parse::<Address>()
                    .map_err(|_err| UserError::InvalidEmailAddress)?,
            ))
            .subject(subject.to_string())
            .date_now()
            .multipart(body)
            .map_err(|_err| InternalError::EmailBuildError)?;

        self.mailer
            .send(&email)
            .map_err(InternalError::SendEmailError)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_friends(&self, friends: &[FriendLink]) -> Result<(), InternalError> {
        self.friends
            .insert_many(friends, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // clear the friend cache
        let mut cache = FRIEND_CACHE.write().unwrap();
        cache.clear();

        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn drop_all_data(&self) -> Result<(), InternalError> {
        let bucket = &self.settings.s3.bucket;
        let request = rusoto_s3::DeleteBucketRequest {
            bucket: bucket.clone(),
            ..Default::default()
        };

        if self.s3.delete_bucket(request).await.is_err() {
            info!("Bucket does not exist");
        }

        Ok(())
    }
}

async fn update_tor_nodes(tor_exit_nodes: &Collection<TorNode>) -> Result<(), UserError> {
    let url = "https://check.torproject.org/torbulkexitlist";
    let response = reqwest::get(url)
        .await
        .map_err(InternalError::TorNodeListFetchError)?;

    let node_list: Vec<TorNode> = response
        .text()
        .await
        .map_err(|_err| UserError::InternalError)?
        .split_ascii_whitespace()
        .map(|addr| TorNode {
            addr: addr.to_string(),
        })
        .collect();

    tor_exit_nodes
        .delete_many(doc! {}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    tor_exit_nodes
        .insert_many(node_list, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct TorNode {
    addr: String,
}

fn get_unique_name(existing: Vec<String>, name: &str) -> String {
    let names: HashSet<std::string::String> = HashSet::from_iter(existing.iter().cloned());
    let base_name = name;
    let mut role_name = base_name.to_owned();
    let mut number: u8 = 2;
    while names.contains(&role_name) {
        role_name = format!("{} ({})", base_name, number);
        number += 1;
    }
    role_name
}

fn is_opera_vpn(addr: &str) -> bool {
    let opera_prefixes = ["77.111.244.", "77.111.245.", "77.111.246.", "77.111.247."];
    opera_prefixes
        .into_iter()
        .any(|prefix| addr.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use netsblox_cloud_common::api;

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    #[ignore]
    async fn test_save_role_blob() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_save_role_set_transient_false() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_request() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(sender.username.clone(), rcvr.username.clone(), None);

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let link = app_data
                    .respond_to_request(&rcvr.username, &sender.username, FriendLinkState::Approved)
                    .await
                    .unwrap();

                assert!(matches!(link.state, FriendLinkState::Approved));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_404() {
        test_utils::setup()
            .run(|app_data| async move {
                let result = app_data
                    .respond_to_request("rcvr", "sender", FriendLinkState::Approved)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_rejected() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(FriendLinkState::Rejected),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let result = app_data
                    .respond_to_request("rcvr", "sender", FriendLinkState::Approved)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_approved() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(FriendLinkState::Approved),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let result = app_data
                    .respond_to_request("rcvr", "sender", FriendLinkState::Approved)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_blocked() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(FriendLinkState::Blocked),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let result = app_data
                    .respond_to_request("rcvr", "sender", FriendLinkState::Approved)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }
}
