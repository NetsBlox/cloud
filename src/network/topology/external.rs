use std::collections::HashMap;

use super::address::ClientAddress;
use actix::Message;

// TODO: this can be generalized:
//   we have ExternalNetworks:
//     - netsblox
//     - pyblox
//     - etc

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ExternalClientState {
    address: String,
    user_id: String,
    pub app_id: String,
}

pub struct ExternalNetwork {
    states: HashMap<String, ExternalClientState>,
    apps: HashMap<String, HashMap<String, String>>,
}

impl ExternalNetwork {
    pub fn new() -> ExternalNetwork {
        ExternalNetwork {
            apps: HashMap::new(),
            states: HashMap::new(),
        }
    }

    fn reset_client_state(&mut self, id: &str) {
        if let Some(state) = self.states.remove(id) {
            if let Some(network) = self.apps.get_mut(&state.app_id) {
                if network.keys().len() == 1 {
                    self.apps.remove(&state.app_id);
                } else {
                    network.remove(&state.address);
                }
            }
        }
    }

    pub fn set_client_state(&mut self, client_id: &str, state: ExternalClientState) {
        self.reset_client_state(&client_id);

        self.states.insert(client_id.to_string(), state.clone());
        let app_id = state.app_id;
        let address = state.address;

        let mut empty_network = HashMap::new();
        let network = self
            .apps
            .get_mut(&app_id)
            .unwrap_or_else(|| &mut empty_network);
        network.insert(address, client_id.to_string());
    }

    pub fn get_clients_at(&self, addr: &ClientAddress) -> Vec<&String> {
        // TODO: Make a client ID type?
        todo!();
    }
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_set_client_state() {
        todo!();
    }

    #[actix_web::test]
    async fn test_set_client_new_state() {
        todo!();
    }
}
