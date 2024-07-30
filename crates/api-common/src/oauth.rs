use std::time::SystemTime;

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

#[cfg(feature = "wasm")]
use tsify_next::Tsify;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Display, Hash, FromStr)]
#[cfg_attr(feature = "wasm", derive(Tsify), tsify(into_wasm_abi, from_wasm_abi))]
pub struct ClientId(String);

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
#[cfg_attr(feature = "wasm", derive(Tsify), tsify(into_wasm_abi, from_wasm_abi))]
pub struct CreateClientData {
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(Tsify), tsify(into_wasm_abi, from_wasm_abi))]
pub struct CreatedClientData {
    pub id: ClientId,
    pub secret: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[cfg_attr(feature = "wasm", derive(Tsify), tsify(into_wasm_abi, from_wasm_abi))]
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
