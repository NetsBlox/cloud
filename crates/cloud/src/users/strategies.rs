use std::{collections::HashMap, time::SystemTime};

pub(crate) use crate::common::api::Credentials;
use crate::{
    common::api::{self, UserRole},
    utils,
};
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime},
    options::{FindOneAndUpdateOptions, ReturnDocument, UpdateOptions},
    Collection,
};
use reqwest::{Method, Response};
use serde::Deserialize;

use crate::utils::sha512;
use crate::{
    common::User,
    errors::{InternalError, UserError},
};

#[allow(dead_code)]
#[derive(Deserialize)]
struct SnapUser {
    id: u32,
    created: String,
    email: String,
    role: String,
    username: String,
    project_count: u32,
    verified: bool,
}

pub async fn authenticate(credentials: &Credentials) -> Result<Option<Response>, UserError> {
    match credentials {
        Credentials::Snap { username, password } => {
            let url = &format!("https://snap.berkeley.edu/api/v1/users/{}/login", username,);
            let client = reqwest::Client::new();
            let pwd_hash = sha512(password);
            let response = client
                .request(Method::POST, url)
                .body(pwd_hash)
                .send()
                .await
                .map_err(|_err| UserError::SnapConnectionError)?;

            if response.status().as_u16() > 399 {
                return Err(UserError::IncorrectUsernameOrPasswordError);
            }
            Ok(Some(response))
        }
        Credentials::NetsBlox { .. } => Ok(None),
    }
}

pub async fn login(users: &Collection<User>, credentials: Credentials) -> Result<User, UserError> {
    match credentials {
        Credentials::Snap { ref username, .. } => {
            let response = authenticate(&credentials)
                .await?
                .ok_or(UserError::SnapConnectionError)?;

            let account = api::LinkedAccount {
                username: username.to_lowercase(),
                strategy: "snap".to_owned(),
            };

            let query = doc! {"linkedAccounts": {"$elemMatch": &account}};
            let user_opt = users
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;

            let user = if let Some(user) = user_opt {
                user
            } else {
                let cookie = response
                    .cookies()
                    .next()
                    .ok_or(UserError::SnapConnectionError)?;
                let url = &format!("https://snap.berkeley.edu/api/v1/users/{}", username);

                let client = reqwest::Client::new();
                let user_data = client
                    .request(Method::GET, url)
                    .header("Cookie", format!("{}={}", cookie.name(), cookie.value()))
                    .send()
                    .await
                    .map_err(|_err| UserError::SnapConnectionError)?
                    .json::<SnapUser>()
                    .await
                    .map_err(|_err| UserError::SnapConnectionError)?;

                // TODO: ensure email isn't banned?

                create_account(users, user_data.email, &account).await?
            };

            Ok(user)
        }
        Credentials::NetsBlox { username, password } => {
            let query = doc! {"username": &username};
            let user = users
                .find_one(query.clone(), None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::UserNotFoundError)?;

            let needs_update = user.salt.is_none();
            let salt = user.salt.clone().unwrap_or_default();
            let hash = sha512(&(password.clone() + &salt));
            if hash != user.hash {
                return Err(UserError::IncorrectPasswordError);
            }

            // Ensure they have a salt (empty until first login for migrated accounts)
            let user = if needs_update {
                update_salt(users, username.as_str(), password).await?
            } else {
                user
            };

            Ok(user)
        }
    }
}

async fn update_salt(
    users: &Collection<User>,
    username: &str,
    password: String,
) -> Result<User, UserError> {
    let query = doc! {"username": &username};
    let salt = passwords::PasswordGenerator::new()
        .length(8)
        .exclude_similar_characters(true)
        .numbers(true)
        .spaces(false)
        .generate_one()
        .unwrap_or_else(|_err| "salt".to_owned());

    let hash = sha512(&(password + &salt));

    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let update = doc! {
        "$set": {
            "salt": &salt,
            "hash": hash
        }
    };

    users
        .find_one_and_update(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::UserNotFoundError)
}

async fn create_account(
    users: &Collection<User>,
    email: String,
    account: &api::LinkedAccount,
) -> Result<User, UserError> {
    let username = username_from(users, account).await?;
    let query = doc! {"username": &username};
    let salt = passwords::PasswordGenerator::new()
        .length(8)
        .exclude_similar_characters(true)
        .numbers(true)
        .spaces(false)
        .generate_one()
        .unwrap_or("salt".to_owned());

    let hash: String = "None".to_owned();
    let user = User {
        // TODO: impl From instead?
        username,
        hash,
        salt: Some(salt),
        email,
        group_id: None,
        created_at: DateTime::from_system_time(SystemTime::now()),
        linked_accounts: Vec::new(),
        role: UserRole::User,
        services_hosts: None,
        service_settings: HashMap::new(),
    };

    let update = doc!("$setOnInsert": &user);
    let options = UpdateOptions::builder().upsert(true).build();
    users
        .update_one(query, update, options)
        .await
        .map_err(InternalError::DatabaseConnectionError)?;

    Ok(user)
}

