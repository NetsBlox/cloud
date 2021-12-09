use crate::network::topology::Topology;
use actix::{Actor, Addr};
use mongodb::{Collection, Database};

pub struct AppData {
    prefix: &'static str,
    pub db: Database,
    pub network: Addr<Topology>,
}

impl AppData {
    pub fn new(
        db: Database,
        network: Option<Addr<Topology>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let network = network.unwrap_or(Topology::new().start());
        let prefix = prefix.unwrap_or("");
        AppData {
            db,
            network,
            prefix,
        }
    }

    pub fn collection<T>(&self, name: &str) -> Collection<T> {
        let name = &(self.prefix.to_owned() + name);
        self.db.collection::<T>(name)
    }
}
