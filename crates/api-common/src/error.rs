use crate::{ServiceHostId, ServiceName, SettingName};
use derive_more::{Display, Error};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Display, Debug, Error)]
pub enum ValidationError {
    #[display(fmt = "Project name is invalid")]
    ProjectName,
    #[display(fmt = "Role name is invalid")]
    RoleName,
    #[display(fmt = "Host name is invalid")]
    HostId,
    #[display(fmt = "Service name is invalid")]
    ServiceName,
    #[display(fmt = "Setting name is invalid")]
    SettingName,
    #[display(fmt = "Username is invalid")]
    Username,
    #[display(fmt = "Email is invalid")]
    Email,
}

static HOST_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z][A-Za-z0-9_-]{2,24}$").expect("Failed to compile HOST_ID_REGEX")
});

impl TryFrom<String> for ServiceHostId {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if HOST_ID_REGEX.is_match(&value) {
            Ok(ServiceHostId(value))
        } else {
            Err(ValidationError::HostId)
        }
    }
}

impl TryFrom<String> for ServiceName {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.contains('.') || value.contains('$') {
            Err(ValidationError::ServiceName)
        } else {
            Ok(ServiceName(value))
        }
    }
}

impl TryFrom<String> for SettingName {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.contains('.') || value.contains('$') {
            Err(ValidationError::SettingName)
        } else {
            Ok(SettingName(value))
        }
    }
}
