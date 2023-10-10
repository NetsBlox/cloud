use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::auth;
use actix::Addr;
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lettre::{
    message::{Mailbox, MultiPart},
    Address, Message, SmtpTransport,
};
use lru::LruCache;
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Collection,
};
use netsblox_cloud_common::{api, BannedAccount, ProjectMetadata, SetPasswordToken, User};
use regex::Regex;
use rustrict::CensorStr;

use crate::{
    app_data::metrics,
    errors::{InternalError, UserError},
    network::topology::{self, TopologyActor},
    utils,
};

use super::{email_template, strategies};

pub(crate) struct UserActions<'a> {
    users: &'a Collection<User>,
    banned_accounts: &'a Collection<BannedAccount>,
    password_tokens: &'a Collection<SetPasswordToken>,
    metrics: &'a metrics::Metrics,

    project_metadata: &'a Collection<ProjectMetadata>,
    project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    network: &'a Addr<TopologyActor>,

    friend_cache: &'a Arc<RwLock<LruCache<String, Vec<String>>>>,

    // email support
    mailer: &'a SmtpTransport,
    sender: &'a Mailbox,
    public_url: &'a String,
}

/// A struct for passing data to the constructor of `UserActions` w/o either 1) making
/// all fields public on UserActions or 2) having *way* too many arguments
pub(crate) struct UserActionData<'a> {
    pub(crate) users: &'a Collection<User>,
    pub(crate) banned_accounts: &'a Collection<BannedAccount>,
    pub(crate) password_tokens: &'a Collection<SetPasswordToken>,
    pub(crate) metrics: &'a metrics::Metrics,

    pub(crate) project_metadata: &'a Collection<ProjectMetadata>,
    pub(crate) project_cache: &'a Arc<RwLock<LruCache<api::ProjectId, ProjectMetadata>>>,
    pub(crate) network: &'a Addr<TopologyActor>,

    pub(crate) friend_cache: &'a Arc<RwLock<LruCache<String, Vec<String>>>>,

    // email support
    pub(crate) mailer: &'a SmtpTransport,
    pub(crate) sender: &'a Mailbox,
    pub(crate) public_url: &'a String,
}

impl<'a> UserActions<'a> {
    pub(crate) fn new(data: UserActionData<'a>) -> Self {
        UserActions {
            users: data.users,
            banned_accounts: data.banned_accounts,
            password_tokens: data.password_tokens,
            metrics: data.metrics,

            project_cache: data.project_cache,
            project_metadata: data.project_metadata,
            network: data.network,

            friend_cache: data.friend_cache,

            mailer: data.mailer,
            sender: data.sender,
            public_url: data.public_url,
        }
    }

    pub(crate) async fn create_user(&self, cu: auth::CreateUser) -> Result<api::User, UserError> {
        ensure_valid_email(&cu.data.email)?;
        let user: User = cu.data.into();
        ensure_valid_username(&user.username)?;

        let query = doc! {"email": &user.email};
        if let Some(_account) = self
            .banned_accounts
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            return Err(UserError::InvalidEmailAddress);
        }

