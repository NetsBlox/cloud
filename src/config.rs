use std::env;

use crate::models::ServiceHost;
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
}

#[derive(Clone, Deserialize)]
pub struct Settings {
    pub address: String,
    pub public_url: String,
    pub database: Database,
    pub s3: S3,
    pub services_hosts: Vec<ServiceHost>,
    pub cookie: CookieSettings,
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
