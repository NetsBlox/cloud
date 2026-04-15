use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{api, Group, User};

use crate::{
    auth,
    errors::{InternalError, UserError},
};

pub(crate) struct SettingsActions<'a> {
    users: &'a Collection<User>,
    groups: &'a Collection<Group>,
}

impl<'a> SettingsActions<'a> {
    pub(crate) fn new(users: &'a Collection<User>, groups: &'a Collection<Group>) -> Self {
        Self { users, groups }
    }
    pub(crate) async fn get_settings(
        &self,
        vu: &auth::ViewUser,
        host: &api::ServiceHostId,
    ) -> Result<api::AllServiceSettings, UserError> {
        let query = doc! {"username": &vu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let user_settings = user
            .service_settings
            .get(&host)
            .cloned();

        let member_settings = if let Some(group_id) = user.group_id {
            let query = doc! {"id": group_id};
            let group = self
                .groups
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::UserNotFoundError)?;

            group
                .service_settings
                .get(&api::ServiceHostId::from(host.to_string()))
                .cloned()
        } else {
            None
        };

        let all_settings = api::AllServiceSettings {
            user: user_settings,
            member: member_settings,
        };

        Ok(all_settings)
    }
}
