use derive_more::Display;
use into_jsvalue_derive::IntoJsValue;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Display, IntoJsValue)]
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
