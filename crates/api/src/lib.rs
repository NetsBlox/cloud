use reqwest::{self, Method, RequestBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub url: String,
    pub token: Option<String>,
    pub username: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: None,
            token: None,
            url: "http://editor.netsblox.org".to_owned(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub type Token = String;
pub async fn login(cfg: &Config, credentials: &Credentials) -> Result<Token, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/users/login", cfg.url))
        .json(&credentials)
        .send()
        .await?;

    println!("{:?} status {}", &credentials, response.status());
    println!("headers: {:?}", &response.headers());
    let cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "netsblox")
        .ok_or("No cookie received.")
        .unwrap();

    Ok(cookie.value().to_owned())
}

#[derive(Serialize)]
struct UserData<'a> {
    username: &'a str,
    email: &'a str,
    admin: &'a bool,
    group_id: Option<&'a str>,
    password: Option<&'a str>,
}

#[derive(Clone)]
pub struct Client {
    cfg: Config,
}

impl Client {
    pub fn new(cfg: Config) -> Self {
        Client { cfg }
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let client = reqwest::Client::new();
        let empty = "".to_owned();
        let token = self.cfg.token.as_ref().unwrap_or(&empty);
        client
            .request(method, format!("{}{}", self.cfg.url, path))
            .header("Cookie", format!("netsblox={}", token))
    }

    pub async fn login(&self, credentials: &Credentials) -> Result<Token, reqwest::Error> {
        let response = self
            .request(Method::POST, "/users/login")
            .json(&credentials)
            .send()
            .await?;

        println!("{:?} status {}", &credentials, response.status());
        println!("headers: {:?}", &response.headers());
        let cookie = response
            .cookies()
            .find(|cookie| cookie.name() == "netsblox")
            .ok_or("No cookie received.")
            .unwrap();

        Ok(cookie.value().to_owned())
    }

    pub async fn create_user(
        // TODO: How to pass the proxy user?
        &self,
        name: &str,
        email: &str,
        password: Option<&str>, // TODO: Make these CreateUserOptions
        group_id: Option<&str>,
        admin: &bool,
    ) -> Result<(), reqwest::Error> {
        let user_data = UserData {
            username: name,
            email,
            admin,
            group_id,
            password,
        };

        let response = self
            .request(Method::POST, "/users/create")
            .json(&user_data)
            .send()
            .await?;

        println!("status {}", response.status());
        Ok(())
        // TODO: return the user data?
    }

    pub async fn list_users(&self) -> Vec<String> {
        let response = self.request(Method::GET, "/users/").send().await.unwrap();

        println!("status {}", response.status());
        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn delete_user(&self, username: &str) {
        let response = self
            .request(Method::POST, &format!("/users/{}/delete", username))
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
    }

    pub async fn link_account(
        &self,
        username: &str,
        account: &str,
        password: &str,
        strategy: &str,
    ) {
        let response = self
            .request(Method::POST, &format!("/users/{}/link/", username))
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
        todo!();
    }

    pub async fn unlink_account(&self, username: &str, account: &str, strategy: &str) {
        let response = self
            .request(Method::POST, &format!("/users/{}/unlink/", username))
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
        todo!();
    }

    pub async fn list_projects(&self, owner: &str) -> Vec<String> {
        let response = self
            .request(Method::GET, &format!("/projects/user/{}", &owner))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn export_project(&self, owner: &str, name: &str, latest: &bool) {
        todo!();
    }

    pub async fn export_role(&self, owner: &str, name: &str, role: &str, latest: &bool) {
        todo!();
    }

    pub async fn list_networks(&self) -> Vec<String> {
        let response = self.request(Method::GET, "/network/").send().await.unwrap();
        // TODO: Return addresses? or IDs?. Probably addresses since this is universal
        // This can't be used for fetching the room though...

        println!("status {}", response.status());
        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn list_friends(&self, username: &str) -> Vec<String> {
        let path = &format!("/friends/{}", username);
        let response = self.request(Method::GET, path).send().await.unwrap();
        println!("status {}", response.status());
        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn list_online_friends(&self, username: &str) -> Vec<String> {
        let path = &format!("/friends/{}/online", username);
        let response = self.request(Method::GET, path).send().await.unwrap();
        println!("status {}", response.status());
        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn list_friend_invites(&self, username: &str) -> Vec<String> {
        // FIXME: this is the wrong type
        todo!();
    }
    // pub async fn view() -> {
    //     // FIXME: refactor into an API crate and use it here

    // }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
