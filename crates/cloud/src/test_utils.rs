use std::sync::{Arc, Mutex};

use futures::{future::join_all, Future};
use lazy_static::lazy_static;
use mongodb::{bson::doc, Client};
use netsblox_cloud_common::{
    api, AuthorizedServiceHost, BannedAccount, CollaborationInvite, FriendLink, Gallery, Group,
    Library, MagicLink, User,
};

use crate::{
    app_data::AppData,
    auth,
    config::Settings,
    projects::{actions::CreateProjectDataDict, ProjectActions},
};

lazy_static! {
    static ref COUNTER: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
}

pub(crate) fn setup() -> TestSetupBuilder {
    let mut counter = COUNTER.lock().unwrap();
    *counter += 1_u32;
    let prefix = format!("test-{}", counter);
    TestSetupBuilder {
        prefix,
        users: Vec::new(),
        banned_users: Vec::new(),
        projects: Vec::new(),
        libraries: Vec::new(),
        groups: Vec::new(),
        clients: Vec::new(),
        friends: Vec::new(),
        magic_links: Vec::new(),
        collab_invites: Vec::new(),
        authorized_services: Vec::new(),
        galleries: Vec::new(),
        // network: None,
    }
}

pub(crate) struct TestSetupBuilder {
    prefix: String,
    users: Vec<User>,
    projects: Vec<project::ProjectFixture>,
    libraries: Vec<Library>,
    galleries: Vec<Gallery>,
    groups: Vec<Group>,
    clients: Vec<network::Client>,
    friends: Vec<FriendLink>,
    magic_links: Vec<MagicLink>,
    banned_users: Vec<String>,
    collab_invites: Vec<CollaborationInvite>,
    authorized_services: Vec<AuthorizedServiceHost>,
    //network: Option<Addr<TopologyActor>>,
}

impl TestSetupBuilder {
    pub(crate) fn with_users(mut self, users: &[User]) -> Self {
        self.users.extend_from_slice(users);
        self
    }

    pub(crate) fn with_banned_users(mut self, banned_users: &[String]) -> Self {
        self.banned_users.extend_from_slice(banned_users);
        self
    }

    pub(crate) fn with_groups(mut self, groups: &[Group]) -> Self {
        self.groups.extend_from_slice(groups);
        self
    }

    pub(crate) fn with_projects(mut self, projects: &[project::ProjectFixture]) -> Self {
        self.projects.extend_from_slice(projects);
        self
    }

    pub(crate) fn with_galleries(mut self, galleries: &[Gallery]) -> Self {
        self.galleries.extend_from_slice(galleries);
        self
    }

    pub(crate) fn with_libraries(mut self, libraries: &[Library]) -> Self {
        self.libraries.extend_from_slice(libraries);
        self
    }

    pub(crate) fn with_collab_invites(mut self, invites: &[CollaborationInvite]) -> Self {
        self.collab_invites.extend_from_slice(invites);
        self
    }

    pub(crate) fn with_authorized_services(mut self, hosts: &[AuthorizedServiceHost]) -> Self {
        self.authorized_services.extend_from_slice(hosts);
        self
    }

    pub(crate) fn with_clients(mut self, clients: &[network::Client]) -> Self {
        self.clients.extend_from_slice(clients);
        self
    }

    pub(crate) fn with_friend_links(mut self, friends: &[FriendLink]) -> Self {
        self.friends.extend_from_slice(friends);
        self
    }

    pub(crate) fn with_magic_links(mut self, links: &[MagicLink]) -> Self {
        self.magic_links.extend_from_slice(links);
        self
    }

    // pub(crate) fn with_network(mut self, network: Addr<TopologyActor>) -> Self {
    //     self.network = Some(network);
    //     self
    // }

