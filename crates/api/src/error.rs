use derive_more::Display;

#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "{}", _0)]
    BadRequestError(String),
    #[display(fmt = "Login required.")]
    LoginRequiredError,
    #[display(fmt = "Unauthorized: {}", _0)]
    PermissionsError(String),
    #[display(fmt = "{}", _0)]
    NotFoundError(String),
    #[display(fmt = "{}", _0)]
    InternalServerError(String),
    RequestError(reqwest::Error),
    WebSocketSendError(tokio_tungstenite::tungstenite::Error),
}
