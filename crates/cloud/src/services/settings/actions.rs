use std::collections::HashMap;

use futures::TryStreamExt;
use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{api, Group, User};

use crate::{
    auth,
    errors::{InternalError, UserError},
};

pub(crate) struct SettingsActions {
    users: Collection<User>,
    groups: Collection<Group>,
}

impl SettingsActions {
    pub(crate) fn new(users: Collection<User>, groups: Collection<Group>) -> Self {
        Self { users, groups }
    }
    pub(crate) async fn get_settings(
        &self,
        vu: &auth::ViewUser,
        host: &str,
    ) -> Result<api::ServiceSettings, UserError> {
        let query = doc! {"username": &vu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let query = match user.group_id {
            Some(ref group_id) => doc! {"$or": [
                {"owner": &vu.username},
                {"id": group_id}
            ]},
            None => doc! {"owner": &vu.username},
        };
        let cursor = self
            .groups
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let mut groups: Vec<_> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let member_settings = user
            .group_id
            .and_then(|group_id| groups.iter().position(|group| group.id == group_id))
            .map(|pos| groups.swap_remove(pos))
            .and_then(|group| group.service_settings.get(host).map(|s| s.to_owned()));

        let all_settings = api::ServiceSettings {
            user: user.service_settings.get(host).cloned(),
            member: member_settings,
            groups: groups
                .into_iter()
                .filter_map(|group| {
                    group
                        .service_settings
                        .get(host)
                        .map(|s| (group.id, s.to_owned()))
                })
                .collect::<HashMap<_, _>>(),
        };

        Ok(all_settings)
    }
}
