use std::time::SystemTime;

use mongodb::{bson::doc, Collection};
use netsblox_cloud_common::{api::oauth, OAuthClient, OAuthToken};
use uuid::Uuid;

use crate::{
    auth,
    errors::{InternalError, OAuthFlowError, UserError},
    utils::sha512,
};

use super::routes::AuthorizeClientParams;

pub(crate) struct OAuthActions {
    clients: Collection<OAuthClient>,
    tokens: Collection<OAuthToken>,
    codes: Collection<oauth::Code>,
}

impl OAuthActions {
    pub(crate) async fn authorize(
        &self,
        eu: &auth::EditUser,
        params: AuthorizeClientParams,
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
}
