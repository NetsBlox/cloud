use actix::Addr;
use actix_session::SessionExt;
use actix_web::HttpRequest;
use futures::TryStreamExt;
use lettre::{Message, SmtpTransport, Transport};
use log::error;
use lru::LruCache;
use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{
    api::{self, GroupId, UserRole},
    AuthorizedServiceHost, FriendLink, Group, ProjectMetadata, User,
};
use nonempty::NonEmpty;
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
) -> ProjectMetadata {
    network.do_send(topology::SendRoomState {
        project: metadata.clone(),
    });

    update_project_cache(cache, metadata)
}

pub(crate) fn update_project_cache(
    cache: &Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    metadata: ProjectMetadata,
) -> ProjectMetadata {
    let mut cache = cache.write().unwrap();
    let latest = cache
        .get(&metadata.id)
        .and_then(|existing| {
            if existing.updated > metadata.updated {
                Some(existing.to_owned())
            } else {
                None
            }
        })
        .unwrap_or(metadata);

    cache.put(latest.id.clone(), latest.clone());

    latest
}

/// Get a unique project name for the given user and preferred name.
pub(crate) async fn get_valid_project_name(
    project_metadata: &Collection<ProjectMetadata>,
    owner: &str,
    basename: &str,
) -> Result<String, UserError> {
    let query = doc! {"owner": &owner};
    let cursor = project_metadata
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;
    let project_names: Vec<_> = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|md| md.name)
        .collect();

    get_unique_name(project_names.iter().map(|n| n.as_str()), basename)
}

pub(crate) fn get_unique_name<'a>(
    existing: impl Iterator<Item = &'a str>,
    basename: &str,
) -> Result<String, UserError> {
    let candidates = std::iter::once(basename.into())
        .chain((2..=1000).map(|n| format!("{} ({})", &basename, n)));

    find_first_unique(existing, candidates).ok_or(UserError::RoleOrProjectNameExists)
}

pub(crate) fn find_first_unique<'a>(
    existing: impl Iterator<Item = &'a str>,
    mut candidates: impl Iterator<Item = String>,
) -> Option<String> {
    let names: HashSet<&str> = HashSet::from_iter(existing);
    //get_unique_str(names, name, |s, n| format!("{} ({})", s, n))
    candidates.find(|name| !names.contains(name.as_str()))
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

pub(crate) fn send_email(
    mailer: &SmtpTransport,
    email: impl TryInto<Message>,
) -> Result<(), UserError> {
    let message = email
        .try_into()
        .map_err(|_err| InternalError::EmailBuildError)?;

    mailer
        .send(&message)
        .map_err(InternalError::SendEmailError)?;

    Ok(())
}

/// Find the usernames given the associated email address.
///
/// Results in a UserNotFound error if no users are associated with the given email address.
///
pub(crate) async fn find_usernames(
    collection: &Collection<User>,
    email: &str,
) -> Result<NonEmpty<String>, UserError> {
    let query = doc! {"email": email};
    let cursor = collection
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    let usernames = cursor
        .map_ok(|user| user.username)
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    NonEmpty::from_vec(usernames).ok_or(UserError::UserNotFoundError)
}

// TODO: tests for cache invalidation
// - [ ] friends

// TODO: tests for friend-checking

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        num::NonZeroUsize,
        time::{Duration, SystemTime},
    };

    use lru::LruCache;
    use mongodb::bson::DateTime;

    use crate::test_utils;

    use super::*;

    #[actix_web::test]
    async fn test_update_project_cache_ignore_stale() {
        // This issue was discovered around old projects hanging around in the project cache
        // due to what appears to be a high-level race condition
        // - set publish state (get published one, metadata1)
        // - rename (get renamed and published one, metadata2)
        //   - add update time and use this when updating the cache?
        // - update cache with metadata2
        // - update cache with metadata1
        let original = ProjectMetadata::new("owner", "name", HashMap::new(), api::SaveState::Saved);
        let id = original.id.clone();
        let mut new_project = original.clone();
        new_project.name = "new name".into();
        new_project.updated =
            DateTime::from_system_time(SystemTime::now() + Duration::from_secs(10));

        let project_cache = Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(2).unwrap())));

        // Suppose concurrent requests try to update the cache in the wrong order
        update_project_cache(&project_cache, new_project);
        update_project_cache(&project_cache, original);

        // check that it still has the latest
        let mut cache = project_cache.write().unwrap();
        let metadata = cache.get(&id).unwrap();
        assert_eq!(&metadata.name, "new name");
    }

    #[actix_web::test]
    async fn test_update_project_cache_tie_goes_to_update() {
        let original = ProjectMetadata::new("owner", "name", HashMap::new(), api::SaveState::Saved);
        let id = original.id.clone();
        let mut new_project = original.clone();
        new_project.name = "new name".into();

        let project_cache = Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(2).unwrap())));

        // Suppose concurrent requests try to update the cache with the same update time
        update_project_cache(&project_cache, original);
        update_project_cache(&project_cache, new_project);

        // check that it still has the latest
        let mut cache = project_cache.write().unwrap();
        let metadata = cache.get(&id).unwrap();
        assert_eq!(&metadata.name, "new name");
    }

    #[actix_web::test]
    async fn test_get_unique_name() {
        let names = ["name", "name (2)", "name (3)", "name (4)"].into_iter();
        let name = get_unique_name(names.clone(), "name").unwrap();
        assert_eq!(name, "name (5)");
    }

    #[actix_web::test]
    async fn test_get_unique_name_existing_parens() {
        let names = ["name", "name (2)", "name (3)", "name (4)"].into_iter();
        let name = get_unique_name(names, "name (3)").unwrap();
        assert_eq!(name, "name (3) (2)");
    }

    #[actix_web::test]
    async fn test_get_unique_name_none() {
        let existing: Vec<_> = std::iter::once("name".to_string())
            .chain((2..10000).map(|n| format!("name ({})", n)))
            .collect();

        let name_res = get_unique_name(existing.iter().map(|n| n.as_str()), "name");
        assert!(matches!(name_res, Err(UserError::RoleOrProjectNameExists)));
    }

    #[actix_web::test]
    async fn test_find_usernames() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone(), other])
            .run(|app_data| async move {
                let usernames = find_usernames(&app_data.users, &user.email).await.unwrap();
                assert_eq!(usernames.len(), 1);
                assert!(usernames.iter().any(|name| name == "user"));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_find_usernames_multi() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let u2: User = api::NewUser {
            username: "u2".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone(), u2, other])
            .run(|app_data| async move {
                let usernames = find_usernames(&app_data.users, &user.email).await.unwrap();
                assert_eq!(usernames.len(), 2);
                assert!(usernames.iter().any(|name| name == "user"));
                assert!(usernames.iter().any(|name| name == "u2"));
            })
            .await;
    }
}
