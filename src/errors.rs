use actix_web::{error, http::StatusCode, HttpResponse, HttpResponseBuilder};
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum InternalError {
    DatabaseConnectionError, // TODO: wrap the mongodb error
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    #[display(fmt = "Not allowed.")]
    PermissionsError,
    #[display(fmt = "Project not found.")]
    ProjectNotFoundError,
    #[display(fmt = "An internal error occurred. Please try again later.")]
    InternalError,
}

impl error::ResponseError for UserError {
    fn error_response(&self) -> HttpResponse {
        // TODO: make these JSON?
        HttpResponseBuilder::new(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            UserError::PermissionsError => StatusCode::UNAUTHORIZED,
            UserError::ProjectNotFoundError => StatusCode::NOT_FOUND,
            UserError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<InternalError> for UserError {
    fn from(err: InternalError) -> UserError {
        UserError::InternalError
    }
}