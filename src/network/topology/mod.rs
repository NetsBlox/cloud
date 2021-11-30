pub mod client;
use client::Client;

#[derive(Clone)]
pub struct Topology {
    clients: std::vec::Vec<Client>,
}

impl Topology {
    pub fn new() -> Topology {
        Topology{clients: std::vec::Vec::new()}
    }

    pub fn add_client(&mut self, client: Client) {
        self.clients.push(client);
    }
}
