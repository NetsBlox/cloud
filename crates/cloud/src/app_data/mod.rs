pub(crate) mod metrics;

use crate::collaboration_invites::actions::CollaborationInviteActions;
use crate::common::api::{oauth, NewUser, ProjectId, UserRole};
use crate::friends::actions::FriendActions;
use crate::galleries::actions::GalleryActions;
use crate::groups::actions::GroupActions;
use crate::libraries::actions::LibraryActions;
use crate::login_helper::LoginHelper;
use crate::magic_links::actions::MagicLinkActions;
use crate::network::actions::NetworkActions;
use crate::oauth::actions::OAuthActions;
use crate::projects::ProjectActions;
use crate::services::hosts::actions::HostActions;
use crate::services::settings::actions::SettingsActions;
use crate::users::actions::{UserActionData, UserActions};
use actix::dev::OneshotSender;
use actix_web::rt::time;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::SmtpTransport;
use log::{error, info, warn};
use lru::LruCache;
use mongodb::bson::{doc, Document};
use mongodb::options::{FindOptions, IndexOptions, UpdateOptions};
use netsblox_cloud_common::{api, Bucket, Gallery, GalleryProjectMetadata, MagicLink};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::RwLock as AsyncRwLock;

use crate::common::api::SaveState;
use crate::common::{
    AuthorizedServiceHost, BannedAccount, CollaborationInvite, FriendLink, Group, Library,
    OAuthClient, OAuthToken, ProjectMetadata, SetPasswordToken, User,
};
use crate::common::{LogMessage, OccupantInvite, SentMessage};
use crate::config::Settings;
use crate::errors::{InternalError, UserError};
use crate::network::topology::{SetStorage, TopologyActor, TopologyPanic};
use actix::{Actor, Addr};
use aws_config::SdkConfig;
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials as S3Credentials};
use aws_sdk_s3::{self as s3, config::Region};
use futures::TryStreamExt;
use mongodb::{Client, Collection, IndexModel};

#[derive(Clone)]
pub struct AppData {
    bucket: Bucket,
    tor_exit_nodes: Collection<TorNode>,
    s3: s3::Client,
    pub(crate) settings: Settings,
    pub(crate) network: Addr<TopologyActor>,
    pub(crate) groups: Collection<Group>,
    pub(crate) users: Collection<User>,
    pub(crate) banned_accounts: Collection<BannedAccount>,
    friends: Collection<FriendLink>,
    magic_links: Collection<MagicLink>,
    pub(crate) galleries: Collection<Gallery>,
    pub(crate) gallery_projects: Collection<GalleryProjectMetadata>,
    pub(crate) project_metadata: Collection<ProjectMetadata>,
    pub(crate) libraries: Collection<Library>,
    pub(crate) authorized_services: Collection<AuthorizedServiceHost>,

    pub(crate) password_tokens: Collection<SetPasswordToken>,
    pub(crate) recorded_messages: Collection<SentMessage>,
    pub(crate) logged_messages: Collection<LogMessage>,
    pub(crate) collab_invites: Collection<CollaborationInvite>,
    pub(crate) occupant_invites: Collection<OccupantInvite>,

    pub(crate) oauth_clients: Collection<OAuthClient>,
    pub(crate) oauth_tokens: Collection<OAuthToken>,
    pub(crate) oauth_codes: Collection<oauth::Code>,

    pub(crate) metrics: metrics::Metrics,
    mailer: SmtpTransport,
    sender: Mailbox,

    // cached data
    project_cache: Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    membership_cache: Arc<AsyncRwLock<LruCache<String, bool>>>,
    admin_cache: Arc<AsyncRwLock<LruCache<String, bool>>>,
    friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,
}

#[allow(clippy::too_many_lines)]
impl AppData {
    pub fn new(
        client: Client,
        settings: Settings,
        network: Option<Addr<TopologyActor>>,
        prefix: Option<&str>,
        tx: Option<OneshotSender<TopologyPanic>>,
    ) -> AppData {
        // Blob storage
        let access_key = settings.s3.credentials.access_key.clone();
        let secret_key = settings.s3.credentials.secret_key.clone();
        let region = Region::new(settings.s3.region_name.clone());

        let config = SdkConfig::builder()
            .region(region)
            .endpoint_url(settings.s3.endpoint.clone())
            .credentials_provider(SharedCredentialsProvider::new(S3Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "NetsBloxConfig",
            )))
            .build();

