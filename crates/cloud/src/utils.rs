use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use actix::Addr;
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{api, ProjectMetadata};
use regex::Regex;
use rustrict::CensorStr;
use sha2::{Digest, Sha512};

use crate::{
    errors::{InternalError, UserError},
    network::topology::{self, TopologyActor},
};

pub(crate) fn on_room_changed(
    network: &Addr<TopologyActor>,
    cache: &Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    metadata: ProjectMetadata,
) {
    network.do_send(topology::SendRoomState {
        project: metadata.clone(),
    });

    update_project_cache(cache, metadata);
}

pub(crate) fn update_project_cache(
    cache: &Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    metadata: ProjectMetadata,
) {
    let mut cache = cache.write().unwrap();
    cache.put(metadata.id.clone(), metadata);
}

/// Get a unique project name for the given user and preferred name.
pub(crate) async fn get_valid_project_name(
    project_metadata: &Collection<ProjectMetadata>,
    owner: &str,
    basename: &str,
) -> Result<String, UserError> {
    ensure_valid_name(basename)?;

    let query = doc! {"owner": &owner};
    let cursor = project_metadata
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    let project_names = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .iter()
        .map(|md| md.name.to_owned())
        .collect();

    Ok(get_unique_name(project_names, basename))
}

// FIXME: Can this be rolled into the data type itself?
pub(crate) fn ensure_valid_name(name: &str) -> Result<(), UserError> {
    if !is_valid_name(name) {
        Err(UserError::InvalidRoleOrProjectName)
    } else {
        Ok(())
    }
}

fn is_valid_name(name: &str) -> bool {
    let max_len = 25;
    let min_len = 1;
    let char_count = name.chars().count();
    lazy_static! {
        static ref NAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_ \(\)\-]*$").unwrap();
    }

    char_count >= min_len
        && char_count <= max_len
        && NAME_REGEX.is_match(name)
        && !name.is_inappropriate()
}

pub(crate) fn get_unique_name(existing: Vec<String>, name: &str) -> String {
    let names: HashSet<std::string::String> = HashSet::from_iter(existing.iter().cloned());
    let base_name = name;
    let mut role_name = base_name.to_owned();
    let mut number: u8 = 2;
    while names.contains(&role_name) {
        role_name = format!("{} ({})", base_name, number);
        number += 1;
    }
    role_name
}

pub(crate) fn is_approval_required(text: &str) -> bool {
    text.contains("reportJSFunction") || text.is_inappropriate()
}

pub(crate) fn sha512(text: &str) -> String {
    let mut hasher = Sha512::new();
    hasher.update(text);
    let hash = hasher.finalize();
    hex::encode(hash)
}
