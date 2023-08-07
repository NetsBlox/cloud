use futures::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::{bson::doc, options::UpdateOptions, Collection};
use netsblox_cloud_common::{api, AuthorizedServiceHost};
use regex::Regex;

use crate::{
    auth,
    errors::{InternalError, UserError},
};

pub(crate) struct HostActions {
    authorized_services: Collection<AuthorizedServiceHost>,
}

impl HostActions {
    pub(crate) fn new(authorized_services: Collection<AuthorizedServiceHost>) -> Self {
        Self {
            authorized_services,
        }
    }

    pub(crate) async fn get_hosts(
        &self,
        _lh: &auth::ViewAuthHosts,
    ) -> Result<Vec<api::AuthorizedServiceHost>, UserError> {
        let query = doc! {};
        let cursor = self
            .authorized_services
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let hosts: Vec<api::AuthorizedServiceHost> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|host| host.into())
            .collect();

        Ok(hosts)
    }
    pub(crate) async fn authorize(
        &self,
        _lh: &auth::AuthorizeHost,
        host: api::AuthorizedServiceHost,
    ) -> Result<String, UserError> {
        ensure_valid_service_id(&host.id)?;

        let query = doc! {"id": &host.id};
        let host: AuthorizedServiceHost = host.into();
        let update = doc! {"$setOnInsert": &host};
        let options = UpdateOptions::builder().upsert(true).build();
        let result = self
            .authorized_services
            .update_one(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 0 {
            Ok(host.secret)
        } else {
            Err(UserError::ServiceHostAlreadyAuthorizedError)
        }
    }

    pub(crate) async fn unauthorize(
        &self,
        lh: &auth::AuthorizeHost,
        host_id: &str,
    ) -> Result<api::AuthorizedServiceHost, UserError> {
        let query = doc! {"id": &host_id};
        let host = self
            .authorized_services
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::ServiceHostNotFoundError)?;

        Ok(host.into())
    }
}

pub fn ensure_valid_service_id(id: &str) -> Result<(), UserError> {
    let max_len = 25;
    let min_len = 3;
    let char_count = id.chars().count();
    lazy_static! {
        // This is safe to unwrap since it is a constant
        static ref SERVICE_ID_REGEX: Regex = Regex::new(r"^[A-Za-z][A-Za-z0-9_\-]+$").unwrap();
    }

    let is_valid = char_count > min_len && char_count < max_len && SERVICE_ID_REGEX.is_match(id);
    if is_valid {
        Ok(())
    } else {
        Err(UserError::InvalidServiceHostIDError)
    }
}