async fn username_from(
    users: &Collection<User>,
    credentials: &api::LinkedAccount,
) -> Result<String, UserError> {
    let basename = credentials.username.to_owned();
    let starts_with_name = mongodb::bson::Regex {
        pattern: format!("^{}", &basename),
        options: String::new(),
    };
    let query = doc! {"username": {"$regex": starts_with_name}};
    // TODO: this could be optimized to map on the stream...
    let existing_names = users
        .find(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .into_iter()
        .map(|user| user.username)
        .collect::<Vec<_>>();

    get_unique_username(existing_names.iter().map(|n| n.as_str()), credentials)
}

fn get_unique_username<'a>(
    names: impl Iterator<Item = &'a str>,
    credentials: &api::LinkedAccount,
) -> Result<String, UserError> {
    let strategy: String = credentials
        .strategy
        .to_ascii_lowercase()
        .chars()
        .filter(|l| l.is_alphabetic())
        .collect();

    let candidates = std::iter::once(credentials.username.clone())
        .chain(std::iter::once_with(|| {
            format!("{}_{}", &credentials.username, &strategy)
        }))
        .chain((2..1000).map(|n| format!("{}_{}{}", &credentials.username, &strategy, n)));

    utils::find_first_unique(names, candidates).ok_or(UserError::UsernameExists)
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;

    #[actix_web::test]
    async fn test_login_update_salt() {
        let password: String = "somePassword...".into();
        let mut user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();
        user.salt = None;
        user.hash = sha512(&password);

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                // check initial login
                let credentials = Credentials::NetsBlox {
                    username: api::Username::new(&user.username),
                    password,
                };
                login(&app_data.users, credentials.clone()).await.unwrap();

                // check that the salt has been set
                let query = doc! {"username": &user.username};
                let user = app_data.users.find_one(query, None).await.unwrap().unwrap();
                assert!(user.salt.is_some());

                // check that we can login again
                login(&app_data.users, credentials).await.unwrap();
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_dont_update_salt_failed_login() {
        let password: String = "somePassword...".into();
        let mut user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();
        user.salt = None;
        user.hash = sha512(&password);

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                // check initial login
                let credentials = Credentials::NetsBlox {
                    username: api::Username::new(user.username),
                    password: "badPassword".into(),
                };
                let result = login(&app_data.users, credentials.clone()).await;
                assert!(result.is_err());

                // salt should still be none
                assert!(user.salt.is_none());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_dont_update_existing_salt() {
        let password: String = "somePassword...".into();
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: Some(password.clone()),
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                // check initial login
                let credentials = Credentials::NetsBlox {
                    username: api::Username::new(&user.username),
                    password,
                };
                login(&app_data.users, credentials.clone()).await.unwrap();

                // check that the salt has been set
                let query = doc! {"username": &user.username};
                let updated_user = app_data.users.find_one(query, None).await.unwrap().unwrap();
                assert_eq!(user.salt.unwrap(), updated_user.salt.unwrap());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_get_unique_username_strat_suffix() {
        let creds = api::LinkedAccount {
            username: "brian".into(),
            strategy: "snap".into(),
        };
        let names = ["brian"];
        let name = get_unique_username(names.into_iter(), &creds).unwrap();
        assert_eq!(name.as_str(), "brian_snap");
    }

    #[actix_web::test]
    async fn test_get_unique_username_strat_suffix_inc() {
        let creds = api::LinkedAccount {
            username: "brian".into(),
            strategy: "snap".into(),
        };
        let names = ["brian", "brian_snap", "brian_snap2"];
        let name = get_unique_username(names.into_iter(), &creds).unwrap();
        assert_eq!(name.as_str(), "brian_snap3");
    }

    #[actix_web::test]
    async fn test_get_unique_username_none() {
        let creds = api::LinkedAccount {
            username: "brian".into(),
            strategy: "snap".into(),
        };
        let existing: Vec<_> = std::iter::once("brian".to_string())
            .chain(std::iter::once("brian_snap".to_string()))
            .chain((2..10000).map(|n| format!("brian_snap{}", n)))
            .collect();
        let name_res = get_unique_username(existing.iter().map(|n| n.as_str()), &creds);

        assert!(matches!(name_res, Err(UserError::UsernameExists)));
    }
}
