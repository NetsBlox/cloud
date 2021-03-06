use actix_web::{error, http::StatusCode, HttpResponse, HttpResponseBuilder};
use derive_more::{Display, Error};
use log::warn;

#[derive(Debug, Display, Error)]
pub enum InternalError {
    DatabaseConnectionError(mongodb::error::Error),
    TimeoutError,
    S3Error,
    S3ContentError,
    TorNodeListFetchError(reqwest::Error),
    ActixMessageError(actix::MailboxError),
    SendEmailError(lettre::transport::smtp::Error),
    Base64DecodeError(base64::DecodeError),
    ThumbnailDecodeError(image::ImageError),
    ThumbnailEncodeError(image::ImageError),
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    #[display(fmt = "Login required.")]
    LoginRequiredError,
    #[display(fmt = "Not allowed.")]
    PermissionsError,
    #[display(fmt = "Project not found.")]
    ProjectNotFoundError,
    #[display(fmt = "Thumbnail not available.")]
    ThumbnailNotFoundError,
    #[display(fmt = "Password reset link already sent. Only 1 can be sent per hour.")]
    PasswordResetLinkSentError,
    #[display(fmt = "Network trace not found.")]
    NetworkTraceNotFoundError,
    #[display(fmt = "Library not found.")]
    LibraryNotFoundError,
    #[display(fmt = "Role not found.")]
    RoleNotFoundError,
    #[display(fmt = "Group not found.")]
    GroupNotFoundError,
    #[display(fmt = "User not found.")]
    UserNotFoundError,
    #[display(fmt = "Invitation not found.")]
    InviteNotFoundError,
    #[display(fmt = "Service host not found.")]
    ServiceHostNotFoundError,
    #[display(fmt = "Project not active.")]
    ProjectNotActiveError,
    #[display(fmt = "Cannot delete last role.")]
    CannotDeleteLastRoleError,
    #[display(fmt = "Incorrect password.")]
    IncorrectPasswordError,
    #[display(fmt = "Incorrect username or password.")]
    IncorrectUsernameOrPasswordError,
    #[display(fmt = "User has been banned.")]
    BannedUserError,
    #[display(fmt = "User already exists.")]
    UserExistsError,
    #[display(fmt = "Group already exists.")]
    GroupExistsError,
    #[display(fmt = "Email already exists.")]
    EmailExistsError,
    #[display(fmt = "Invalid username.")]
    InvalidUsername,
    #[display(fmt = "Invalid email address.")]
    InvalidEmailAddress,
    #[display(fmt = "Invalid client ID.")]
    InvalidClientIdError,
    #[display(fmt = "Invalid app ID.")]
    InvalidAppIdError,
    #[display(fmt = "Invalid service host ID.")]
    InvalidServiceHostIDError,
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
    #[display(fmt = "Services endpoint already authorized.")]
    ServiceHostAlreadyAuthorizedError,
}

impl error::ResponseError for UserError {
    fn error_response(&self) -> HttpResponse {
        // TODO: make these JSON?
        HttpResponseBuilder::new(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            UserError::LoginRequiredError => StatusCode::UNAUTHORIZED,
            UserError::PermissionsError
            | UserError::IncorrectUsernameOrPasswordError
            | UserError::BannedUserError
            | UserError::IncorrectPasswordError => StatusCode::FORBIDDEN,

            UserError::ProjectNotFoundError
            | UserError::ThumbnailNotFoundError
            | UserError::NetworkTraceNotFoundError
            | UserError::LibraryNotFoundError
            | UserError::ServiceHostNotFoundError
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
            | UserError::InvalidServiceHostIDError
            | UserError::AccountAlreadyLinkedError
            | UserError::PasswordResetLinkSentError
            | UserError::InvalidAccountTypeError
            | UserError::TorAddressError
            | UserError::UserExistsError
            | UserError::GroupExistsError
            | UserError::EmailExistsError
            | UserError::CannotDeleteLastRoleError
            | UserError::ServiceHostAlreadyAuthorizedError
            | UserError::ProjectNotActiveError => StatusCode::BAD_REQUEST,
        }
    }
}

impl From<InternalError> for UserError {
    fn from(err: InternalError) -> UserError {
        warn!("Internal error occurred: {:?}", err);
        UserError::InternalError
    }
}
