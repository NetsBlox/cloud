use std::sync::{Arc, Mutex};

use actix::Addr;
use futures::{future::join_all, Future};
use lazy_static::lazy_static;
use mongodb::{bson::doc, Client};
use netsblox_cloud_common::{BannedAccount, FriendLink, Group, Project, User};

use crate::{app_data::AppData, config::Settings, network::topology::TopologyActor};

lazy_static! {
    static ref COUNTER: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
}

pub(crate) fn setup() -> TestSetupBuilder {
    let mut counter = COUNTER.lock().unwrap();
    *counter += 1_u32;
    let prefix = format!("test_{}", counter);
    TestSetupBuilder {
        prefix,
        users: Vec::new(),
        banned_users: Vec::new(),
        projects: Vec::new(),
        groups: Vec::new(),
        clients: Vec::new(),
        friends: Vec::new(),
        network: None,
    }
}

pub(crate) struct TestSetupBuilder {
    prefix: String,
    users: Vec<User>,
    projects: Vec<Project>,
    groups: Vec<Group>,
    clients: Vec<network::Client>,
    friends: Vec<FriendLink>,
    banned_users: Vec<String>,
    network: Option<Addr<TopologyActor>>,
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

    pub(crate) fn with_projects(mut self, projects: &[Project]) -> Self {
        self.projects.extend_from_slice(projects);
        self
    }

    pub(crate) fn with_groups(mut self, groups: &[Group]) -> Self {
        self.groups.extend_from_slice(groups);
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

    pub(crate) fn with_network(mut self, network: Addr<TopologyActor>) -> Self {
        self.network = Some(network);
        self
    }

    pub(crate) async fn run<Fut>(self, f: impl FnOnce(AppData) -> Fut)
    where
        Fut: Future<Output = ()>,
    {
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        let mut settings = Settings::new().unwrap();
        let db_name = format!("{}_{}", &self.prefix, settings.database.name);
        settings.database.name = db_name.clone();
        settings.s3.bucket = format!("{}_{}", &self.prefix, settings.s3.bucket);

        let app_data = AppData::new(client.clone(), settings, None, None);

        // create the test fixtures (users, projects, etc)
        client.database(&db_name).drop(None).await.unwrap();
        join_all(self.projects.iter().map(|proj| async {
            let Project {
                id,
                owner,
                name,
                roles,
                save_state,
                ..
            } = proj;
            let roles: Vec<_> = roles.values().map(|r| r.to_owned()).collect();
            let metadata = app_data
                .import_project(owner, name, Some(roles), Some(save_state.clone()))
                .await
                .unwrap();

            let query = doc! {"id": metadata.id};
            let update = doc! {"$set": {"id": id}};
            app_data
                .project_metadata
                .update_one(query, update, None)
                .await
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
        if !self.users.is_empty() {
            app_data.users.insert_many(self.users, None).await.unwrap();
        }
        if !self.friends.is_empty() {
            app_data.insert_friends(&self.friends).await.unwrap();
        }
        if !self.groups.is_empty() {
            app_data
                .groups
                .insert_many(self.groups, None)
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

    use mongodb::bson::DateTime;
    use netsblox_cloud_common::{api, Project};
    use uuid::Uuid;

    pub(crate) struct ProjectBuilder {
        id: Option<api::ProjectId>,
        owner: Option<String>,
        name: Option<String>,
        collaborators: Vec<String>,
        roles: HashMap<api::RoleId, api::RoleData>,
    }

    impl ProjectBuilder {
        pub(crate) fn with_name(mut self, name: String) -> Self {
            self.name = Some(name);
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

        pub(crate) fn with_id(mut self, id: api::ProjectId) -> Self {
            self.id = Some(id);
            self
        }

        pub(crate) fn with_collaborators(mut self, names: &[&str]) -> Self {
            self.collaborators = names.iter().map(|n| n.to_string()).collect::<Vec<String>>();
            self
        }

        pub(crate) fn build(self) -> Project {
            let id = self
                .id
                .unwrap_or_else(|| api::ProjectId::new(Uuid::new_v4().to_string()));

            let owner = self.owner.unwrap_or_else(|| String::from("admin"));

            Project {
                id,
                owner,
                name: "old name".into(),
                updated: DateTime::now(),
                state: api::PublishState::Private,
                collaborators: self.collaborators,
                origin_time: DateTime::now(),
                save_state: api::SaveState::SAVED,
                roles: self.roles,
            }
        }
    }

    pub(crate) fn builder() -> ProjectBuilder {
        ProjectBuilder {
            id: None,
            owner: None,
            name: None,
            collaborators: Vec::new(),
            roles: HashMap::new(),
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
        state: Option<ClientState>,
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
