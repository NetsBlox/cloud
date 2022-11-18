use std::env;

use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Database {
    pub url: String,
    pub name: String,
}

#[derive(Clone, Deserialize)]
pub struct S3 {
    pub bucket: String,
    pub endpoint: String,
    pub region_name: String,
    pub credentials: S3Credentials,
}

#[derive(Clone, Deserialize)]
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Clone, Deserialize)]
pub struct CookieSettings {
    pub name: String,
    pub domain: String,
    pub key: String,
}

#[derive(Clone, Deserialize)]
pub struct EmailSettings {
    pub sender: String,
    pub smtp: SMTPSettings,
}

#[derive(Clone, Deserialize)]
pub struct SMTPSettings {
    pub host: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize)]
pub struct SecuritySettings {
    pub allow_tor_login: bool,
}

#[derive(Clone, Deserialize)]
pub struct UserCreds {
    pub username: String,
    pub password: String,
    pub email: String,
}

#[derive(Clone, Deserialize)]
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
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".to_owned());
        let mut c = Config::new();

        c.merge(File::with_name("config/default"))?;
        c.merge(File::with_name(&format!("config/{}", run_mode)).required(false))?;

        c.try_into()
    }
}
