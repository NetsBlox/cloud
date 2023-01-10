use std::{collections::HashMap, time::SystemTime};

pub(crate) use crate::common::api::Credentials;
use crate::common::api::{self, UserRole};
use mongodb::bson::{doc, DateTime};
use reqwest::{Method, Response};
use serde::Deserialize;

use crate::{app_data::AppData, common::User, errors::UserError};

use super::sha512;

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

pub async fn login(app: &AppData, credentials: Credentials) -> Result<User, UserError> {
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
            let user_opt = app.find_user_where(query).await?;

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

                create_account(app, user_data.email, &account).await?
            };

            Ok(user)
        }
        Credentials::NetsBlox { username, password } => {
            let user = app
                .find_user(&username)
                .await?
                .ok_or(UserError::UserNotFoundError)?;

            let hash = sha512(&(password + &user.salt));
            if hash != user.hash {
                return Err(UserError::IncorrectPasswordError);
            }
            Ok(user)
        }
    }
}

async fn create_account(
    app: &AppData,
    email: String,
    account: &api::LinkedAccount,
) -> Result<User, UserError> {
    let username = username_from(app, account).await?;
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
        salt,
        email,
        group_id: None,
        created_at: DateTime::from_system_time(SystemTime::now()),
        linked_accounts: Vec::new(),
        role: UserRole::User,
        services_hosts: None,
        service_settings: HashMap::new(),
    };

    app.create_user(user.clone()).await?;
    Ok(user)
}

async fn username_from(
    app: &AppData,
    credentials: &api::LinkedAccount,
) -> Result<String, UserError> {
    let basename = &credentials.username;
    let existing_names = app.usernames_with_prefix(basename).await?;
    if existing_names.contains(basename) {
        let strategy: String = credentials
            .strategy
            .to_ascii_lowercase()
            .chars()
            .filter(|l| l.is_alphabetic())
            .collect();
        let mut username = format!("{}_{}", &basename, &strategy);
        let mut count = 2;

        while existing_names.contains(&username) {
            username = format!("{}_{}", basename, count);
            count += 1;
        }
        Ok(username)
    } else {
        Ok(basename.to_owned())
    }
}
