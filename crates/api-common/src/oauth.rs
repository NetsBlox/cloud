use std::time::SystemTime;

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

use wasm_bindgen::prelude::*;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, FromStr)]
#[wasm_bindgen(getter_with_clone)]
pub struct ClientId(String);

#[wasm_bindgen(constructor)]
impl ClientId {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}
impl ClientId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Display)]
#[wasm_bindgen(getter_with_clone)]
pub struct CreateClientData {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[wasm_bindgen(getter_with_clone)]
pub struct CreatedClientData {
    pub id: ClientId,
    pub secret: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[wasm_bindgen(getter_with_clone)]
pub struct Client {
    pub id: ClientId,
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, FromStr)]
pub struct CodeId(String);

impl CodeId {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    pub id: CodeId,
    pub username: String,
    pub client_id: ClientId,
    pub redirect_uri: String,
    pub created_at: SystemTime,
}

#[derive(Deserialize, Serialize, Clone, Debug, Display, FromStr)]
pub struct TokenId(String);

impl TokenId {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: TokenId,
    pub client_id: ClientId,
    pub username: String,
    pub created_at: SystemTime,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CreateTokenParams {
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub grant_type: Option<String>,
}
