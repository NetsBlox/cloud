use derive_more::Display;

#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "{}", _0)]
    BadRequestError(String),
    #[display(fmt = "{}", _0)]
    ParseResponseFailedError(String),
    #[display(fmt = "Login required.")]
    LoginRequiredError,
    #[display(fmt = "Unauthorized: {}", _0)]
    PermissionsError(String),
    #[display(fmt = "{}", _0)]
    NotFoundError(String),
    #[display(fmt = "Internal server error occurred")]
    InternalServerError,
    #[display(fmt = "Request Error: {}", _0)]
    RequestError(String),
    #[display(fmt = "WebSocketError: {}", _0)]
    WebSocketError(String),
}
