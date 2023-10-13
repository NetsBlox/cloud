use actix::Addr;
use actix_session::SessionExt;
use actix_web::HttpRequest;
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use log::{error, warn};
use lru::LruCache;
use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{
    api::{self, GroupId, UserRole},
    AuthorizedServiceHost, FriendLink, Group, ProjectMetadata, User,
};
use regex::Regex;
use rustrict::CensorStr;
use sha2::{Digest, Sha512};
use std::{
    borrow::Borrow,
    collections::HashSet,
    sync::{Arc, RwLock},
};

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
    let max_len = 50;
    let min_len = 1;
    let char_count = name.chars().count();
    lazy_static! {
        static ref NAME_REGEX: Regex = Regex::new(r"^[\w\d_][\w\d_ \(\)\.,\-]*$").unwrap();
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

// Friends
pub(crate) async fn get_friends(
    users: &Collection<User>,
    groups: &Collection<Group>,
    friends: &Collection<FriendLink>,
    friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,
    username: &str,
) -> Result<Vec<String>, UserError> {
    let friend_names = if let Some(names) = get_cached_friends(friend_cache.clone(), username) {
        names
    } else {
        let names = lookup_friends(users, groups, friends, username).await?;
        let mut cache = friend_cache.write().unwrap();
        cache.put(username.to_owned(), names.clone());
        names
    };
    Ok(friend_names)
}

async fn lookup_friends(
    users: &Collection<User>,
    groups: &Collection<Group>,
    friends: &Collection<FriendLink>,
    username: &str,
) -> Result<Vec<String>, UserError> {
    let query = doc! {"username": &username};
    let user = users
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)?;

    let is_universal_friend = matches!(user.role, UserRole::Admin);

    let friend_names: Vec<_> = if is_universal_friend {
        users
            .find(doc! {}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<User>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|user| user.username)
            .filter(|name| name != username)
            .collect()
    } else if let Some(group_id) = user.group_id {
        // get owner + all members
        let query = doc! {"id": &group_id};
        let group = groups
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;
        let members = lookup_members(users, std::iter::once(&group_id)).await?;

        std::iter::once(group.owner)
            .chain(members.into_iter().map(|user| user.username))
            .collect()
    } else {
        // look up:
        //   - members of any group we own
        //   - accepted friend requests/links
        let query = doc! {"owner": &username};
        let groups = groups
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let group_ids = groups.into_iter().map(|group| group.id);
        let members = lookup_members(users, group_ids).await?;

        let query = doc! {"$or": [
            {"sender": &username, "state": api::FriendLinkState::Approved},
            {"recipient": &username, "state": api::FriendLinkState::Approved}
        ]};
        let cursor = friends
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let links = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        links
            .into_iter()
            .map(|l| {
                if l.sender == username {
                    l.recipient
                } else {
                    l.sender
                }
            })
            .chain(members.into_iter().map(|user| user.username))
            .collect()
    };

    Ok(friend_names)
}

async fn lookup_members<T>(
    users: &Collection<User>,
    group_ids: impl Iterator<Item = T>,
) -> Result<Vec<User>, UserError>
where
    T: Borrow<GroupId>,
{
    let member_queries: Vec<_> = group_ids.map(|id| doc! {"groupId": id.borrow()}).collect();
    if !member_queries.is_empty() {
        let query = doc! {"$or": member_queries};

        let members = users
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(members)
    } else {
        Ok(Vec::new())
    }
}

/// Invalidate the relevant cached values when a user is added or removed
/// from a group
pub(crate) async fn group_members_updated(
    users: &Collection<User>,
    friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,
    group_id: &GroupId,
) {
    if let Ok(members) = lookup_members(users, std::iter::once(group_id)).await {
        let mut cache = friend_cache.write().unwrap();
        members.into_iter().for_each(|user| {
            cache.pop(&user.username);
        });
    } else {
        error!("Error occurred while retrieving members for {}", group_id);
    }
}

fn get_cached_friends(
    friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,
    username: &str,
) -> Option<Vec<String>> {
    let mut cache = friend_cache.write().unwrap();
    cache.get(username).map(|friends| friends.to_owned())
}

pub(crate) fn get_username(req: &HttpRequest) -> Option<String> {
    let session = req.get_session();
    println!("{:?}", session.get::<String>("username"));
    session.get::<String>("username").unwrap_or(None)
}

pub(crate) async fn get_authorized_host(
    authorized_services: &Collection<AuthorizedServiceHost>,
    req: &HttpRequest,
) -> Result<Option<AuthorizedServiceHost>, UserError> {
    let query = req
        .headers()
        .get("X-Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value_str| {
            let mut chunks = value_str.split(':');
            let id = chunks.next();
            let secret = chunks.next();
            id.and_then(|id| secret.map(|s| (id, s)))
        })
        .map(|(id, secret)| doc! {"id": id, "secret": secret});

    let host = authorized_services
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(host)
}

pub(crate) fn send_email<T: lettre::Transport>(
    mailer: &T,
    email: impl TryInto<Message>,
) -> Result<(), UserError>
where
    <T as Transport>::Error: std::fmt::Debug,
{
    let message = email
        .try_into()
        .map_err(|_err| InternalError::EmailBuildError)?;

    mailer.send(&message).map_err(|err| {
        warn!("Unable to send email: {:?}", err);
        InternalError::SendEmailError
    })?;

    Ok(())
}

// TODO: tests for cache invalidation
// - [ ] friends

// TODO: tests for friend-checking

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_x_is_valid_name() {
        assert!(is_valid_name("X"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_spaces() {
        assert!(is_valid_name("Player 1"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_leading_nums() {
        assert!(is_valid_name("2048 Game"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_dashes() {
        assert!(is_valid_name("Player-i"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_long_name() {
        assert!(is_valid_name("RENAMED-rename-test-1696865702584"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_parens() {
        assert!(is_valid_name("untitled (20)"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_dots() {
        assert!(is_valid_name("untitled v1.2"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_comma() {
        assert!(is_valid_name("Lab2, SomeName"));
    }

    #[actix_web::test]
    async fn test_is_valid_name_profanity() {
        assert!(!is_valid_name("shit"));
        assert!(!is_valid_name("fuck"));
        assert!(!is_valid_name("damn"));
        assert!(!is_valid_name("hell"));
    }
}
