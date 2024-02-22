use crate::utils;
use lettre::{
    message::{Mailbox, MultiPart},
    Address, Message, SmtpTransport,
};
use mongodb::{bson::doc, options::ReturnDocument, Collection};
use netsblox_cloud_common::api;
use netsblox_cloud_common::{MagicLink, User};
use nonempty::NonEmpty;

use crate::errors::{InternalError, UserError};

use super::email_template;

pub(crate) struct MagicLinkActions<'a> {
    links: &'a Collection<MagicLink>,
    users: &'a Collection<User>,

    // email support
    mailer: &'a SmtpTransport,
    sender: &'a Mailbox,
    public_url: &'a String,
}

impl<'a> MagicLinkActions<'a> {
    pub(crate) fn new(
        links: &'a Collection<MagicLink>,
        users: &'a Collection<User>,
        mailer: &'a SmtpTransport,
        sender: &'a Mailbox,
        public_url: &'a String,
    ) -> Self {
        Self {
            links,
            users,
            mailer,
            sender,
            public_url,
        }
    }

    /// Make a magic link for the email address and send it to the address.
    /// The link can be used to login as any user associated with the given address.
    pub(crate) async fn create_link(
        &self,
        data: &api::CreateMagicLinkData,
    ) -> Result<(), UserError> {
        let email = self.try_create_link(data).await?;
        utils::send_email(self.mailer, email)?;
        Ok(())
    }

    /// Try to create the given magic link running all the standard checks.
    async fn try_create_link(
        &self,
        data: &api::CreateMagicLinkData,
    ) -> Result<MagicLinkEmail, UserError> {
        let usernames: NonEmpty<String> = utils::find_usernames(self.users, &data.email).await?;

        let query = doc! {"email": &data.email};
        let link = MagicLink::new(data.email.clone());
        let update = doc! {"$setOnInsert": &link};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .upsert(true)
            .build();
        let existing = self
            .links
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if existing.is_some() {
            Err(UserError::MagicLinkSentError)
        } else {
            Ok(MagicLinkEmail {
                sender: self.sender.clone(),
                public_url: self.public_url.clone(),
                redirect_uri: data.redirect_uri.clone(),
                link,
                usernames,
            })
        }
    }

    pub(crate) async fn login(
        &self,
        username: &str,
        link_id: &api::MagicLinkId,
    ) -> Result<api::User, UserError> {
        let query = doc! {"id": &link_id};
        let link = self
            .links
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::MagicLinkNotFoundError)?;

        let query = doc! {"username": username, "email": &link.email};

        self.users
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)
            .map(|user| user.into())
    }
}

struct MagicLinkEmail {
    sender: Mailbox,
    link: MagicLink,
    usernames: NonEmpty<String>,
    public_url: String,
    redirect_uri: Option<String>,
}

impl MagicLinkEmail {
    fn render(&self) -> MultiPart {
        email_template::magic_link_email(
            &self.public_url,
            &self.usernames,
            &self.link.id,
            self.redirect_uri.clone(),
        )
    }
}

impl TryFrom<MagicLinkEmail> for lettre::Message {
    type Error = UserError;

    fn try_from(data: MagicLinkEmail) -> Result<Self, UserError> {
        let subject = "Magic sign-in link for NetsBlox";
        let body = data.render();
        let to_email = data.link.email;
        let message = Message::builder()
            .from(data.sender)
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
    use netsblox_cloud_common::{api, User};

    use super::*;

    #[actix_web::test]
    async fn test_try_create_link() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_magic_link_actions();

                let data = api::CreateMagicLinkData {
                    email: user.email.clone(),
                    redirect_uri: None,
                };
                actions.try_create_link(&data).await.unwrap();
                let query = doc! {"email": &user.email};
                let link_res = actions.links.find_one(query, None).await.unwrap();

                assert!(link_res.is_some());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_link_duplicate() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let l1 = MagicLink::new(user.email.clone());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_magic_links(&[l1])
            .run(|app_data| async move {
                let actions = app_data.as_magic_link_actions();

                let data = api::CreateMagicLinkData {
                    email: user.email.clone(),
                    redirect_uri: None,
                };
                let res = actions.create_link(&data).await;
                assert!(matches!(res, Err(UserError::MagicLinkSentError)))
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_link_email_not_found() {
        test_utils::setup()
            .run(|app_data| async move {
                let actions = app_data.as_magic_link_actions();

                let data = api::CreateMagicLinkData {
                    email: "IDon'tExist!".into(),
                    redirect_uri: None,
                };
                let res = actions.create_link(&data).await;
                assert!(matches!(res, Err(UserError::UserNotFoundError)));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let l1 = MagicLink::new(user.email.clone());

        test_utils::setup()
            .with_magic_links(&[l1.clone()])
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_magic_link_actions();

                let data = actions.login(&user.username, &l1.id).await.unwrap();
                assert_eq!(data.username, user.username);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_login_one_time_only() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let l1 = MagicLink::new(user.email.clone());

        test_utils::setup()
            .with_magic_links(&[l1.clone()])
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_magic_link_actions();

                let res1 = actions.login(&user.username, &l1.id).await;
                assert!(res1.is_ok());

                let res2 = actions.login(&user.username, &l1.id).await;
                assert!(res2.is_err(), "Should not allow more than one use.");
            })
            .await;
    }
}
