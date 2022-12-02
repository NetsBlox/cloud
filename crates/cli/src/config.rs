use std::collections::HashMap;

use lazy_static::lazy_static;
use netsblox_api::{self, common::AppId};
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref DEFAULT_HOST: HostConfig = HostConfig::default();
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct HostConfig {
    pub(crate) url: String,
    pub(crate) username: Option<String>,
    pub(crate) token: Option<String>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            url: "https://cloud.netsblox.org".to_owned(),
            username: None,
            token: None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct Config {
    pub(crate) current_host: String,
    pub(crate) hosts: HashMap<String, HostConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let current_host = String::from("cloud");
        let dev_host = HostConfig {
            url: String::from("http://localhost:7777"),
            username: None,
            token: None,
        };
        let hosts = HashMap::from([
            (current_host.clone(), HostConfig::default()),
            (String::from("dev"), dev_host),
        ]);

        Self {
            current_host,
            hosts,
        }
    }
}

impl Config {
    pub(crate) fn host(&self) -> &HostConfig {
        self.hosts.get(&self.current_host).unwrap_or(&DEFAULT_HOST) // TODO: add a warning log?
    }

    pub(crate) fn set_credentials(&mut self, api_cfg: &netsblox_api::Config) {
        if let Some(cfg) = self.hosts.get_mut(&self.current_host) {
            cfg.username = api_cfg.username.to_owned();
            cfg.token = api_cfg.token.to_owned();
        }
    }

    pub(crate) fn clear_credentials(&mut self) {
        if let Some(cfg) = self.hosts.get_mut(&self.current_host) {
            cfg.username = None;
            cfg.token = None;
        }
    }
}

impl From<HostConfig> for netsblox_api::Config {
    fn from(config: HostConfig) -> netsblox_api::Config {
        netsblox_api::Config {
            app_id: Some(AppId::new("NetsBloxCLI")),
            url: config.url,
            username: config.username,
            token: config.token,
        }
    }
}
