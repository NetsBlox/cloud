use crate::network::topology::Topology;
use mongodb::{Database,Collection};
use std::sync::Mutex;
use actix::{Actor,Addr};

pub struct AppData {
    pub database: Database,
    pub network: Addr<Topology>,
}

impl AppData {
    pub fn new(database: Database, network: Option<Addr<Topology>>) -> AppData {
        let network = network.unwrap_or(Topology::new().start());
        //let network = Mutex::new(network.unwrap_or(Topology::new()));
        AppData{database, network}
    }
}

