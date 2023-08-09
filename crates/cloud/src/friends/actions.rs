use std::sync::{Arc, RwLock};

use actix::Addr;
use futures::TryStreamExt;
use lru::LruCache;
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, FindOptions, ReturnDocument},
    Collection,
};
use netsblox_cloud_common::{
    api::{self, FriendLinkState},
    FriendLink, Group, User,
};

use crate::{
    auth,
    errors::{InternalError, UserError},
    network::{
        self,
        topology::{GetOnlineUsers, TopologyActor},
    },
    utils,
};

pub(crate) struct FriendActions {
    friends: Collection<FriendLink>,
    friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,

    users: Collection<User>,
    groups: Collection<Group>,
    network: Addr<TopologyActor>,
}

impl FriendActions {
    pub(crate) fn new(
        friends: Collection<FriendLink>,
        friend_cache: Arc<RwLock<LruCache<String, Vec<String>>>>,

        users: Collection<User>,
        groups: Collection<Group>,
        network: Addr<TopologyActor>,
    ) -> Self {
        Self {
            friends,
            friend_cache,

            users,
            groups,
            network,
        }
    }

    pub(crate) async fn list_friends(
        &self,
        vu: &auth::users::ViewUser,
    ) -> Result<Vec<String>, UserError> {
        let friends = utils::get_friends(
            &self.users,
            &self.groups,
            &self.friends,
            self.friend_cache.clone(),
            &vu.username,
        )
        .await?;

        Ok(friends)
    }
    pub(crate) async fn list_online_friends(
        &self,
        vu: &auth::users::ViewUser,
    ) -> Result<Vec<String>, UserError> {
        let query = doc! {"username": &vu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let is_universal_friend = matches!(user.role, api::UserRole::Admin);
        let filter_usernames = if is_universal_friend {
            None
        } else {
            Some(self.list_friends(vu).await?)
        };

        let task = self
            .network
            .send(GetOnlineUsers(filter_usernames))
            .await
            .map_err(InternalError::ActixMessageError)?;
        let online_friends = task.run().await;

        Ok(online_friends)
    }

    pub(crate) async fn unfriend(
        &self,
        vu: &auth::users::EditUser,
        friend: &str,
    ) -> Result<(), UserError> {
        let query = doc! {
            "$or": [
                {"sender": &vu.username, "recipient": &friend, "state": FriendLinkState::APPROVED},
                {"sender": &friend, "recipient": &vu.username, "state": FriendLinkState::APPROVED}
            ]
        };
        self.friends
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::FriendNotFoundError)?;

        // invalidate friend cache
        let mut cache = self.friend_cache.write().unwrap();
        cache.pop(&vu.username);
        cache.pop(friend);

        Ok(())
    }

    pub(crate) async fn block(
        &self,
        eu: &auth::users::EditUser,
        other_user: &str,
    ) -> Result<api::FriendLink, UserError> {
        let query = doc! {
            "$or": [
                {"sender": &eu.username, "recipient": &other_user},
                {"sender": &other_user, "recipient": &eu.username}
            ]
        };
        let link = FriendLink::new(
            eu.username.to_owned(),
            other_user.to_owned(),
            Some(FriendLinkState::BLOCKED),
        );
        let update = doc! {
            "$set": {
                "state": &link.state,
                "updatedAt": &link.updated_at,
            },
            "$setOnInsert": {
                "createdAt": &link.created_at,
            },
        };
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .upsert(true)
            .build();

        let original = self
            .friends
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // invalidate friend cache
        if let Some(mut original) = original {
            let mut cache = self.friend_cache.write().unwrap();
            cache.pop(&eu.username);
            cache.pop(other_user);

            original.state = link.state;
            original.updated_at = link.updated_at;

            Ok(original.into())
        } else {
            Ok(link.into())
        }
    }

    pub(crate) async fn unblock(
        &self,
        eu: &auth::users::EditUser,
        other_user: &str,
    ) -> Result<(), UserError> {
        let query = doc! {
            "sender": &eu.username,
            "recipient": &other_user,
            "state": FriendLinkState::BLOCKED,
        };
        self.friends
            .delete_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        // No need to invalidate cache since it only caches the list of friend names
        Ok(())
    }

    pub async fn list_invites(
        &self,
        vu: &auth::users::ViewUser,
    ) -> Result<Vec<api::FriendInvite>, UserError> {
        let query = doc! {"recipient": &vu.username, "state": FriendLinkState::PENDING}; // TODO: ensure they are still pending
        let cursor = self
            .friends
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let invites: Vec<api::FriendInvite> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|link| link.into())
            .collect();

