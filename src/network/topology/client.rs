use serde_json::Value;

#[derive(Clone)]
pub struct Client {
    id: String,
    // TODO: we need a reference to a connection/socket that we can send messages to
}

impl Client {
    pub fn new(id: String) -> Client {
        Client{id}
    }

    fn send_msg(&self, msg: Value) {
        // TODO: send a message to the current websocket
    }

    pub fn handle_msg(&self, msg_type: String, msg: Value) {
        match msg_type.as_str() {
            "message" => {
                let addresses = match &msg["dstId"] {
                    Value::String(address) => vec![address.as_str()],
                    Value::Array(values) => values.iter()
                        .filter(|v| v.is_string())
                        .map(|v| v.as_str().unwrap())
                        .collect::<Vec<&str>>(),
                    _ => std::vec::Vec::new(),
                };
                let recipients = addresses.iter()
                    .map(|addr| self.send_msg_to(msg.clone(), addr));

                // TODO: resolve the IDs
                //srcProjectId = 
                //dstId = addresses
                //recipients = recipients.flat()
                // TODO: save the message
            },
            "client-message" => {  // combine this with the above type?
            },
            "user-action" => {
            },
            "project-response" => {  // TODO: move this to rest?
            },
            "request-actions" => {  // TODO: move this to REST?
            },
            _ => {
                println!("unrecognized message type: {}", msg_type);
            }
        }
    }

    fn send_msg_to(&self, msg: Value, addr: &str) -> std::vec::Vec<String> {
        unimplemented!();
    }
}


