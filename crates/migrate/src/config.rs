use std::env;

use config::{ConfigError, File};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Database {
    pub url: String,
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
pub struct StorageConfig {
    pub database: Database,
    pub s3: S3,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub source: StorageConfig,
    pub target: StorageConfig,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".to_owned());
        let mut c = config::Config::new();

        c.merge(File::with_name("config/default"))?;
        c.merge(File::with_name(&format!("config/{}", run_mode)).required(false))?;

        c.try_into()
    }
}
