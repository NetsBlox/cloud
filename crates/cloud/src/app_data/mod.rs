use crate::common::api::{
    oauth, LibraryMetadata, NewUser, ProjectId, PublishState, RoleId, UserRole,
};
use actix_web::rt::time;
use actix_web::HttpRequest;
use futures::future::join_all;
use futures::join;
use lazy_static::lazy_static;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use log::{info, warn};
use lru::LruCache;
use mongodb::bson::{doc, DateTime, Document};
<<<<<<< Updated upstream
use mongodb::options::{FindOneAndUpdateOptions, IndexOptions, ReturnDocument};
use netsblox_cloud_common::api::{FriendInvite, FriendLinkState};
=======
use mongodb::options::{FindOneAndUpdateOptions, FindOptions, IndexOptions, ReturnDocument};
>>>>>>> Stashed changes
use rusoto_core::credential::StaticProvider;
use rusoto_core::Region;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
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
use crate::users::get_user_role;
use actix::{Actor, Addr};
use futures::TryStreamExt;
use mongodb::{Client, Collection, IndexModel};
use rusoto_s3::{
    CreateBucketRequest, DeleteObjectRequest, GetObjectRequest, PutObjectOutput, PutObjectRequest,
    S3Client, S3,
};

// This is lazy_static to ensure it is shared between threads
// TODO: it would be nice to be able to configure the cache size from the settings
lazy_static! {
    static ref MEMBERSHIP_CACHE: Arc<RwLock<LruCache<String, bool>>> =
        Arc::new(RwLock::new(LruCache::new(1000)));
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

    mailer: SmtpTransport,
    sender: Mailbox,
}

