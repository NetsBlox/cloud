use actix_web::{error, http::StatusCode, HttpResponse, HttpResponseBuilder};
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum InternalError {
    DatabaseConnectionError, // TODO: wrap the mongodb error
    TimeoutError,
    S3Error,
    S3ContentError,
    TorNodeListFetchError,
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    #[display(fmt = "Login required.")]
    LoginRequiredError,
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
    #[display(fmt = "Invitation not found.")]
    InviteNotFoundError,
    #[display(fmt = "Project not active.")]
    ProjectNotActiveError,
    #[display(fmt = "Incorrect password.")]
    IncorrectPasswordError,
    #[display(fmt = "Incorrect username or password.")]
    IncorrectUsernameOrPasswordError,
    #[display(fmt = "User has been banned.")]
    BannedUserError,
    #[display(fmt = "Invalid username.")]
    InvalidUsername,
    #[display(fmt = "Invalid email address.")]
    InvalidEmailAddress,
    #[display(fmt = "Invalid client ID.")]
    InvalidClientIdError,
    #[display(fmt = "Invalid app ID.")]
    InvalidAppIdError,
    #[display(fmt = "Invalid authentication strategy.")]
    InvalidAuthStrategyError,
    #[display(fmt = "Unable to connect to Snap! Please try again later.")]
    SnapConnectionError,
    #[display(fmt = "Account already linked to NetsBlox user.")]
    AccountAlreadyLinkedError,
    #[display(fmt = "Invalid account type.")]
    InvalidAccountTypeError,
    #[display(fmt = "Login from Tor not allowed.")]
    TorAddressError,
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
            UserError::LoginRequiredError => StatusCode::UNAUTHORIZED,
            // TODO: use Forbidden if logged in
            UserError::PermissionsError
            | UserError::IncorrectUsernameOrPasswordError
            | UserError::BannedUserError
            | UserError::IncorrectPasswordError => StatusCode::FORBIDDEN,

            UserError::ProjectNotFoundError
            | UserError::RoleNotFoundError
            | UserError::InviteNotFoundError
            | UserError::UserNotFoundError
            | UserError::GroupNotFoundError => StatusCode::NOT_FOUND,
            UserError::InternalError | UserError::SnapConnectionError => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            UserError::InvalidUsername
            | UserError::InvalidEmailAddress
            | UserError::InvalidClientIdError
            | UserError::InvalidAppIdError
            | UserError::InvalidAuthStrategyError
            | UserError::AccountAlreadyLinkedError
            | UserError::InvalidAccountTypeError
            | UserError::TorAddressError
            | UserError::ProjectNotActiveError => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<InternalError> for UserError {
    fn from(_err: InternalError) -> UserError {
        // TODO: log this?
        UserError::InternalError
    }
}
