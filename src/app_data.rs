use crate::network::topology::Topology;
use mongodb::{Database,Collection};
use std::sync::Mutex;

pub struct AppData {
    pub database: Database,
    pub network: Mutex<Topology>,
}

impl AppData {
    pub fn new(database: Database, network: Option<Topology>) -> AppData {
        let network = Mutex::new(network.unwrap_or(Topology::new()));
        AppData{database, network}
    }
}