    pub(crate) async fn run<Fut>(self, f: impl FnOnce(AppData) -> Fut)
    where
        Fut: Future<Output = ()>,
    {
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        let mut settings = Settings::new().unwrap();
        let db_name = format!("{}-{}", &self.prefix, settings.database.name);
        settings.database.name = db_name.clone();
        settings.s3.bucket = format!("{}-{}", &self.prefix, settings.s3.bucket);

        let app_data = AppData::new(client.clone(), settings, None, None, None);

        // create the test fixtures (users, projects, etc)
        client.database(&db_name).drop(None).await.unwrap();
        app_data
            .initialize()
            .await
            .expect("Unable to initialize AppData");

        join_all(self.projects.into_iter().map(|fixture| async {
            let project::ProjectFixture {
                id,
                owner,
                name,
                roles,
                traces,
                state,
                ..
            } = fixture;

            let auth_eu = auth::EditUser::test(owner.clone());
            let actions: ProjectActions = app_data.as_project_actions();
            let project_data = CreateProjectDataDict {
                name,
                roles,
                save_state: Some(api::SaveState::Saved),
                state,
            };
            let metadata = actions
                .create_project(&auth_eu, project_data)
                .await
                .unwrap();

            let query = doc! {"id": &metadata.id};
            let update = if traces.is_empty() {
                doc! {"$set": {"id": &id}}
            } else {
                doc! {
                    "$set": {
                        "id": &id,
                        "networkTraces": &traces
                    },
                }
            };
            app_data
                .project_metadata
                .update_one(query, update, None)
                .await
                .unwrap();
        }))
        .await;

        if !self.banned_users.is_empty() {
            let banned_users = self.banned_users.into_iter().map(|username| {
                let email = self
                    .users
                    .iter()
                    .find(|user| user.username == username)
                    .map(|user| user.email.clone())
                    .unwrap_or_else(|| String::from("none@netsblox.org"));

                BannedAccount::new(username, email)
            });
            app_data
                .banned_accounts
                .insert_many(banned_users, None)
                .await
                .unwrap();
        }
        if !self.libraries.is_empty() {
            app_data
                .libraries
                .insert_many(self.libraries, None)
                .await
                .unwrap();
        }
        if !self.users.is_empty() {
            app_data.users.insert_many(self.users, None).await.unwrap();
        }
        if !self.friends.is_empty() {
            app_data.insert_friends(&self.friends).await.unwrap();
        }
        if !self.galleries.is_empty() {
            app_data.insert_galleries(&self.galleries).await.unwrap();
        }
        if !self.magic_links.is_empty() {
            app_data
                .insert_magic_links(&self.magic_links)
                .await
                .unwrap();
        }
        if !self.groups.is_empty() {
            app_data
                .groups
                .insert_many(self.groups, None)
                .await
                .unwrap();
        }

        if !self.collab_invites.is_empty() {
            app_data
                .collab_invites
                .insert_many(self.collab_invites, None)
                .await
                .unwrap();
        }
        if !self.authorized_services.is_empty() {
            app_data
                .authorized_services
                .insert_many(self.authorized_services, None)
                .await
                .unwrap();
        }

        // Connect the clients
        join_all(
            self.clients
                .into_iter()
                .map(|client| client.add_into(&app_data.network)),
        )
        .await;

        f(app_data.clone()).await;

        // cleanup
        client.database(&db_name).drop(None).await.unwrap();
        app_data.drop_all_data().await.unwrap();
    }
}

pub(crate) mod cookie {

    use actix_session::{storage::CookieSessionStore, SessionMiddleware};
    use actix_web::cookie::{Cookie, CookieJar, Key};
    use serde_json::json;

    static COOKIE_NAME: &str = "test_netsblox";
    pub(crate) fn new(username: &str) -> Cookie {
        let data = json!({
            "username": format!("\"{}\"", username),  // FIXME: this shouldn't need extra quotes...
        });
        let cookie = Cookie::new(COOKIE_NAME, data.to_string());

        // Use the cookie jar to encrypt & sign the cookie
        let mut jar = CookieJar::new();
        let key = Key::from(&[0; 64]);
        jar.private_mut(&key).add(cookie);

        let cookie = jar.get(COOKIE_NAME).unwrap();
        cookie.to_owned()
    }

    pub(crate) fn middleware() -> SessionMiddleware<CookieSessionStore> {
        let secret_key = Key::from(&[0; 64]);
        SessionMiddleware::builder(CookieSessionStore::default(), secret_key)
            .cookie_name(COOKIE_NAME.to_string())
            .build()
    }
}

pub(crate) mod project {
    use std::collections::HashMap;

    use netsblox_cloud_common::{
        api::{self, PublishState, RoleData, RoleId},
        NetworkTraceMetadata,
    };
    use uuid::Uuid;

    pub(crate) struct ProjectBuilder {
        id: Option<api::ProjectId>,
        owner: Option<String>,
        name: Option<String>,
        collaborators: Vec<String>,
        roles: HashMap<api::RoleId, api::RoleData>,
        traces: Vec<NetworkTraceMetadata>,
        state: PublishState,
    }

    impl ProjectBuilder {
        pub(crate) fn with_name(mut self, name: &str) -> Self {
            self.name = Some(name.to_owned());
            self
        }

        pub(crate) fn with_owner(mut self, owner: String) -> Self {
            self.owner = Some(owner);
            self
        }