        let s3 = s3::Client::new(&config);

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
        let libraries = db.collection::<Library>(&(prefix.to_owned() + "libraries"));
        let authorized_services =
            db.collection::<AuthorizedServiceHost>(&(prefix.to_owned() + "authorizedServices"));
        let collab_invites =
            db.collection::<CollaborationInvite>(&(prefix.to_owned() + "collaborationInvitations"));
        let occupant_invites =
            db.collection::<OccupantInvite>(&(prefix.to_owned() + "occupantInvites"));
        let friends = db.collection::<FriendLink>(&(prefix.to_owned() + "friends"));
        let magic_links = db.collection::<MagicLink>(&(prefix.to_owned() + "magicLinks"));
        let galleries = db.collection::<Gallery>(&(prefix.to_owned() + "galleries"));
        let gallery_projects =
            db.collection::<GalleryProjectMetadata>(&(prefix.to_owned() + "galleryProjects"));
        let recorded_messages =
            db.collection::<SentMessage>(&(prefix.to_owned() + "recordedMessages"));
        let logged_messages = db.collection::<LogMessage>(&(prefix.to_owned() + "loggedMessages"));
        let network = network.unwrap_or_else(|| {
            TopologyActor::new(settings.cache_settings.num_addresses, tx).start()
        });
        let oauth_clients = db.collection::<OAuthClient>(&(prefix.to_owned() + "oauthClients"));
        let oauth_tokens = db.collection::<OAuthToken>(&(prefix.to_owned() + "oauthToken"));
        let oauth_codes = db.collection::<oauth::Code>(&(prefix.to_owned() + "oauthCode"));
        let tor_exit_nodes = db.collection::<TorNode>(&(prefix.to_owned() + "torExitNodes"));
        let bucket = Bucket::new(settings.s3.bucket.clone());

        let project_cache = Arc::new(RwLock::new(LruCache::new(
            settings.cache_settings.num_projects,
        )));
        let membership_cache = Arc::new(AsyncRwLock::new(LruCache::new(
            settings.cache_settings.num_users_membership_data,
        )));
        let admin_cache = Arc::new(AsyncRwLock::new(LruCache::new(
            settings.cache_settings.num_users_admin_data,
        )));
        let friend_cache = Arc::new(RwLock::new(LruCache::new(
            settings.cache_settings.num_users_friend_data,
        )));

