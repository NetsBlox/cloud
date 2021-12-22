use crate::models::{Group, User};
use crate::network::topology::Topology;
use actix::{Actor, Addr};
use mongodb::{Collection, Database};
use rusoto_s3::S3Client;

pub struct AppData {
    prefix: &'static str,
    pub db: Database,
    pub network: Addr<Topology>,
    pub s3: S3Client,
    pub groups: Collection<Group>,
    pub users: Collection<User>,
}

impl AppData {
    pub fn new(
        db: Database,
        s3: S3Client,
        network: Option<Addr<Topology>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let network = network.unwrap_or(Topology::new().start());
        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        AppData {
            db,
            network,
            s3,
            groups,
            users,
            prefix,
        }
    }

    pub fn collection<T>(&self, name: &str) -> Collection<T> {
        let name = &(self.prefix.to_owned() + name);
        self.db.collection::<T>(name)
    }
}