        pub(crate) fn with_roles(mut self, roles: HashMap<api::RoleId, api::RoleData>) -> Self {
            self.roles = roles;
            self
        }

        pub(crate) fn with_traces(mut self, traces: &[NetworkTraceMetadata]) -> Self {
            self.traces.extend_from_slice(traces);
            self
        }

        pub(crate) fn with_id(mut self, id: api::ProjectId) -> Self {
            self.id = Some(id);
            self
        }

        pub(crate) fn with_collaborators(mut self, names: &[&str]) -> Self {
            self.collaborators = names.iter().map(|n| n.to_string()).collect::<Vec<String>>();
            self
        }

        pub(crate) fn with_state(mut self, state: PublishState) -> Self {
            self.state = state;
            self
        }

        pub(crate) fn build(mut self) -> ProjectFixture {
            let id = self
                .id
                .unwrap_or_else(|| api::ProjectId::new(Uuid::new_v4().to_string()));

            let owner = self.owner.unwrap_or_else(|| String::from("admin"));

            if self.roles.is_empty() {
                self.roles.insert(
                    RoleId::new(Uuid::new_v4().to_string()),
                    RoleData {
                        name: "myRole".to_owned(),
                        code: "".to_owned(),
                        media: "".to_owned(),
                    },
                );
            }

            ProjectFixture {
                id,
                owner,
                name: self.name.unwrap_or("my project".into()),
                collaborators: self.collaborators,
                roles: self.roles,
                state: self.state,
                //save_state: api::SaveState::Saved,
                traces: self.traces,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct ProjectFixture {
        pub(crate) id: api::ProjectId,
        pub(crate) owner: String,
        pub(crate) name: String,
        pub(crate) collaborators: std::vec::Vec<String>,
        //pub(crate) save_state: api::SaveState,
        pub(crate) roles: HashMap<RoleId, RoleData>,
        pub(crate) traces: Vec<NetworkTraceMetadata>,
        pub(crate) state: PublishState,
    }

    // impl ProjectFixture {
    //     pub(crate) fn to_project(&self) -> Project {
    //         Project {
    //             id: self.id.clone(),
    //             owner: self.owner.clone(),
    //             name: self.name.clone(),
    //             collaborators: self.collaborators.clone(),
    //             roles: self.roles.clone(),
    //             origin_time: self.origin_time.clone(),
    //             save_state: self.save_state.clone(),
    //             updated: self.updated.clone(),
    //             state: self.state.clone(),
    //         }
    //     }
    // }

    pub(crate) fn builder() -> ProjectBuilder {
        ProjectBuilder {
            id: None,
            owner: None,
            name: None,
            collaborators: Vec::new(),
            roles: HashMap::new(),
            traces: Vec::new(),
            state: PublishState::Private,
        }
    }
}

pub(crate) mod network {
    use actix::{Actor, Addr, Context, Handler};
    use netsblox_cloud_common::api::{ClientId, ClientState};
    use uuid::Uuid;

    use crate::network::topology::{
        AddClient, ClientCommand, SetClientState, SetClientUsername, TopologyActor,
    };

    #[derive(Clone)]
    pub(crate) struct Client {
        pub(crate) id: ClientId,
        pub(crate) state: Option<ClientState>,
        username: Option<String>,
    }

    impl Client {
        pub(crate) fn new(username: Option<String>, state: Option<ClientState>) -> Self {
            let id = ClientId::new(format!("_{}", Uuid::new_v4()));
            Self {
                id,
                username,
                state,
            }
        }
        pub(crate) async fn add_into(self, network: &Addr<TopologyActor>) {
            let id = self.id.clone();
            let username = self.username.clone();
            let state = self.state.clone();
            let addr = self.start();
            let recipient = addr.recipient();
            let add_client = AddClient {
                id: id.clone(),
                addr: recipient,
            };
            network.send(add_client).await.unwrap();

            if let Some(state) = state {
                let set_state = SetClientState {
                    id,
                    state,
                    username,
                };
                network.send(set_state).await.unwrap();
            } else {
                let set_username = SetClientUsername { id, username };
                network.send(set_username).await.unwrap();
            }
        }
    }

    impl Actor for Client {
        type Context = Context<Self>;
    }

    impl Handler<ClientCommand> for Client {
        type Result = ();
        fn handle(&mut self, _msg: ClientCommand, _ctx: &mut Self::Context) {
            // We don't yet have any tests that require us to check received messages
            // but the handlers would go run here
        }
    }
}
