use std::time::SystemTime;

use futures::TryStreamExt;
use mongodb::{bson::doc, options::ReturnDocument, Collection};
use netsblox_cloud_common::{api::oauth, OAuthClient, OAuthToken};
use passwords::PasswordGenerator;
use uuid::Uuid;

use crate::{
    auth,
    errors::{InternalError, OAuthFlowError, UserError},
    utils::sha512,
};

use super::{
    html_template,
    routes::{AuthorizeClientParams, Scope},
};

pub(crate) struct OAuthActions {
    clients: Collection<OAuthClient>,
    tokens: Collection<OAuthToken>,
    codes: Collection<oauth::Code>,
}

impl OAuthActions {
    pub(crate) fn new(
        clients: Collection<OAuthClient>,
        tokens: Collection<OAuthToken>,
        codes: Collection<oauth::Code>,
    ) -> Self {
        Self {
            clients,
            tokens,
            codes,
        }
    }

    pub(crate) async fn render_auth_page(
        &self,
        eu: &auth::EditUser,
        client_id: &oauth::ClientId,
    ) -> Result<String, UserError> {
        let query = doc! {"id": &client_id};
        let client = self
            .clients
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::OAuthClientNotFoundError)?;

        // FIXME: the requestor should pass the scopes
        let scopes = [Scope::ViewAlexaSkills, Scope::ExecuteBlocks];
        Ok(html_template::authorize_page(
            &eu.username,
            &client.name,
            &scopes,
        ))
    }

    pub(crate) async fn authorize(
        &self,
        eu: &auth::EditUser,
        params: &AuthorizeClientParams,
    ) -> Result<String, UserError> {
        let redirect_uri = params
            .redirect_uri
            .as_ref()
            .ok_or(OAuthFlowError::InvalidRedirectUrlError)?;
        // TODO: Make sure the uri error is correct

        let url = if let Some(error) = &params.error {
            params
                .error_description
                .as_ref()
                .map(|desc| {
                    format!(
                        "{}?error={}&error_description={}",
                        redirect_uri, error, desc
                    )
                })
                .unwrap_or_else(|| format!("{}?error={}", redirect_uri, error))
        } else {
            // Check that the client exists
            // FIXME: this needs to use the salt, too!
            let hashed_secret = sha512(&params.client_secret);
            let query = doc! {
                "id": &params.client_id,
                "hash": hashed_secret
            };

            let client_exists = self
                .clients
                .find_one(query, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .is_some();

            // TODO: is incorrect secret a different error?
            if !client_exists {
                return Err(UserError::OAuthClientNotFoundError);
            }

            // create a new code for the user
            let code = oauth::Code {
                id: oauth::CodeId::new(Uuid::new_v4().to_string()),
                username: eu.username.clone(),
                client_id: params.client_id.to_owned(),
                redirect_uri: redirect_uri.to_owned(),
                created_at: SystemTime::now(),
            };

            self.codes
                .insert_one(&code, None)
                .await
                // FIXME: we may need to handle these errors differently
                .map_err(InternalError::DatabaseConnectionError)?;

            format!(
                "{}?code={}&state={}",
                redirect_uri,
                code.id.as_str(),
                params.state
            )
        };

        Ok(url)
    }

    pub(crate) async fn create_client(
        &self,
        _cc: &auth::ManageClient,
        name: &str,
    ) -> Result<oauth::CreatedClientData, UserError> {
        let query = doc! {"name": &name};
        let secret = PasswordGenerator::new()
            .length(12)
            .spaces(false)
            .exclude_similar_characters(true)
            .generate_one()
            .map_err(|_err| InternalError::PasswordGenerationError)?;

        let client = OAuthClient::new(name.to_owned(), secret.clone());
        let client_id = client.id.clone();

        let update = doc! {"$setOnInsert": client};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::Before)
            .upsert(true)
            .build();

        let existing_client = self
            .clients
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if existing_client.is_some() {
            Err(UserError::OAuthClientAlreadyExistsError)
        } else {
            let created_client = oauth::CreatedClientData {
                id: client_id,
                secret,
            };
            Ok(created_client)
        }
    }

    pub(crate) async fn delete_client(
        &self,
        _cc: &auth::ManageClient,
        client_id: &oauth::ClientId,
    ) -> Result<oauth::Client, UserError> {
        let query = doc! {"id": client_id};
        let client = self
            .clients
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::OAuthClientNotFoundError)?;

        Ok(client.into())
    }

    pub(crate) async fn list_clients(
        &self,
        _cc: &auth::ManageClient,
    ) -> Result<Vec<oauth::Client>, UserError> {
        let cursor = self
            .clients
            .find(doc! {}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let clients: Vec<oauth::Client> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|c| c.into())
            .collect();

        Ok(clients)
    }
}