        Ok(invites)
    }

    pub async fn send_invite(
        &self,
        eu: &auth::users::EditUser,
        recipient: &str,
    ) -> Result<api::FriendLinkState, UserError> {
        // ensure users are valid
        let query = doc! {
            "$or": [
                {"username": &eu.username},
                {"username": &recipient},
            ]
        };
        let options = FindOptions::builder().limit(Some(2)).build();
        let users = self
            .users
            .find(query, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if users.len() != 2 {
            return Err(UserError::UserNotFoundError);
        }

        // block requests into a group
        if users.into_iter().any(|user| user.is_member()) {
            return Err(UserError::InviteNotAllowedError);
        }

        self.send_invite_unchecked(eu, &recipient).await
    }

    async fn send_invite_unchecked(
        &self,
        eu: &auth::users::EditUser,
        recipient: &str,
    ) -> Result<api::FriendLinkState, UserError> {
        let query = doc! {
            "sender": &recipient,
            "recipient": &eu.username,
            "state": FriendLinkState::PENDING
        };

        let update = doc! {"$set": {"state": FriendLinkState::APPROVED}};
        let approved_existing = self
            .friends
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .modified_count
            > 0;

        let state = if approved_existing {
            let mut cache = self.friend_cache.write().unwrap();
            cache.pop(&eu.username);
            cache.pop(recipient);

            // TODO: send msg about removing the existing invite

            FriendLinkState::APPROVED
        } else {
            let query = doc! {
                "$or": [
                    {"sender": &eu.username, "recipient": &recipient, "state": FriendLinkState::BLOCKED},
                    {"sender": &recipient, "recipient": &eu.username, "state": FriendLinkState::BLOCKED},
                    {"sender": &eu.username, "recipient": &recipient, "state": FriendLinkState::APPROVED},
                    {"sender": &recipient, "recipient": &eu.username, "state": FriendLinkState::APPROVED},
                ]
            };

            let link = FriendLink::new(eu.username.to_owned(), recipient.to_owned(), None);
            let update = doc! {"$setOnInsert": &link};
            let options = FindOneAndUpdateOptions::builder().upsert(true).build();
            let result = self
                .friends
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            if let Some(link) = result {
                // user is already blocked or approved
                link.state
            } else {
                // new friend link
                let request: api::FriendInvite = link.into();
                self.network
                    .send(network::topology::FriendRequestChangeMsg::new(
                        network::topology::ChangeType::Add,
                        request.clone(),
                    ))
                    .await
                    .map_err(InternalError::ActixMessageError)?;

                FriendLinkState::PENDING
            }
        };

        Ok(state)
    }

    pub(crate) async fn respond_to_invite(
        &self,
        eu: &auth::users::EditUser,
        sender: &str,
        resp: FriendLinkState,
    ) -> Result<FriendLink, UserError> {
        let query = doc! {
          "recipient": &eu.username,
          "sender": &sender,
          "state": FriendLinkState::PENDING
        };
        let update = doc! {"$set": {"state": &resp}};

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let link = self
            .friends
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::InviteNotFoundError)?;

        let friend_list_changed = matches!(resp, FriendLinkState::APPROVED);
        if friend_list_changed {
            // invalidate cache
            let mut cache = self.friend_cache.write().unwrap();
            cache.pop(sender);
            cache.pop(&eu.username);
        }

        let request: api::FriendInvite = link.clone().into();
        self.network
            .send(network::topology::FriendRequestChangeMsg::new(
                network::topology::ChangeType::Remove,
                request.clone(),
            ))
            .await
            .map_err(InternalError::ActixMessageError)?;

        Ok(link)
    }
}

// TODO: test that cache is invalidated on unfriend, block
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_data::*;
    use crate::{errors::UserError, test_utils};
    use netsblox_cloud_common::{api, User};

    #[actix_web::test]
    async fn test_respond_to_request() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(sender.username.clone(), rcvr.username.clone(), None);

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let actions: FriendActions = app_data.into();
                let auth_eu = auth::EditUser::test(rcvr.username.clone());
                let link = actions
                    .respond_to_invite(&auth_eu, &sender.username, api::FriendLinkState::APPROVED)
                    .await
                    .unwrap();

                assert!(matches!(link.state, api::FriendLinkState::APPROVED));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_404() {
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[rcvr.clone()])
            .run(|app_data| async move {
                let actions: FriendActions = app_data.into();
                let auth_eu = auth::EditUser::test(rcvr.username.clone());
                let result = actions
                    .respond_to_invite(&auth_eu, "sender", api::FriendLinkState::APPROVED)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_rejected() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(FriendLinkState::REJECTED),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let actions: FriendActions = app_data.into();
                let auth_eu = auth::EditUser::test(rcvr.username.clone());
                let result = actions
                    .respond_to_invite(&auth_eu, "sender", api::FriendLinkState::APPROVED)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_approved() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(api::FriendLinkState::APPROVED),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let actions: FriendActions = app_data.into();
                let auth_eu = auth::EditUser::test(rcvr.username.clone());
                let result = actions
                    .respond_to_invite(&auth_eu, "sender", api::FriendLinkState::APPROVED)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_respond_to_request_blocked() {
        let sender: User = api::NewUser {
            username: "sender".into(),
            email: "sender@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let rcvr: User = api::NewUser {
            username: "rcvr".into(),
            email: "rcvr@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let link = FriendLink::new(
            sender.username.clone(),
            rcvr.username.clone(),
            Some(api::FriendLinkState::BLOCKED),
        );

        test_utils::setup()
            .with_users(&[sender.clone(), rcvr.clone()])
            .with_friend_links(&[link])
            .run(|app_data| async move {
                let actions: FriendActions = app_data.into();
                let auth_eu = auth::EditUser::test(rcvr.username.clone());
                let result = actions
                    .respond_to_invite(&auth_eu, "sender", api::FriendLinkState::APPROVED)
                    .await;

                assert!(matches!(result, Err(UserError::InviteNotFoundError)));
            })
            .await;
    }
}
