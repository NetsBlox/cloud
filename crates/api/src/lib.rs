pub mod core;

use crate::core::*;
use futures_util::stream::SplitStream;
use netsblox_core::{CreateGroupData, UpdateGroupData};
use reqwest::{self, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub app_id: Option<String>,
    pub url: String,
    pub token: Option<String>,
    pub username: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_id: None,
            username: None,
            token: None,
            url: "http://editor.netsblox.org".to_owned(),
        }
    }
}

pub type Token = String;
pub async fn login(cfg: &mut Config, credentials: &LoginRequest) -> Result<(), reqwest::Error> {
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

    let token = cookie.value().to_owned();

    cfg.username = Some(response.text().await.unwrap());
    cfg.token = Some(token);
    Ok(())
}

#[derive(Serialize)]
struct UserData<'a> {
    username: &'a str,
    email: &'a str,
    role: &'a UserRole,
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

    pub async fn login(&self, credentials: &LoginRequest) -> Result<Token, reqwest::Error> {
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

    // User management
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

        println!(
            "status {} {}",
            response.status(),
            response.text().await.unwrap()
        );
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

    pub async fn view_user(&self, username: &str) -> User {
        let response = self
            .request(Method::GET, &format!("/users/{}", username))
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
        response.json::<User>().await.unwrap()
    }

    pub async fn set_password(&self, username: &str, password: &str) {
        let path = format!("/users/{}/password", username);
        let response = self
            .request(Method::PATCH, &path)
            .json(&password)
            .send()
            .await
            .unwrap();
        println!("status {}", response.status());
    }

    pub async fn link_account(&self, username: &str, credentials: &core::Credentials) {
        let response = self
            .request(Method::POST, &format!("/users/{}/link/", username))
            .json(&credentials)
            .send()
            .await
            .unwrap();

        println!("status: {}", response.status());
    }

    pub async fn unlink_account(&self, username: &str, account: &LinkedAccount) {
        let response = self
            .request(Method::POST, &format!("/users/{}/unlink", username))
            .json(&account)
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
    }

    pub async fn ban_user(&self, username: &str) {
        let response = self
            .request(Method::POST, &format!("/users/{}/ban", username))
            .send()
            .await
            .unwrap();
        println!("status: {}", response.status());
    }

    // Project management
    pub async fn list_projects(&self, owner: &str) -> Vec<ProjectMetadata> {
        let response = self
            .request(Method::GET, &format!("/projects/user/{}", &owner))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<Vec<ProjectMetadata>>().await.unwrap()
    }

    pub async fn list_shared_projects(&self, owner: &str) -> Vec<ProjectMetadata> {
        let response = self
            .request(Method::GET, &format!("/projects/shared/{}", &owner))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<Vec<ProjectMetadata>>().await.unwrap()
    }

    pub async fn get_project_metadata(&self, owner: &str, name: &str) -> ProjectMetadata {
        let response = self
            .request(
                Method::GET,
                &format!("/projects/user/{}/{}/metadata", &owner, name),
            )
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<ProjectMetadata>().await.unwrap()
    }

    pub async fn rename_project(&self, id: &ProjectId, name: &str) {
        let response = self
            .request(Method::PATCH, &format!("/projects/id/{}", &id))
            .json(&UpdateProjectData {
                name: name.to_owned(),
                client_id: None,
            })
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn rename_role(&self, id: &ProjectId, role_id: &str, name: &str) {
        let response = self
            .request(Method::PATCH, &format!("/projects/id/{}/{}", &id, &role_id))
            .json(&UpdateRoleData {
                name: name.to_owned(),
                client_id: None,
            })
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn delete_project(&self, id: &ProjectId) {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}", id))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn delete_role(&self, id: &ProjectId, role_id: &str) {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}/{}", id, role_id))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn publish_project(&self, id: &ProjectId) {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/publish", id))
            .send()
            .await
            .unwrap();

        println!(
            "status {} {}",
            response.status(),
            response.text().await.unwrap()
        );
    }

    pub async fn unpublish_project(&self, id: &ProjectId) {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/unpublish", id))
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn get_project(&self, id: &ProjectId, latest: &bool) -> Project {
        let path = if *latest {
            format!("/projects/id/{}/latest", id)
        } else {
            format!("/projects/id/{}", id)
        };
        let response = self.request(Method::GET, &path).send().await.unwrap();
        response.json::<Project>().await.unwrap()
    }

    pub async fn get_role(&self, id: &ProjectId, role_id: &str, latest: &bool) -> RoleData {
        let path = if *latest {
            format!("/projects/id/{}/{}/latest", id, role_id)
        } else {
            format!("/projects/id/{}/{}", id, role_id)
        };
        let response = self.request(Method::GET, &path).send().await.unwrap();
        response.json::<RoleData>().await.unwrap()
    }

    // Project collaborators
    pub async fn list_collaborators(&self, project_id: &str) -> Vec<String> {
        let response = self
            .request(Method::GET, &format!("/id/{}/collaborators/", project_id))
            .send()
            .await
            .unwrap();

        response.json::<Vec<String>>().await.unwrap()
    }

    pub async fn remove_collaborator(&self, project_id: &ProjectId, username: &str) {
        let response = self
            .request(
                Method::DELETE,
                &format!("/projects/id/{}/collaborators/{}", project_id, username),
            )
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn list_collaboration_invites(&self, username: &str) -> Vec<CollaborationInvite> {
        let response = self
            .request(
                Method::GET,
                &format!("/collaboration-invites/user/{}/", username),
            )
            .send()
            .await
            .unwrap();

        response.json::<Vec<CollaborationInvite>>().await.unwrap()
    }

    pub async fn invite_collaborator(&self, id: &ProjectId, username: &str) {
        let response = self
            .request(
                Method::POST,
                &format!("/collaboration-invites/{}/invite/{}", id, username),
            )
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn respond_to_collaboration_invite(
        &self,
        id: &InvitationId,
        state: &InvitationState,
    ) {
        let response = self
            .request(Method::POST, &format!("/collaboration-invites/id/{}", id))
            .json(state)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    // Friend capabilities
    pub async fn list_friends(&self, username: &str) -> Vec<String> {
        let path = &format!("/friends/{}/", username);
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

    pub async fn list_friend_invites(&self, username: &str) -> Vec<FriendInvite> {
        let path = &format!("/friends/{}/invites/", username);
        let response = self.request(Method::GET, path).send().await.unwrap();
        println!("status {}", response.status());
        response.json::<Vec<FriendInvite>>().await.unwrap()
    }

    pub async fn send_friend_invite(&self, username: &str, recipient: &str) {
        let path = &format!("/friends/{}/invite/", username);
        let response = self
            .request(Method::POST, path)
            .json(recipient)
            .send()
            .await
            .unwrap();
        println!("status {}", response.status());
    }

    pub async fn respond_to_friend_invite(
        &self,
        recipient: &str,
        sender: &str,
        state: FriendLinkState,
    ) -> () {
        let path = format!("/friends/{}/invites/{}", recipient, sender);
        let response = self
            .request(Method::POST, &path)
            .json(&state)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn unfriend(&self, username: &str, friend: &str) -> () {
        let path = format!("/friends/{}/unfriend/{}", username, friend);
        let response = self.request(Method::POST, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    pub async fn block_user(&self, username: &str, other_user: &str) {
        let path = format!("/friends/{}/block/{}", username, other_user);
        let response = self.request(Method::POST, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    pub async fn unblock_user(&self, username: &str, other_user: &str) {
        let path = format!("/friends/{}/unblock/{}", username, other_user);
        let response = self.request(Method::POST, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    // Library capabilities
    pub async fn get_libraries(&self, username: &str) -> Vec<LibraryMetadata> {
        let path = format!("/libraries/user/{}/", username);
        let response = self.request(Method::GET, &path).send().await.unwrap();
        response.json::<Vec<LibraryMetadata>>().await.unwrap()
    }

    pub async fn get_submitted_libraries(&self) -> Vec<LibraryMetadata> {
        let response = self
            .request(Method::GET, "/libraries/mod/pending")
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<Vec<LibraryMetadata>>().await.unwrap()
    }

    pub async fn get_public_libraries(&self) -> Vec<LibraryMetadata> {
        let response = self
            .request(Method::GET, "/libraries/community")
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        response.json::<Vec<LibraryMetadata>>().await.unwrap()
    }

    pub async fn get_library(&self, username: &str, name: &str) -> String {
        let path = format!("/libraries/user/{}/{}", username, name); // TODO: URI escape?
        let response = self.request(Method::GET, &path).send().await.unwrap();
        println!("status {}", response.status());
        response.text().await.unwrap()
    }

    pub async fn save_library(&self, username: &str, name: &str, blocks: &str, notes: &str) {
        let path = format!("/libraries/user/{}/", username);
        let response = self
            .request(Method::POST, &path)
            .json(&CreateLibraryData {
                name: name.to_owned(),
                blocks: blocks.to_owned(),
                notes: notes.to_owned(),
            })
            .send()
            .await
            .unwrap();
        println!("status {}", response.status());
    }

    pub async fn delete_library(&self, username: &str, library: &str) {
        let path = format!("/libraries/user/{}/{}", username, library);
        let response = self.request(Method::DELETE, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    pub async fn publish_library(&self, username: &str, library: &str) {
        let path = format!("/libraries/user/{}/{}/publish", username, library);
        let response = self.request(Method::POST, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    pub async fn unpublish_library(&self, username: &str, library: &str) {
        let path = format!("/libraries/user/{}/{}/unpublish", username, library);
        let response = self.request(Method::POST, &path).send().await.unwrap();
        println!("status {}", response.status());
    }

    pub async fn approve_library(
        &self,
        username: &str,
        library: &str,
        state: &LibraryPublishState,
    ) {
        let path = format!("/libraries/mod/{}/{}", username, library);
        let response = self
            .request(Method::POST, &path)
            .json(&state)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    // Group management
    pub async fn list_groups(&self, username: &str) -> Vec<Group> {
        let path = format!("/groups/user/{}", username);
        let response = self.request(Method::GET, &path).send().await.unwrap();

        println!("status {}", response.status());
        response.json::<Vec<Group>>().await.unwrap()
    }

    pub async fn create_group(&self, owner: &str, name: &str) {
        let path = format!("/groups/user/{}", owner);
        let group = CreateGroupData {
            name: name.to_owned(),
            services_hosts: None,
        };
        let response = self
            .request(Method::POST, &path)
            .json(&group)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn delete_group(&self, id: &str) {
        let path = format!("/groups/id/{}", id);
        let response = self.request(Method::DELETE, &path).send().await.unwrap();

        println!("status {}", response.status());
    }

    pub async fn list_members(&self, id: &str) -> Vec<User> {
        let path = format!("/groups/id/{}/members", id);
        let response = self.request(Method::GET, &path).send().await.unwrap();

        println!("status {}", response.status());
        response.json::<Vec<User>>().await.unwrap()
    }

    pub async fn rename_group(&self, id: &str, name: &str) {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::PATCH, &path)
            .json(&UpdateGroupData {
                name: name.to_owned(),
            })
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn view_group(&self, id: &str) -> Group {
        let path = format!("/groups/id/{}", id);
        let response = self.request(Method::GET, &path).send().await.unwrap();

        println!("status {}", response.status());
        response.json::<Group>().await.unwrap()
    }

    // Service host management
    pub async fn list_user_hosts(&self, username: &str) -> Vec<ServiceHost> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/user/{}", username))
            .send()
            .await
            .unwrap();

        response.json::<Vec<ServiceHost>>().await.unwrap()
    }

    pub async fn list_group_hosts(&self, group_id: &str) -> Vec<ServiceHost> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/group/{}", group_id))
            .send()
            .await
            .unwrap();

        response.json::<Vec<ServiceHost>>().await.unwrap()
    }

    pub async fn list_hosts(&self, username: &str) -> Vec<ServiceHost> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/all/{}", username))
            .send()
            .await
            .unwrap();

        response.json::<Vec<ServiceHost>>().await.unwrap()
    }

    pub async fn set_user_hosts(&self, username: &str, hosts: Vec<ServiceHost>) {
        let response = self
            .request(Method::POST, &format!("/service-hosts/user/{}", username))
            .json(&hosts)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    pub async fn set_group_hosts(&self, group_id: &str, hosts: Vec<ServiceHost>) {
        let response = self
            .request(Method::POST, &format!("/service-hosts/group/{}", group_id))
            .json(&hosts)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
    }

    // NetsBlox network capabilities
    pub async fn list_external_clients(&self) -> Vec<ExternalClient> {
        let response = self
            .request(Method::GET, "/network/external")
            .send()
            .await
            .unwrap();

        response.json::<Vec<ExternalClient>>().await.unwrap()
    }

    pub async fn list_networks(&self) -> Vec<ProjectId> {
        let response = self.request(Method::GET, "/network/").send().await.unwrap();

        response.json::<Vec<ProjectId>>().await.unwrap()
    }

    pub async fn get_room_state(&self, id: &ProjectId) -> RoomState {
        let response = self
            .request(Method::GET, &format!("/network/id/{}", id))
            .send()
            .await
            .unwrap();

        response.json::<RoomState>().await.unwrap()
    }

    pub async fn evict_occupant(&self, client_id: &ClientID) {
        let response = self
            .request(
                Method::POST,
                &format!("/network/clients/{}/evict", client_id),
            )
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());
        println!("text {}", response.text().await.unwrap());
    }

    pub async fn connect(&self, address: &str) -> MessageChannel {
        let response = self
            .request(Method::GET, "/configuration")
            .send()
            .await
            .unwrap();

        let config = response.json::<ClientConfig>().await.unwrap();

        println!("Connecting with client ID: {}", &config.client_id);
        let url = format!(
            "{}/network/{}/connect",
            self.cfg.url.replace("http", "ws"),
            config.client_id
        );
        println!("trying to connect to: {}", &url);
        let (ws_stream, _) = connect_async(&url).await.unwrap();

        let state = ClientStateData {
            state: ClientState::External(ExternalClientState {
                address: address.to_owned(),
                app_id: self.cfg.app_id.as_ref().unwrap().clone(),
                // .ok_or(ClientError::NoAppID)?,
            }),
        };

        let response = self
            .request(
                Method::POST,
                &format!("/network/{}/state", config.client_id),
            )
            .json(&state)
            .send()
            .await
            .unwrap();

        println!("status {}", response.status());

        MessageChannel {
            id: config.client_id,
            stream: ws_stream,
        }
        // let (write, read) = ws_stream.split();
        // let read_channel = read.filter_map(|msg| {
        //     future::ready(match msg {
        //         Ok(Message::Text(txt)) => Some(txt),
        //         _ => None,
        //     })
        // });
        // // let read_channel = read.filter(|msg| future::ready(msg.is_ok()));
        // MessageChannel {
        //     id: config.client_id,
        //     read: Box::new(read_channel),
        //     //     .filter_map(|msg| match msg {
        //     //     Ok(Message::Text(txt)) => Some(txt),
        //     //     _ => None,
        //     // }),
        //     write,
        // }
    }
}

struct MessageReadStream {
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

// impl Stream for MessageReadStream {
// type Item =
// }

pub struct MessageChannel {
    pub id: String,
    pub stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl MessageChannel {
    // TODO: do we need a method for sending other types?
    pub async fn send(&self, msg_type: &str) {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
