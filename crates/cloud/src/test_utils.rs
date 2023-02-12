use std::sync::{Arc, Mutex};

use actix_web::{
    dev::Service,
    test::{self, TestRequest},
    web::{self, ServiceConfig},
    App,
};
use futures::{future::join_all, Future};
use lazy_static::lazy_static;
use mongodb::Client;
use netsblox_cloud_common::{Group, Project, User};

use crate::{app_data::AppData, config::Settings};

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
        projects: Vec::new(),
        groups: Vec::new(),
    }
}

pub(crate) struct TestSetupBuilder {
    prefix: String,
    users: Vec<User>,
    projects: Vec<Project>,
    groups: Vec<Group>,
}

impl TestSetupBuilder {
    pub(crate) fn with_users(mut self, users: &[User]) -> Self {
        self.users.extend_from_slice(users);
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

    pub(crate) async fn run<Fut>(self, f: impl FnOnce(AppData) -> Fut)
    where
        Fut: Future<Output = ()>,
    {
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        let mut settings = Settings::new().unwrap();
        let db_name = format!("test_{}", settings.database.name);
        settings.database.name = db_name.clone();
        settings.s3.bucket = format!("test_{}", settings.s3.bucket);

        let app_data = AppData::new(client.clone(), settings, None, Some(self.prefix));

        // create the test fixtures (users, projects)
        client.database(&db_name).drop(None).await.unwrap();
        join_all(self.projects.iter().map(|proj| {
            let Project {
                owner,
                name,
                roles,
                save_state,
                ..
            } = proj;
            let roles: Vec<_> = roles.values().map(|r| r.to_owned()).collect();
            app_data.import_project(owner, name, Some(roles), Some(save_state.clone()))
        }))
        .await;
        if !self.users.is_empty() {
            app_data.users.insert_many(self.users, None).await.unwrap();
        }
        if !self.groups.is_empty() {
            app_data
                .groups
                .insert_many(self.groups, None)
                .await
                .unwrap();
        }

        f(app_data.clone()).await;

        // cleanup
        client.database(&db_name).drop(None).await.unwrap();
        app_data.drop_all_data().await;
        // TODO: delete s3 bucket
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

        pub(crate) fn with_id(mut self, id: api::ProjectId) -> Self {
            self.id = Some(id);
            self
        }
        pub(crate) fn build(self) -> Project {
            let roles = HashMap::new(); // FIXME: we should populate with some defaults..
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
                collaborators: Vec::new(),
                origin_time: DateTime::now(),
                save_state: api::SaveState::SAVED,
                roles,
            }
        }
    }

    pub(crate) fn builder() -> ProjectBuilder {
        ProjectBuilder {
            id: None,
            owner: None,
            name: None,
        }
    }
}