impl AppData {
    pub fn new(
        client: Client,
        settings: Settings,
        network: Option<Addr<TopologyActor>>,
        prefix: Option<&'static str>,
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
            //StaticProvider::from(AwsCredentials::default()),
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
        let network = network.unwrap_or_else(|| TopologyActor {}.start());
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
                                .partial_filter_expression(doc! {"saveState": SaveState::TRANSIENT})
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
        ids: &'a [ProjectId],
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

    pub async fn get_project_metadata(
        &self,
        ids: &[ProjectId],
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
        let query = doc! {"owner": &owner};
        // TODO: validate the project name (no profanity)
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
        roles: Option<Vec<RoleData>>,
        save_state: Option<SaveState>,
    ) -> Result<ProjectMetadata, UserError> {
        let unique_name = self.get_valid_project_name(owner, name).await?;
        let roles = roles.unwrap_or_else(|| {
            vec![RoleData {
                name: "myRole".to_owned(),
                code: "".to_owned(),
                media: "".to_owned(),
            }]
        });

        let role_mds: Vec<RoleMetadata> = join_all(
            roles
                .iter()
                .map(|role| self.upload_role(owner, &unique_name, role)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<RoleMetadata>, _>>()?;

        let save_state = save_state.unwrap_or(SaveState::CREATED);
        let metadata = ProjectMetadata::new(owner, &unique_name, role_mds, save_state);
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
        self.s3
            .put_object(request)
            .await
            .map_err(|_err| InternalError::S3Error)
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

    pub async fn delete_project(&self, metadata: ProjectMetadata) -> Result<(), UserError> {
        let query = doc! {"id": &metadata.id};
        let result = self
            .project_metadata
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if let Some(metadata) = result {
            let paths = metadata
                .roles
                .into_values()
                .flat_map(|role| vec![role.code, role.media]);

            join_all(paths.map(move |path| self.delete(path)))
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            // TODO: send update to any current occupants
            Ok(())
        } else {
            Err(UserError::ProjectNotFoundError)
        }
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
                "saveState": SaveState::SAVED,
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
    pub async fn keep_members(
        &self,
        usernames: impl Iterator<Item = Option<&String>>,
    ) -> Result<impl Iterator<Item = &String>, UserError> {
        let mut cache = MEMBERSHIP_CACHE.write().unwrap();
        let unknown_users: Vec<_> = usernames
            .filter(|name_opt| name_opt.map(|name| !cache.contains(name)).unwrap_or(false))
            .collect();
        drop(cache); // don't hold the lock over the upcoming async boundary

        let unknown_count = unknown_users.len() as i64;
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

        let mut cache = MEMBERSHIP_CACHE.write().unwrap();
        users.into_iter().for_each(|usr| {
            cache.put(usr.username, usr.group_id.is_some());
        });
        // TODO: check the cache?
        // TODO: check membership
        // TODO: I think the logic ahead is a little problematic and needs to be checked out...
        usernames.filter(|name_opt| {
            name_opt
                .map(|name| {
                    // Althought unlikely, it's possible that the entries
                    // have been invalidated from the cache while looking up
                    // the unknown users. In this case, we will be conservative
                    // and just assume they are members.
                    let is_member = cache.get(name).unwrap_or(&true);
                    *is_member
                })
                .unwrap_or(true)
        })
    }

    // Friend-related features
    fn get_cached_friends(&self, username: &str) -> Option<Vec<String>> {
        let mut cache = FRIEND_CACHE.write().unwrap();
        cache.get(username).map(|friends| friends.to_owned())
    }

    async fn lookup_friends(&self, username: &str) -> Result<Vec<String>, UserError> {
        let is_universal_friend = matches!(get_user_role(self, username).await?, UserRole::Admin);
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
        } else {
            let query = doc! {"$or": [
                {"sender": &username, "state": FriendLinkState::APPROVED},
                {"recipient": &username, "state": FriendLinkState::APPROVED}
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
                .collect()
        };

        Ok(friend_names)
    }

    pub async fn get_friends(&self, username: &str) -> Result<Vec<String>, UserError> {
        let friend_names = if let Some(names) = self.get_cached_friends(username) {
            names
        } else {
            let names = self.lookup_friends(username).await?;
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.put(username.to_owned(), names.clone());
            names
            // TODO: support friends for group members
        };
        Ok(friend_names)
    }

    pub async fn unfriend(&self, owner: &str, friend: &str) -> Result<(), UserError> {
        let query = doc! {
            "$or": [
                {"sender": &owner, "recipient": &friend, "state": FriendLinkState::APPROVED},
                {"sender": &friend, "recipient": &owner, "state": FriendLinkState::APPROVED}
            ]
        };
        let result = self
            .friends
            .delete_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.deleted_count > 0 {
            // invalidate friend cache
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.pop(owner);
            cache.pop(friend);
            Ok(())
        } else {
            Err(UserError::FriendNotFoundError)
        }
    }

    pub async fn block_user(&self, owner: &str, other_user: &str) -> Result<(), UserError> {
        let query = doc! {
            "$or": [
                {"sender": &owner, "recipient": &other_user},
                {"sender": &other_user, "recipient": &owner}
            ]
        };
        let link = FriendLink::new(
            owner.to_owned(),
            other_user.to_owned(),
            Some(FriendLinkState::BLOCKED),
        );
        let update = doc! {
            "$set": &link,
            "$setOnInsert": &link
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
        let invalidate_cache = original.is_some();
        if invalidate_cache {
            let mut cache = FRIEND_CACHE.write().unwrap();
            cache.pop(owner);
            cache.pop(other_user);
        }
        Ok(())
    }

    pub async fn unblock_user(&self, owner: &str, other_user: &str) -> Result<(), UserError> {
        let query = doc! {
            "sender": &owner,
            "recipient": &other_user,
            "state": FriendLinkState::BLOCKED,
        };
        self.friends
            .delete_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // No need to invalidate cache since it only caches the list of friend names
        Ok(())
    }

    pub async fn list_invites(&self, owner: &str) -> Result<Vec<FriendInvite>, UserError> {
        let query = doc! {"recipient": &owner, "state": FriendLinkState::PENDING}; // TODO: ensure they are still pending
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
            "state": FriendLinkState::PENDING
        };

        let update = doc! {"$set": {"state": FriendLinkState::APPROVED}};
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
            FriendLinkState::APPROVED
        } else {
            let query = doc! {
                "$or": [
                    {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::BLOCKED},
                    {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::BLOCKED},
                    {"sender": &owner, "recipient": &recipient, "state": FriendLinkState::APPROVED},
                    {"sender": &recipient, "recipient": &owner, "state": FriendLinkState::APPROVED},
                ]
            };

            let link = FriendLink::new(owner.to_owned(), recipient.to_owned(), None);
            let update = doc! {"$setOnInsert": link};
            let options = FindOneAndUpdateOptions::builder().upsert(true).build();
            let result = self
                .friends
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            // TODO: Should we allow users to send multiple invitations (ie, after rejection)?
            result
                .map(|link| link.state)
                .unwrap_or(FriendLinkState::PENDING)
        };

        Ok(state)
    }

    pub async fn response_to_invite(
        &self,
        recipient: &str,
        sender: &str,
        resp: FriendLinkState,
    ) -> Result<(), UserError> {
        let query = doc! {"recipient": &recipient, "sender": &sender};
        let update = doc! {"$set": {"state": &resp}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .build();
        let original = self
            .friends
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if let Some(original) = original {
            let invalidate_cache = matches!(resp, FriendLinkState::APPROVED)
                || matches!(original.state, FriendLinkState::APPROVED);
            if invalidate_cache {
                let mut cache = FRIEND_CACHE.write().unwrap();
                cache.pop(sender);
                cache.pop(recipient);
            }
            Ok(())
        } else {
            Err(UserError::InviteNotFoundError)
        }
    }

    // Tor-related restrictions
    pub async fn ensure_not_tor_ip(&self, req: HttpRequest) -> Result<(), UserError> {
        match req.peer_addr().map(|addr| addr.ip()) {
            Some(addr) => {
                let addr = addr.to_string();
                let query = doc! {"addr": &addr};
                let node = self
                    .tor_exit_nodes
                    .find_one(query, None)
                    .await
                    .map_err(InternalError::DatabaseConnectionError)?;

                if node.is_some() {
                    Err(UserError::TorAddressError)
                } else if is_opera_vpn(&addr) {
                    Err(UserError::OperaVPNError)
                } else {
                    Ok(())
                }
            }
            None => Ok(()),
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
    #[actix_web::test]
    async fn test_save_role_blob() {
        todo!();
    }

    #[actix_web::test]
    async fn test_save_role_set_transient_false() {
        todo!();
    }
}
