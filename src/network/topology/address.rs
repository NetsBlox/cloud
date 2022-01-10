use itertools::Itertools;
use std::{error, fmt, str::FromStr};

pub static DEFAULT_APP_ID: &str = "netsblox";

#[derive(Clone)]
pub struct ClientAddress {
    pub address: String,
    pub user_id: String,
    pub app_ids: Vec<String>,
}

impl ClientAddress {
    /// Get the address for routing within the app (ie, excluding the app tags).
    pub fn to_app_string(&self) -> String {
        format!("{}@{}", self.address, self.user_id)
    }
}

#[derive(Debug)]
pub struct AddressError {
    addr: String,
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid address: {}", &self.addr)
    }
}

impl error::Error for AddressError {}

impl FromStr for ClientAddress {
    type Err = AddressError;

    fn from_str(addr: &str) -> Result<Self, Self::Err> {
        if let Some(index) = addr.rfind('@') {
            let address = addr.chars().into_iter().take(index).collect::<String>();
            let user_id = addr
                .chars()
                .into_iter()
                .skip(index + 1)
                .take_while(|c| !c.is_whitespace() && *c != '#')
                .collect::<String>();

            let mut app_ids: Vec<String> = addr
                .chars()
                .into_iter()
                .skip(index + user_id.len() + 1)
                .group_by(|c| !c.is_whitespace() && *c != '#')
                .into_iter()
                .filter(|(k, _iter)| *k)
                .map(|(_k, iter)| iter.collect::<String>().to_lowercase())
                .collect();

            if app_ids.is_empty() {
                app_ids.push(DEFAULT_APP_ID.to_owned());
            }

            Ok(ClientAddress {
                address,
                user_id,
                app_ids,
            })
        } else {
            Err(AddressError {
                addr: addr.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_parse_address() {
        let addr = ClientAddress::from_str("role@untitled@brian").unwrap();
        assert_eq!(addr.address, "role@untitled");
    }

    #[actix_web::test]
    async fn test_parse_user_id() {
        let addr = ClientAddress::from_str("untitled@brian").unwrap();
        assert_eq!(addr.user_id, "brian");
    }

    #[actix_web::test]
    async fn test_parse_default_app_id() {
        let addr = ClientAddress::from_str("untitled@brian").unwrap();
        assert_eq!(addr.app_ids.len(), 1);
        assert_eq!(addr.app_ids[0], "netsblox");
    }

    #[actix_web::test]
    async fn test_parse_app_id() {
        let addr = ClientAddress::from_str("untitled@brian \t#PyBlox").unwrap();

        assert_eq!(addr.app_ids.len(), 1);
        assert_eq!(addr.app_ids[0], "pyblox");
    }

    #[actix_web::test]
    async fn test_parse_multi_app_ids() {
        let addr = ClientAddress::from_str("untitled@brian#PyBlox #NetsBlox#NewExample").unwrap();

        assert_eq!(addr.app_ids.len(), 3);
        assert_eq!(addr.app_ids[0], "pyblox");
        assert_eq!(addr.app_ids[1], "netsblox");
        assert_eq!(addr.app_ids[2], "newexample");
    }
}
