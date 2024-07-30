use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Display)]
#[cfg_attr(
    target_arch = "wasm32",
    derive(tsify_next::Tsify),
    tsify(into_wasm_abi, from_wasm_abi)
)]
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
