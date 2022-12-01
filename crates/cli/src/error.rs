use derive_more::Display;

#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "{}", _0)]
    APIError(netsblox_api::error::Error),
    #[display(fmt = "Host not found.")]
    HostNotFoundError,
}

impl From<netsblox_api::error::Error> for Error {
    fn from(api_err: netsblox_api::error::Error) -> Error {
        Error::APIError(api_err)
    }
}
