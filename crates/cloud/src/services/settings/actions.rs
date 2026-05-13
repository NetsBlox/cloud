use std::collections::HashMap;

use crate::auth;
use crate::errors::{InternalError, UserError};
use crate::utils;
use mongodb::bson;
use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{api, Group, User};

pub(crate) struct SettingsActions<'a> {
    users: &'a Collection<User>,
    groups: &'a Collection<Group>,
}

impl<'a> SettingsActions<'a> {
    pub(crate) fn new(users: &'a Collection<User>, groups: &'a Collection<Group>) -> Self {
        Self { users, groups }
    }

    pub(crate) async fn get_all_settings(
        &self,
        vs: &auth::ViewSettings,
        host: &api::ServiceHostId,
    ) -> Result<api::AllServiceSettings, UserError> {
        let query = doc! {"username": &vs.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let mut user_settings = user
            .service_settings
            .unwrap_or_default()
            .get(&host)
            .cloned();

        let mut member_settings = if let Some(group_id) = user.group_id {
            let query = doc! {"id": group_id};
            let settings = self
                .groups
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::UserNotFoundError)?
                .service_settings
                .unwrap_or_default()
                .get(host)
                .cloned();
            settings
        } else {
            None
        };

        if let Some(user_settings_ref) = user_settings.as_mut() {
            if Some(host) != vs.requesting_host.as_ref() {
                utils::redact_setting_secrets(user_settings_ref);
            }
        }

        if let Some(member_settings_ref) = member_settings.as_mut() {
            if Some(host) != vs.requesting_host.as_ref() {
                utils::redact_setting_secrets(member_settings_ref);
            }
        }

        let all_settings = api::AllServiceSettings {
            user: user_settings,
            member: member_settings,
        };

        Ok(all_settings)
    }

    pub(crate) async fn update_user_settings(
        &self,
        us: &auth::UpdateSettings,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &us.username};
        let host = &us.host;

        let mut update_doc = bson::Document::new();
        for (service_name, service_settings) in us.update.inner() {
            for (setting_name, value) in service_settings {
                let key = format!("serviceSettings.{host}.{service_name}.{setting_name}");
                update_doc.insert(key, value);
            }
        }
        let update = doc! {"$set": update_doc};
        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }

    pub(crate) async fn get_user_settings(
        &self,
        vs: &auth::ViewSettings,
    ) -> Result<HashMap<api::ServiceHostId, api::ServiceHostSettings>, UserError> {
        let query = doc! {"username": &vs.username};
        let mut settings = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?
            .service_settings
            .unwrap_or_default();

        settings
            .iter_mut()
            .filter(|&(key, _)| Some(key) != vs.requesting_host.as_ref())
            .for_each(|(_, host_settings)| utils::redact_setting_secrets(host_settings));

        Ok(settings)
    }

    pub(crate) async fn get_user_host_settings(
        &self,
        vs: &auth::ViewSettings,
        host: &api::ServiceHostId,
    ) -> Result<api::ServiceHostSettings, UserError> {
        let query = doc! {"username": &vs.username};
        let mut host_settings = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?
            .service_settings
            .unwrap_or_default()
            .remove(host)
            .unwrap_or_default();

        if Some(host) != vs.requesting_host.as_ref() {
            utils::redact_setting_secrets(&mut host_settings);
        }

        Ok(host_settings)
    }

    pub(crate) async fn delete_user_settings(
        &self,
        ds: &auth::DeleteSettings,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &ds.username};
        let key = format!("serviceSettings.{}", ds.host);
        let update = doc! {"$unset": { key: true }};

        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }

    pub(crate) async fn delete_user_service_settings(
        &self,
        ds: &auth::DeleteSettings,
        service_name: &api::ServiceName,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &ds.username};
        let key = format!("serviceSettings.{}.{}", ds.host, service_name);
        let update = doc! {"$unset": {key: true}};

        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }

    pub(crate) async fn delete_user_service_setting(
        &self,
        ds: &auth::DeleteSettings,
        service_name: &api::ServiceName,
        setting_name: &api::SettingName,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &ds.username};
        let key = format!(
            "serviceSettings.{}.{}.{}",
            ds.host, service_name, setting_name
        );
        let update = doc! {"$unset": {key: true}};

        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }

    pub(crate) async fn get_group_settings(
        &self,
        vgs: &auth::ViewGroupSettings,
    ) -> Result<HashMap<api::ServiceHostId, api::ServiceHostSettings>, UserError> {
        let query = doc! {"id": &vgs.id};
        let mut settings = self
            .groups
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?
            .service_settings
            .unwrap_or_default();

        settings
            .iter_mut()
            .filter(|&(key, _)| Some(key) != vgs.requesting_host.as_ref())
            .for_each(|(_, host_settings)| utils::redact_setting_secrets(host_settings));

        Ok(settings)
    }

    pub(crate) async fn get_group_host_settings(
        &self,
        vgs: &auth::ViewGroupSettings,
        host: &api::ServiceHostId,
    ) -> Result<api::ServiceHostSettings, UserError> {
        let query = doc! {"id": &vgs.id};
        let mut host_settings = self
            .groups
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?
            .service_settings
            .unwrap_or_default()
            .remove(host)
            .unwrap_or_default();

        if Some(host) != vgs.requesting_host.as_ref() {
            utils::redact_setting_secrets(&mut host_settings);
        }

        Ok(host_settings)
    }

    pub(crate) async fn update_group_settings(
        &self,
        ugs: &auth::UpdateGroupSettings,
    ) -> Result<(), UserError> {
        let query = doc! {"id": &ugs.id};
        let host = &ugs.host;

        let mut update_doc = bson::Document::new();
        for (service_name, service_settings) in ugs.update.inner() {
            for (setting_name, value) in service_settings {
                let key = format!("serviceSettings.{host}.{service_name}.{setting_name}");
                update_doc.insert(key, value);
            }
        }
        let update = doc! {"$set": update_doc};

        let result = self
            .groups
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Ok(())
        } else {
            Err(UserError::GroupNotFoundError)?
        }
    }

    pub(crate) async fn delete_group_settings(
        &self,
        dgs: &auth::DeleteGroupSettings,
    ) -> Result<(), UserError> {
        let query = doc! {"id": &dgs.id};
        let host = &dgs.host;

        let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};

        let result = self
            .groups
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Ok(())
        } else {
            Err(UserError::GroupNotFoundError)?
        }
    }

    pub(crate) async fn delete_group_service_settings(
        &self,
        dgs: &auth::DeleteGroupSettings,
        service_name: &api::ServiceName,
    ) -> Result<(), UserError> {
        let query = doc! {"id": &dgs.id};
        let host = &dgs.host;
        let update = doc! {"$unset": {format!("serviceSettings.{host}.{service_name}"): true}};
        let result = self
            .groups
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Ok(())
        } else {
            Err(UserError::GroupNotFoundError)
        }
    }

    pub(crate) async fn delete_group_service_setting(
        &self,
        dgs: &auth::DeleteGroupSettings,
        service_name: &api::ServiceName,
        setting_name: &api::SettingName,
    ) -> Result<(), UserError> {
        let query = doc! {"id": &dgs.id};
        let host = &dgs.host;
        let update = doc! {"$unset": {format!("serviceSettings.{host}.{service_name}.{setting_name}"): true}};
        let result = self
            .groups
            .update_one(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Ok(())
        } else {
            Err(UserError::GroupNotFoundError)
        }
    }
}
