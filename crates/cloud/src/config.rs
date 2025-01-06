use std::{env, num::NonZeroUsize};

use figment::{
    providers::{Format, Toml},
    Figment,
};
use netsblox_cloud_common::api::ServiceHostScope;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Database {
    pub url: String,
    pub name: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct S3 {
    pub bucket: String,
    pub endpoint: String,
    pub region_name: String,
    pub credentials: S3Credentials,
}

#[derive(Clone, Deserialize, Debug)]
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct CookieSettings {
    pub name: String,
    pub domain: String,
    pub key: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct EmailSettings {
    pub sender: String,
    pub smtp: SMTPSettings,
}

#[derive(Clone, Deserialize, Debug)]
pub struct SMTPSettings {
    pub host: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct SecuritySettings {
    pub allow_tor_login: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct UserCreds {
    pub username: String,
    pub password: String,
    pub email: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct CacheSettings {
    pub num_projects: NonZeroUsize,
    pub num_users_membership_data: NonZeroUsize,
    pub num_users_admin_data: NonZeroUsize,
    pub num_users_friend_data: NonZeroUsize,
    pub num_addresses: NonZeroUsize,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AuthorizedServiceHost {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) secret: String,
    pub(crate) category: Option<String>,
}

impl From<AuthorizedServiceHost> for netsblox_cloud_common::AuthorizedServiceHost {
    fn from(config: AuthorizedServiceHost) -> Self {
        let categories = config.category.map(|cat| vec![cat]).unwrap_or_default();
        Self {
            id: config.id,
            url: config.url,
            secret: config.secret,
            visibility: ServiceHostScope::Public(categories),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct Settings {
    pub address: String,
    pub public_url: String,
    pub login_url: Option<String>,
    pub database: Database,
    pub s3: S3,
    pub cookie: CookieSettings,
    pub email: EmailSettings,
    pub security: SecuritySettings,
    pub admin: Option<UserCreds>,
    pub authorized_host: Option<AuthorizedServiceHost>,
    pub cache_settings: CacheSettings,
}

impl Settings {
    pub fn new() -> Result<Self, figment::Error> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".to_owned());
        let c: Settings = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file(format!("config/{}.toml", run_mode)))
            .extract()?;

        Ok(c)
    }
}
