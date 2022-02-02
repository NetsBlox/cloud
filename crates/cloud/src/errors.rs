use actix_web::{error, http::StatusCode, HttpResponse, HttpResponseBuilder};
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum InternalError {
    DatabaseConnectionError, // TODO: wrap the mongodb error
    TimeoutError,
    S3Error,
    S3ContentError,
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    #[display(fmt = "Not allowed.")]
    PermissionsError,
    #[display(fmt = "Project not found.")]
    ProjectNotFoundError,
    #[display(fmt = "Role not found.")]
    RoleNotFoundError,
    #[display(fmt = "Group not found.")]
    GroupNotFoundError,
    #[display(fmt = "User not found.")]
    UserNotFoundError,
    #[display(fmt = "Invalid username.")]
    InvalidUsername,
    #[display(fmt = "Invalid email address.")]
    InvalidEmailAddress,
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
            UserError::ProjectNotFoundError
            | UserError::RoleNotFoundError
            | UserError::UserNotFoundError
            | UserError::GroupNotFoundError => StatusCode::NOT_FOUND,
            UserError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            UserError::InvalidUsername | UserError::InvalidEmailAddress => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<InternalError> for UserError {
    fn from(_err: InternalError) -> UserError {
        // TODO: log this?
        UserError::InternalError
    }
}