        let query = doc! {"username": &user.username};
        let update = doc! {"$setOnInsert": &user};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .upsert(true)
            .build();
        let existing_user = self
            .users
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if existing_user.is_some() {
            Err(UserError::UserExistsError)
        } else {
            if let Some(group_id) = user.group_id.clone() {
                utils::group_members_updated(self.users, self.friend_cache.clone(), &group_id)
                    .await;
            }
            self.metrics.record_signup();
            let user: api::User = user.into();
            Ok(user)
        }
    }

    pub(crate) async fn get_user(&self, vu: &auth::ViewUser) -> Result<api::User, UserError> {
        let query = doc! {"username": &vu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.into())
    }

    pub(crate) async fn delete_user(&self, eu: &auth::EditUser) -> Result<api::User, UserError> {
        let query = doc! {"username": &eu.username};
        let user = self
            .users
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        if let Some(group_id) = user.group_id.as_ref() {
            utils::group_members_updated(self.users, self.friend_cache.clone(), group_id).await;
        }

        Ok(user.into())
    }

    pub(crate) async fn login(&self, request: api::LoginRequest) -> Result<api::User, UserError> {
        let client_id = request.client_id.clone();
        let user = strategies::login(self.users, request.credentials).await?;

        let query = doc! {"$or": [
            {"username": &user.username},
            {"email": &user.email},
        ]};

        if let Some(_account) = self
            .banned_accounts
            .find_one(query.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            return Err(UserError::BannedUserError);
        }

        if let Some(client_id) = client_id {
            self.update_ownership(&client_id, &user.username).await?;
            self.network.do_send(topology::SetClientUsername {
                id: client_id,
                username: Some(user.username.clone()),
            });
        }
        self.metrics.record_login();
        Ok(user.into())
    }

    pub(crate) fn logout(&self, client_id: &api::ClientId) {
        self.network.do_send(topology::SetClientUsername {
            id: client_id.clone(),
            username: None,
        });
    }

    pub(crate) async fn reset_password(&self, eu: &auth::EditUser) -> Result<(), UserError> {
        let user = self
            .users
            .find_one(doc! {"username": &eu.username}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let token = SetPasswordToken::new(eu.username.clone());

        let update = doc! {"$setOnInsert": &token};
        let query = doc! {"username": &eu.username};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let result = self
            .password_tokens
            .update_one(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.upserted_id.is_none() {
            return Err(UserError::PasswordResetLinkSentError);
        }

        let email = SetPasswordEmail {
            sender: self.sender.clone(),
            public_url: self.public_url.clone(),
            user,
            token,
        };

        utils::send_email(self.mailer, email)?;

        Ok(())
    }

    pub(crate) async fn set_password(
        &self,
        sp: &auth::SetPassword,
        password: String,
    ) -> Result<api::User, UserError> {
        let query = doc! {"username": &sp.username};
        let user = self
            .users
            .find_one(query.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let update = doc! {
            "$set": {
                "hash": utils::sha512(&(password + &user.salt.unwrap_or_default()))
            }
        };
        let user = self
            .users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.into())
    }

    pub(crate) async fn set_hosts(
        &self,
        eu: &auth::EditUser,
        hosts: &[api::ServiceHost],
    ) -> Result<api::User, UserError> {
        let query = doc! {"username": &eu.username};
        let update = doc! {"$set": {"servicesHosts": &hosts}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();
        let user = self
            .users
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.into())
    }

    pub(crate) async fn ban_user(
        &self,
        bu: &auth::BanUser,
    ) -> Result<api::BannedAccount, UserError> {
        let query = doc! {"username": &bu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        let query = doc! {"username": &user.username};
        let account = BannedAccount::new(user.username, user.email);
        let update = doc! {"$setOnInsert": &account};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .upsert(true)
            .build();

        self.banned_accounts
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(account.into())
    }

    pub(crate) async fn unban_user(
        &self,
        bu: &auth::BanUser,
    ) -> Result<api::BannedAccount, UserError> {
        let query = doc! {"username": &bu.username};
        let account = self
            .banned_accounts
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(account.into())
    }

    pub(crate) async fn link_account(
        &self,
        eu: &auth::EditUser,
        creds: strategies::Credentials,
    ) -> Result<api::User, UserError> {
        if let strategies::Credentials::NetsBlox { .. } = creds {
            return Err(UserError::InvalidAccountTypeError);
        };

        strategies::authenticate(&creds).await?;

        let account: api::LinkedAccount = creds.into();
        let query = doc! {"linkedAccounts": {"$elemMatch": &account}};
        let existing = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if existing.is_some() {
            return Err(UserError::AccountAlreadyLinkedError);
        }

        let query = doc! {"username": &eu.username};
        let update = doc! {"$push": {"linkedAccounts": &account}};
        let user = self
            .users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.into())
    }

    pub(crate) async fn unlink_account(
        &self,
        eu: &auth::EditUser,
        account: api::LinkedAccount,
    ) -> Result<api::User, UserError> {
        let query = doc! {"username": &eu.username};
        let update = doc! {"$pull": {"linkedAccounts": &account}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let user = self
            .users
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.into())
    }

    pub(crate) async fn list_users(&self, _lu: &auth::ListUsers) -> Result<Vec<String>, UserError> {
        let query = doc! {};
        let cursor = self
            .users
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let usernames: Vec<String> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|user| user.username)
            .collect();

        Ok(usernames)
    }

    pub(crate) async fn set_user_settings(
        &self,
        lu: &auth::EditUser,
        host: &str,
        settings: &str,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &lu.username};
        let update = doc! {"$set": {format!("serviceSettings.{}", &host): settings}};
        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }

    pub(crate) async fn get_service_settings(
        &self,
        vu: &auth::ViewUser,
    ) -> Result<HashMap<String, String>, UserError> {
        let query = doc! {"username": &vu.username};
        let user = self
            .users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(user.service_settings)
    }

    pub(crate) async fn delete_user_settings(
        &self,
        lu: &auth::EditUser,
        host: &str,
    ) -> Result<(), UserError> {
        let query = doc! {"username": &lu.username};
        let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};

        self.users
            .find_one_and_update(query, update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(())
    }
    async fn update_ownership(
        &self,
        client_id: &api::ClientId,
        username: &str,
    ) -> Result<(), UserError> {
        // Update ownership of current project
        if !client_id.as_str().starts_with('_') {
            return Err(UserError::InvalidClientIdError);
        }

        let query = doc! {"owner": client_id.as_str()};
        if let Some(metadata) = self
            .project_metadata
            .find_one(query.clone(), None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
        {
            // No project will be found for non-NetsBlox clients such as PyBlox
            let name =
                utils::get_valid_project_name(self.project_metadata, username, &metadata.name)
                    .await?;
            let update = doc! {"$set": {"owner": username, "name": name}};
            let options = mongodb::options::FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();
            let new_metadata = self
                .project_metadata
                .find_one_and_update(query, update, Some(options))
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::ProjectNotFoundError)?;

            utils::on_room_changed(self.network, self.project_cache, new_metadata);
        }
        Ok(())
    }
}

