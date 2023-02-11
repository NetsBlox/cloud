use actix_web::{
    dev::Service,
    test::{self, TestRequest},
    web::{self, ServiceConfig},
    App,
};
use futures::Future;
use mongodb::Client;
use netsblox_cloud_common::User;

use crate::{app_data::AppData, config::Settings};

pub(crate) fn setup() -> TestSetupBuilder {
    TestSetupBuilder {
        prefix: "todo",
        users: Vec::new(),
    }
}

pub(crate) struct TestSetupBuilder {
    prefix: &'static str,
    users: Vec<User>,
}

impl TestSetupBuilder {
    pub(crate) fn with_users(mut self, users: &[User]) -> Self {
        self.users.extend_from_slice(users);
        self
    }

    // pub(crate) async fn run<Fut, S>(&self, f: impl FnOnce(S, AppData) -> Fut)
    // where
    //     Fut: Future<Output = Result<(), ()>>,
    //     S: Service<TestRequest>,
    // TODO: what is the type of the test service
    pub(crate) async fn run<Fut, S>(&self, f: impl FnOnce(&S, &AppData) -> Fut)
    where
        Fut: Future<Output = Result<(), ()>>,
        S: Service<TestRequest>,
    {
        // TODO: setup things
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        let mut settings = Settings::new().unwrap();
        let db_name = format!("test_{}", settings.database.name);
        settings.database.name = db_name.clone();
        settings.s3.bucket = format!("test_{}", settings.s3.bucket);

        let app_data = AppData::new(client.clone(), settings, None, Some(self.prefix));
        let app = test::init_service(
            App::new().app_data(web::Data::new(app_data.clone())), //.configure(config),
        )
        .await;

        // TODO: pass the service, too
        f(&app, &app_data).await.unwrap();

        // cleanup
        client.database(&db_name).drop(None).await.unwrap();
        app_data.drop_all_data().await;
        // TODO: delete s3 bucket
    }
}