        AppData {
            settings,
            network,
            s3,
            bucket,
            groups,
            users,
            banned_accounts,
            project_metadata,
            libraries,
            authorized_services,

            collab_invites,
            occupant_invites,
            password_tokens,
            friends,
            magic_links,
            galleries,
            gallery_projects,

            mailer,
            sender,

            oauth_clients,
            oauth_tokens,
            oauth_codes,

            metrics: metrics::Metrics::new(),

            tor_exit_nodes,
            recorded_messages,
            logged_messages,
            project_cache,
            membership_cache,
            admin_cache,
            friend_cache,
        }
    }

    pub async fn initialize(&self) -> Result<(), InternalError> {
        // Create the s3 bucket
        let bucket = &self.settings.s3.bucket;

        // FIXME: check if bucket exists or invalid bucket name

        let create_result = self.s3.create_bucket().bucket(bucket.clone()).send().await;
        if create_result.is_err() {
            info!("Using existing s3 bucket.");
        }

        // Add database indexes
        let one_hour = Duration::from_secs(60 * 60);

        let index_opts = IndexOptions::builder().expire_after(one_hour).build();
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

        let index_opts = IndexOptions::builder().expire_after(one_hour).build();
        let token_index = IndexModel::builder()
            .keys(doc! {"createdAt": 1})
            .options(index_opts)
            .build();
        self.password_tokens
            .create_index(token_index, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let one_week = Duration::from_secs(60 * 60 * 24 * 7);
        self.project_metadata
            .create_indexes(
                vec![
                    // optimize lookups by ID
                    IndexModel::builder().keys(doc! {"id": 1}).build(),
                    // optimize lookups by owner
                    IndexModel::builder().keys(doc! {"owner": 1}).build(),
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
                                .expire_after(one_week)
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

        // Initialize Message Logs
        self.initialize_message_log().await?;

        self.tor_exit_nodes
            .create_index(IndexModel::builder().keys(doc! {"addr": 1}).build(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let index_opts = IndexOptions::builder().expire_after(one_hour).build();
        let magic_link_indexes = vec![IndexModel::builder()
            .keys(doc! {"createdAt": 1})
            .options(index_opts)
            .build()];
        self.magic_links
            .create_indexes(magic_link_indexes, None)
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

        if let Some(host_config) = self.settings.authorized_host.as_ref() {
            let host: AuthorizedServiceHost = host_config.clone().into();
            let query = doc! {"id": &host.id};
            let update = doc! {"$setOnInsert": &host};
            let options = UpdateOptions::builder().upsert(true).build();
            self.authorized_services
                .update_one(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
        }

        Ok(())
    }

    async fn initialize_message_log(&self) -> Result<(), InternalError> {
        let three_months = Duration::from_secs(60 * 60 * 24 * 30 * 3);
        let index_opts = IndexOptions::builder().expire_after(three_months).build();

        let token_index = IndexModel::builder()
            .keys(doc! {"createdAt": 1})
            .options(index_opts)
            .build();
        self.logged_messages
            .create_index(token_index, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

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

            let mut cache = self.project_cache.write().unwrap();
            projects.iter().for_each(|project| {
                cache.put(project.id.clone(), project.clone());
            });
            results.extend(projects);
        }

        Ok(results)
    }

    fn get_cached_project_metadata<'a>(
        &self,
        ids: impl Iterator<Item = &'a api::ProjectId>,
    ) -> (Vec<ProjectMetadata>, Vec<&'a api::ProjectId>) {
        let mut results = Vec::new();
        let mut missing_projects = Vec::new();
        let mut cache = self.project_cache.write().unwrap();
        for id in ids {
            match cache.get(id) {
                Some(project_metadata) => results.push(project_metadata.clone()),
                None => missing_projects.push(id),
            }
        }
        (results, missing_projects)
    }

    fn get_cached_project(&self, id: &ProjectId) -> Option<ProjectMetadata> {
        let mut cache = self.project_cache.write().unwrap();
        cache.get(id).map(|md| md.to_owned())
    }

    async fn get_project_and_cache(&self, id: &ProjectId) -> Result<ProjectMetadata, UserError> {
        let metadata = self
            .project_metadata
            .find_one(doc! {"id": id}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ProjectNotFoundError)?;

        let mut cache = self.project_cache.write().unwrap();
        cache.put(id.clone(), metadata);
        Ok(cache.get(id).unwrap().clone())
    }

    // Membership queries (cached)
    pub async fn keep_members(&self, usernames: HashSet<String>) -> Result<Vec<String>, UserError> {
        let cache = self.membership_cache.write().await;
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

            let mut cache = self.membership_cache.write().await;
            users.into_iter().for_each(|usr| {
                cache.put(usr.username, usr.group_id.is_some());
            });
        }

        let mut cache = self.membership_cache.write().await;
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
        let cache = self.admin_cache.write().await;
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

            let mut cache = self.admin_cache.write().await;
            cache.put(username.to_owned(), is_admin);
        }

        let mut cache = self.admin_cache.write().await;
        cache
            .get(username)
            .map(|is_admin| is_admin.to_owned())
            .unwrap_or(false)
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

    #[cfg(test)]
    pub(crate) async fn insert_friends(&self, friends: &[FriendLink]) -> Result<(), InternalError> {
        self.friends
            .insert_many(friends, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // clear the friend cache
        let mut cache = self.friend_cache.write().unwrap();
        cache.clear();

        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_galleries(
        &self,
        galleries: &[Gallery],
    ) -> Result<(), InternalError> {
        self.galleries
            .insert_many(galleries, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_gallery_projects(
        &self,
        gallery_projects: &[GalleryProjectMetadata],
    ) -> Result<(), InternalError> {
        for project in gallery_projects {
            for (index, version) in project.versions.iter().enumerate() {
                let color = crate::test_utils::gallery_projects::TestThumbnail::new(index);

                crate::utils::upload(
                    &self.s3,
                    &self.bucket,
                    &version.key,
                    format!(
                        "<project><version>{}</version><thumbnail>{}</thumbnail></project>",
                        index,
                        color.as_str(),
                    ),
                )
                .await?;
            }
        }

        self.gallery_projects
            .insert_many(gallery_projects, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_magic_links(
        &self,
        links: &[MagicLink],
    ) -> Result<(), InternalError> {
        self.magic_links
            .insert_many(links, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(())
    }

    pub(crate) async fn get_friends(&self, username: &str) -> Result<Vec<String>, UserError> {
        crate::utils::get_friends(
            &self.users,
            &self.groups,
            &self.friends,
            self.friend_cache.clone(),
            username,
        )
        .await
    }

    // get resource actions (eg, libraries, users, etc)
    pub(crate) fn as_library_actions(&self) -> LibraryActions {
        LibraryActions::new(&self.libraries)
    }

    pub(crate) fn as_gallery_actions(&self) -> GalleryActions {
        GalleryActions::new(
            &self.galleries,
            &self.gallery_projects,
            &self.bucket,
            &self.s3,
        )
    }

    pub(crate) fn as_project_actions(&self) -> ProjectActions {
        ProjectActions::new(
            &self.project_metadata,
            &self.project_cache,
            &self.network,
            &self.bucket,
            &self.s3,
        )
    }

    pub(crate) fn as_group_actions(&self) -> GroupActions {
        GroupActions::new(&self.groups, &self.users)
    }

    pub(crate) fn as_friend_actions(&self) -> FriendActions {
        FriendActions::new(
            &self.friends,
            &self.friend_cache,
            &self.users,
            &self.groups,
            &self.network,
        )
    }

    pub(crate) fn as_magic_link_actions(&self) -> MagicLinkActions {
        MagicLinkActions::new(
            &self.magic_links,
            &self.users,
            &self.mailer,
            &self.sender,
            &self.settings.public_url,
        )
    }

    pub(crate) fn as_collab_invite_actions(&self) -> CollaborationInviteActions {
        CollaborationInviteActions::new(
            &self.collab_invites,
            &self.project_metadata,
            &self.project_cache,
            &self.network,
        )
    }

    pub(crate) fn as_network_actions(&self) -> NetworkActions {
        NetworkActions::new(
            &self.project_metadata,
            &self.project_cache,
            &self.network,
            &self.occupant_invites,
            &self.recorded_messages,
            &self.logged_messages,
        )
    }

    pub(crate) fn as_settings_actions(&self) -> SettingsActions {
        SettingsActions::new(&self.users, &self.groups)
    }

    pub(crate) fn as_oauth_actions(&self) -> OAuthActions {
        OAuthActions::new(&self.oauth_clients, &self.oauth_tokens, &self.oauth_codes)
    }

    pub(crate) fn as_user_actions(&self) -> UserActions {
        let data = UserActionData {
            users: &self.users,
            banned_accounts: &self.banned_accounts,
            password_tokens: &self.password_tokens,
            metrics: &self.metrics,

            network: &self.network,
            friend_cache: &self.friend_cache,

            mailer: &self.mailer,
            sender: &self.sender,
            public_url: &self.settings.public_url,
        };
        UserActions::new(data)
    }

    pub(crate) fn as_host_actions(&self) -> HostActions {
        HostActions::new(&self.authorized_services)
    }

    pub(crate) fn as_login_helper(&self) -> LoginHelper {
        LoginHelper::new(
            &self.network,
            &self.metrics,
            &self.project_metadata,
            &self.project_cache,
            &self.banned_accounts,
        )
    }

    #[cfg(test)]
    pub(crate) fn drop_s3(&mut self) {
        let config = SdkConfig::builder().build();

        self.s3 = s3::Client::new(&config);
    }

    #[cfg(test)]
    pub(crate) async fn drop_all_data(&self) -> Result<(), InternalError> {
        let bucket = &self.settings.s3.bucket;

        if self
            .s3
            .delete_bucket()
            .bucket(bucket.clone())
            .send()
            .await
            .is_err()
        {
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

fn is_opera_vpn(addr: &str) -> bool {
    let opera_prefixes = ["77.111.244.", "77.111.245.", "77.111.246.", "77.111.247."];
    opera_prefixes
        .into_iter()
        .any(|prefix| addr.starts_with(prefix))
}

#[cfg(test)]
mod tests {

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
}