fn ensure_valid_email(email: &str) -> Result<(), UserError> {
    email
        .parse::<Address>()
        .map_err(|_err| UserError::InvalidEmailAddress)?;

    Ok(())
}
fn ensure_valid_username(name: &str) -> Result<(), UserError> {
    if !is_valid_username(name) {
        Err(UserError::InvalidUsername)
    } else {
        Ok(())
    }
}

fn is_valid_username(name: &str) -> bool {
    let max_len = 25;
    let min_len = 3;
    let char_count = name.chars().count();
    lazy_static! {
        static ref USERNAME_REGEX: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_\-]+$").unwrap();
    }

    char_count > min_len
        && char_count < max_len
        && USERNAME_REGEX.is_match(name)
        && !name.is_inappropriate()
}

pub(crate) struct SetPasswordEmail {
    sender: Mailbox,
    user: User,
    token: SetPasswordToken,
    public_url: String,
}

impl SetPasswordEmail {
    fn render(&self) -> MultiPart {
        let url = format!(
            "{}/users/{}/password?token={}",
            self.public_url, &self.user.username, &self.token.secret
        );
        email_template::set_password_email(&self.user.username, &url)
    }
}

impl TryFrom<SetPasswordEmail> for lettre::Message {
    type Error = UserError;

    fn try_from(email: SetPasswordEmail) -> Result<Self, UserError> {
        let subject = "Password Reset Request";
        let body = email.render();
        let to_email = email.user.email;
        let message = Message::builder()
            .from(email.sender)
            .to(Mailbox::new(
                None,
                to_email
                    .parse::<Address>()
                    .map_err(|_err| UserError::InvalidEmailAddress)?,
            ))
            .subject(subject.to_string())
            .date_now()
            .multipart(body)
            .map_err(|_err| InternalError::EmailBuildError)?;

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;

    #[actix_web::test]
    async fn test_ban_idempotent() {
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
                let actions = app_data.as_user_actions();

                let auth_bu = auth::BanUser::test(user.username.clone());
                actions.ban_user(&auth_bu).await.unwrap();
                actions.ban_user(&auth_bu).await.unwrap();

                actions.unban_user(&auth_bu).await.unwrap();
                // Check that the user is not banned
                let query = doc! {"username": &auth_bu.username};
                let account = actions.banned_accounts.find_one(query, None).await.unwrap();
                assert!(
                    account.is_none(),
                    "Double ban wasn't undone by single unban."
                );
            })
            .await;
    }

    #[actix_web::test]
    async fn test_is_valid_username_caps() {
        assert!(is_valid_username("HelloWorld"));
    }

    #[actix_web::test]
    async fn test_is_valid_username() {
        assert!(is_valid_username("hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_leading_underscore() {
        assert!(!is_valid_username("_hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_leading_dash() {
        assert!(!is_valid_username("-hello"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_at_symbol() {
        assert!(!is_valid_username("hello@gmail.com"));
    }

    #[actix_web::test]
    async fn test_is_valid_username_vulgar() {
        assert!(!is_valid_username("shit"));
    }
}
