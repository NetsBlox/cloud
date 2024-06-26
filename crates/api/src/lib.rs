pub mod common;
pub mod error;

pub use serde_json;

use crate::common::*;
use bytes::Bytes;

use futures_util::SinkExt;
use reqwest::{self, Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub app_id: Option<AppId>,
    pub username: Option<String>,
    pub token: Option<String>,
    pub url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_id: None,
            username: None,
            token: None,
            url: "https://cloud.netsblox.org".to_owned(),
        }
    }
}

async fn check_response(response: Response) -> Result<Response, error::Error> {
    let status_code = response.status().as_u16();
    let is_error = status_code > 399;
    if is_error {
        let msg = response
            .text()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        match status_code {
            400 => Err(error::Error::BadRequestError(msg)),
            401 => Err(error::Error::LoginRequiredError),
            403 => Err(error::Error::PermissionsError(msg)),
            404 => Err(error::Error::NotFoundError(msg)),
            500 => Err(error::Error::InternalServerError),
            _ => panic!("Unknown status code: {:?}", status_code), // FIXME: Use error instead?
        }
    } else {
        Ok(response)
    }
}

pub async fn login(mut cfg: Config, credentials: &LoginRequest) -> Result<Config, error::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/users/login", cfg.url))
        .json(&credentials)
        .send()
        .await
        .map_err(|e| error::Error::RequestError(e.to_string()))?;

    let response = check_response(response).await?;
    let cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "netsblox")
        .ok_or("No cookie received.")
        .unwrap();

    let token = cookie.value().to_owned();

    let user = response.json::<User>().await.unwrap();
    cfg.username = Some(user.username);
    cfg.token = Some(token);
    Ok(cfg)
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

    // User management
    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: Option<&str>, // TODO: Make these CreateUserOptions
        group_id: Option<&GroupId>,
        role: UserRole,
    ) -> Result<(), error::Error> {
        let user_data = NewUser {
            username: name.to_owned(),
            email: email.to_owned(),
            role: Some(role),
            group_id: group_id.map(|id| id.to_owned()),
            password: password.map(|pwd| pwd.to_owned()),
        };

        let response = self
            .request(Method::POST, "/users/create")
            .json(&user_data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        println!(
            "status {} {}",
            response.status(),
            response.text().await.unwrap()
        );
        Ok(())
    }

    pub async fn list_users(&self) -> Result<Vec<User>, error::Error> {
        let response = self
            .request(Method::GET, "/users/")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<User>>().await.unwrap())
    }

    /// Send an email containing all usernames associated with the given
    /// address to the email address.
    pub async fn forgot_username(&self, email: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, "/users/forgot-username")
            .json(&email)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_user(&self, username: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/delete", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_user(&self, username: &str) -> Result<User, error::Error> {
        let response = self
            .request(Method::GET, &format!("/users/{}", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<User>().await.unwrap())
    }

    pub async fn set_password(&self, username: &str, password: &str) -> Result<(), error::Error> {
        let path = format!("/users/{}/password", username);
        let response = self
            .request(Method::PATCH, &path)
            .json(&password)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn link_account(
        &self,
        username: &str,
        credentials: &Credentials,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/link/", username))
            .json(&credentials)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unlink_account(
        &self,
        username: &str,
        account: &LinkedAccount,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/unlink", username))
            .json(&account)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn ban_user(&self, username: &str) -> Result<BannedAccount, error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/ban", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<BannedAccount>().await.unwrap())
    }

    pub async fn unban_user(&self, username: &str) -> Result<BannedAccount, error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/unban", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<BannedAccount>().await.unwrap())
    }

    /// Send a magic link to the given email address. Usable for any user associated with the
    /// address.
    pub async fn send_magic_link(&self, data: &CreateMagicLinkData) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, "/magic-links/")
            .json(data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    // Project management
    pub async fn create_project(
        &self,
        data: &CreateProjectData,
    ) -> Result<ProjectMetadata, error::Error> {
        // TODO: what should the method signature look like for this? Probably should accept CreateProjectData
        let response = self
            .request(Method::POST, "/projects/")
            .json(data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<ProjectMetadata>().await.unwrap())
    }

    pub async fn list_projects(&self, owner: &str) -> Result<Vec<ProjectMetadata>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/projects/user/{}", &owner))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ProjectMetadata>>().await.unwrap())
    }

    pub async fn list_shared_projects(
        &self,
        owner: &str,
    ) -> Result<Vec<ProjectMetadata>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/projects/shared/{}", &owner))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ProjectMetadata>>().await.unwrap())
    }

    pub async fn get_project_metadata(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<ProjectMetadata, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/projects/user/{}/{}/metadata", &owner, name),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<ProjectMetadata>().await.unwrap())
    }

    pub async fn rename_project(&self, id: &ProjectId, name: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::PATCH, &format!("/projects/id/{}", &id))
            .json(&UpdateProjectData {
                name: name.to_owned(),
                client_id: None,
            })
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn rename_role(
        &self,
        id: &ProjectId,
        role_id: &RoleId,
        name: &str,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::PATCH, &format!("/projects/id/{}/{}", &id, &role_id))
            .json(&UpdateRoleData {
                name: name.to_owned(),
                client_id: None,
            })
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}", id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_role(&self, id: &ProjectId, role_id: &RoleId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}/{}", id, role_id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn publish_project(&self, id: &ProjectId) -> Result<PublishState, error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/publish", id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<PublishState>().await.unwrap())
    }

    pub async fn unpublish_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/unpublish", id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn get_project(
        &self,
        id: &ProjectId,
        latest: &bool,
    ) -> Result<Project, error::Error> {
        let path = if *latest {
            format!("/projects/id/{}/latest", id)
        } else {
            format!("/projects/id/{}", id)
        };
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Project>().await.unwrap())
    }

    pub async fn get_role(
        &self,
        id: &ProjectId,
        role_id: &RoleId,
        latest: &bool,
    ) -> Result<RoleData, error::Error> {
        let path = if *latest {
            format!("/projects/id/{}/{}/latest", id, role_id)
        } else {
            format!("/projects/id/{}/{}", id, role_id)
        };
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<RoleData>().await.unwrap())
    }

    // Project collaborators
    pub async fn list_collaborators(&self, project_id: &str) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/id/{}/collaborators/", project_id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn remove_collaborator(
        &self,
        project_id: &ProjectId,
        username: &str,
    ) -> Result<(), error::Error> {
        let response = self
            .request(
                Method::DELETE,
                &format!("/projects/id/{}/collaborators/{}", project_id, username),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn list_collaboration_invites(
        &self,
        username: &str,
    ) -> Result<Vec<CollaborationInvite>, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/collaboration-invites/user/{}/", username),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<CollaborationInvite>>().await.unwrap())
    }

    pub async fn invite_collaborator(
        &self,
        id: &ProjectId,
        username: &str,
    ) -> Result<(), error::Error> {
        let response = self
            .request(
                Method::POST,
                &format!("/collaboration-invites/{}/invite/{}", id, username),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn respond_to_collaboration_invite(
        &self,
        id: &InvitationId,
        state: &InvitationState,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/collaboration-invites/id/{}", id))
            .json(state)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    // Friend capabilities
    pub async fn list_friends(&self, username: &str) -> Result<Vec<String>, error::Error> {
        let path = &format!("/friends/{}/", username);
        let response = self
            .request(Method::GET, path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn list_online_friends(&self, username: &str) -> Result<Vec<String>, error::Error> {
        let path = &format!("/friends/{}/online", username);
        let response = self
            .request(Method::GET, path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn list_friend_invites(
        &self,
        username: &str,
    ) -> Result<Vec<FriendInvite>, error::Error> {
        let path = &format!("/friends/{}/invites/", username);
        let response = self
            .request(Method::GET, path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<FriendInvite>>().await.unwrap())
    }

    pub async fn send_friend_invite(
        &self,
        username: &str,
        recipient: &str,
    ) -> Result<(), error::Error> {
        let path = &format!("/friends/{}/invite/", username);
        let response = self
            .request(Method::POST, path)
            .json(recipient)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn respond_to_friend_invite(
        &self,
        recipient: &str,
        sender: &str,
        state: FriendLinkState,
    ) -> Result<(), error::Error> {
        let path = format!("/friends/{}/invites/{}", recipient, sender);
        let response = self
            .request(Method::POST, &path)
            .json(&state)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unfriend(&self, username: &str, friend: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unfriend/{}", username, friend);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn block_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/block/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unblock_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unblock/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    // Library capabilities
    pub async fn get_libraries(
        &self,
        username: &str,
    ) -> Result<Vec<LibraryMetadata>, error::Error> {
        let path = format!("/libraries/user/{}/", username);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_submitted_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/mod/pending")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_public_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/community/")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_library(&self, username: &str, name: &str) -> Result<String, error::Error> {
        let path = format!("/libraries/user/{}/{}", username, name); // TODO: URI escape?
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.text().await.unwrap())
    }

    pub async fn save_library(
        &self,
        username: &str,
        name: &str,
        blocks: &str,
        notes: &str,
    ) -> Result<(), error::Error> {
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
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}", username, library);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn publish_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}/publish", username, library);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unpublish_library(
        &self,
        username: &str,
        library: &str,
    ) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}/unpublish", username, library);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn approve_library(
        &self,
        username: &str,
        library: &str,
        state: &PublishState,
    ) -> Result<(), error::Error> {
        let path = format!("/libraries/mod/{}/{}", username, library);
        let response = self
            .request(Method::POST, &path)
            .json(&state)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    // Group management
    pub async fn list_groups(&self, username: &str) -> Result<Vec<Group>, error::Error> {
        let path = format!("/groups/user/{}/", username);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<Group>>().await.unwrap())
    }

    pub async fn create_group(&self, owner: &str, name: &str) -> Result<(), error::Error> {
        let path = format!("/groups/user/{}/", owner);
        let group = CreateGroupData {
            name: name.to_owned(),
            services_hosts: None,
        };
        let response = self
            .request(Method::POST, &path)
            .json(&group)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_group(&self, id: &GroupId) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_members(&self, id: &GroupId) -> Result<Vec<User>, error::Error> {
        let path = format!("/groups/id/{}/members", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<User>>().await.unwrap())
    }

    pub async fn rename_group(&self, id: &GroupId, name: &str) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::PATCH, &path)
            .json(&UpdateGroupData {
                name: name.to_owned(),
            })
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_group(&self, id: &GroupId) -> Result<Group, error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Group>().await.unwrap())
    }

    // Service host management
    pub async fn list_user_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/services/hosts/user/{}", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn list_group_hosts(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/services/hosts/group/{}", group_id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn list_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/services/hosts/all/{}", username))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn set_user_hosts(
        &self,
        username: &str,
        hosts: Vec<ServiceHost>,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/services/hosts/user/{}", username))
            .json(&hosts)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn set_group_hosts(
        &self,
        group_id: &GroupId,
        hosts: Vec<ServiceHost>,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/services/hosts/group/{}", group_id))
            .json(&hosts)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn authorize_host(
        &self,
        url: &str,
        id: &str,
        visibility: ServiceHostScope,
    ) -> Result<String, error::Error> {
        let host = AuthorizedServiceHost {
            url: url.to_owned(),
            id: id.to_owned(),
            visibility,
        };
        let response = self
            .request(Method::POST, "/services/hosts/authorized/")
            .json(&host)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<String>().await.unwrap())
    }

    pub async fn unauthorize_host(&self, id: &str) -> Result<(), error::Error> {
        let response = self
            .request(
                Method::DELETE,
                &format!("/services/hosts/authorized/{}", id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_authorized_hosts(&self) -> Result<Vec<AuthorizedServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, "/services/hosts/authorized/")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<AuthorizedServiceHost>>().await.unwrap())
    }

    // Service settings management
    pub async fn list_group_settings(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/services/settings/group/{}/", group_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn list_user_settings(&self, username: &str) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/services/settings/user/{}/", username),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn get_all_settings(
        &self,
        username: &str,
        service_id: &str,
    ) -> Result<ServiceSettings, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/services/settings/user/{}/{}/all", username, service_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.json::<ServiceSettings>().await.unwrap())
    }

    pub async fn get_group_settings(
        &self,
        group_id: &GroupId,
        service_id: &str,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/services/settings/group/{}/{}", group_id, service_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }

    pub async fn get_user_settings(
        &self,
        username: &str,
        service_id: &str,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/services/settings/user/{}/{}", username, service_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }

    pub async fn set_user_settings(
        &self,
        username: &str,
        service_id: &str,
        settings: String,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::POST,
                &format!("/services/settings/user/{}/{}", username, service_id),
            )
            .body(settings)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }

    pub async fn set_group_settings(
        &self,
        group_id: &GroupId,
        service_id: &str,
        settings: String,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::POST,
                &format!("/services/settings/group/{}/{}", group_id, service_id),
            )
            .body(settings)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }

    pub async fn delete_user_settings(
        &self,
        username: &str,
        service_id: &str,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::DELETE,
                &format!("/services/settings/user/{}/{}", username, service_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }

    pub async fn delete_group_settings(
        &self,
        group_id: &GroupId,
        service_id: &str,
    ) -> Result<String, error::Error> {
        let response = self
            .request(
                Method::DELETE,
                &format!("/services/settings/group/{}/{}", group_id, service_id),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }
    // NetsBlox network capabilities
    pub async fn list_external_clients(&self) -> Result<Vec<ExternalClient>, error::Error> {
        let response = self
            .request(Method::GET, "/network/external")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ExternalClient>>().await.unwrap())
    }

    pub async fn list_networks(&self) -> Result<Vec<ProjectId>, error::Error> {
        let response = self
            .request(Method::GET, "/network/")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ProjectId>>().await.unwrap())
    }

    pub async fn get_room_state(&self, id: &ProjectId) -> Result<RoomState, error::Error> {
        let response = self
            .request(Method::GET, &format!("/network/id/{}", id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<RoomState>().await.unwrap())
    }

    pub async fn get_client_state(&self, client_id: &ClientId) -> Result<ClientInfo, error::Error> {
        let response = self
            .request(
                Method::GET,
                &format!("/network/{}/state", client_id.as_str()),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<ClientInfo>().await.unwrap())
    }

    pub async fn evict_occupant(&self, client_id: &ClientId) -> Result<(), error::Error> {
        let response = self
            .request(
                Method::POST,
                &format!("/network/clients/{}/evict", client_id.as_str()),
            )
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn connect(&self, address: &str) -> Result<MessageChannel, error::Error> {
        let response = self
            .request(Method::GET, "/configuration")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let config = response.json::<ClientConfig>().await.unwrap();

        let url = format!(
            "{}/network/{}/connect",
            self.cfg.url.replace("http", "ws"),
            config.client_id
        );
        let (ws_stream, _) = connect_async(&url).await.unwrap();

        let state = ClientStateData {
            state: ClientState::External(ExternalClientState {
                address: address.to_owned(),
                app_id: self.cfg.app_id.as_ref().unwrap().clone(),
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
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;

        Ok(MessageChannel {
            id: config.client_id,
            stream: ws_stream,
        })
    }

    // NetsBlox OAuth capabilities
    pub async fn add_oauth_client(
        &self,
        client: &oauth::CreateClientData,
    ) -> Result<oauth::CreatedClientData, error::Error> {
        let response = self
            .request(Method::POST, "/oauth/clients/")
            .json(&client)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<oauth::CreatedClientData>().await.unwrap())
    }

    pub async fn remove_oauth_client(&self, id: &oauth::ClientId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/oauth/clients/{}", id))
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_oauth_clients(&self) -> Result<Vec<oauth::Client>, error::Error> {
        let response = self
            .request(Method::GET, "/oauth/clients/")
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<oauth::Client>>().await.unwrap())
    }

    /// Asynchronously creates a new gallery.
    ///
    /// # Arguments
    ///
    /// * `data` - A reference to the `CreateGalleryData` struct.
    ///
    /// # Returns
    /// * `Result<common::Gallery, error::Error>` -
    /// * On success, returns an instance of the created `Gallery`.
    /// * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing
    /// the JSON response.
    /// * If server respondes with error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP POST request to the `/galleries/` endpoint
    /// with the provided `data`.
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a `Gallery` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `Gallery` object and returned.
    pub async fn create_gallery(&self, data: &CreateGalleryData) -> Result<Gallery, error::Error> {
        let response = self
            .request(Method::POST, "/galleries/")
            .json(&data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let gallery = response
            .json::<Gallery>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(gallery)
    }

    /// Asynchronously retrieves the galleries of a specified user.
    ///
    /// # Arguments
    ///
    /// * `owner` - A reference to a `String` containing the username or identifier
    /// of the gallery owner.
    ///
    /// # Returns
    ///
    /// * `Result<common::Gallery, error::Error>` -
    ///   * On success, returns an instance of the retrieved `Gallery`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing
    /// the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the `/galleries/user/{owner}` endpoint,
    /// where `{owner}` is replaced with the specified owner identifier.
    /// It then checks the response for any errors and attempts to parse the response body
    /// as JSON into a `Gallery` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `Gallery` object and returned.
    pub async fn view_galleries_with_name(&self, owner: &str) -> Result<Gallery, error::Error> {
        let url = format!("/galleries/user/{owner}");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let gallery = response
            .json::<Gallery>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(gallery)
    }

    /// Asynchronously retrieves a gallery by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the gallery
    /// to retrieve.
    ///
    /// # Returns
    ///
    /// * `Result<common::Gallery, error::Error>` -
    ///   * On success, returns an instance of the retrieved `Gallery`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing
    /// the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the `/galleries/id/{id}` endpoint, where `{id}` is replaced with the specified gallery ID.
    /// It then checks the response for any errors and attempts to parse the response
    /// body as JSON into a `Gallery` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `Gallery` object and returned.
    pub async fn view_gallery_with_id(&self, id: &GalleryId) -> Result<Gallery, error::Error> {
        let url = format!("/galleries/id/{id}");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let gallery = response
            .json::<Gallery>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(gallery)
    }

    /// Asynchronously updates an existing gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the gallery
    /// to update.
    /// * `data` - A reference to the `ChangeGalleryData` struct containing the new data
    /// for the gallery.
    ///
    /// # Returns
    ///
    /// * `Result<Gallery, error::Error>` -
    ///   * On success, returns an instance of the updated `Gallery`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing
    /// the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP PATCH request to the `/galleries/id/{id}` endpoint,
    /// where `{id}` is replaced with the specified gallery ID.
    /// It sends the `data` as JSON in the request body to update the gallery with
    /// the new information provided.
    /// It then checks the response for any errors and attempts to parse the response body
    /// as JSON into a `Gallery` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `Gallery` object and returned.
    pub async fn change_gallery(
        &self,
        id: &GalleryId,
        data: &ChangeGalleryData,
    ) -> Result<Gallery, error::Error> {
        let url = format!("/galleries/id/{id}");

        let response = self
            .request(Method::PATCH, &url)
            .json(&data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let gallery = response
            .json::<Gallery>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(gallery)
    }

    /// Asynchronously deletes an existing gallery by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    /// gallery to delete.
    ///
    /// # Returns
    ///
    /// * `Result<Gallery, error::Error>` -
    ///   * On success, returns an instance of the deleted `Gallery`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing \
    /// the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP DELETE request to the `/galleries/id/{id}` endpoint,
    /// where `{id}` is replaced with the specified gallery ID.
    /// It then checks the response for any errors and attempts to parse the response body
    /// as JSON into a `Gallery` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `Gallery` object and returned.
    pub async fn delete_gallery(&self, id: &GalleryId) -> Result<Gallery, error::Error> {
        let url = format!("/galleries/id/{id}");

        let response = self
            .request(Method::DELETE, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let gallery = response
            .json::<Gallery>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(gallery)
    }

    /// Asynchronously adds a new project to an existing gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery to add the project to.
    /// * `data` - A reference to the `CreateGalleryProjectData` struct containing
    ///   the data for the new project.
    ///
    /// # Returns
    ///
    /// * `Result<GalleryProjectMetadata, error::Error>` -
    ///   * On success, returns an instance of the created `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error
    ///   parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP POST request to the `/galleries/id/{id}`
    /// endpoint, where `{id}` is replaced with the specified gallery ID.
    /// It sends the `data` as JSON in the request body to add the new project to
    /// the gallery.
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a `GalleryProjectMetadata` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `GalleryProjectMetadata`
    /// object and returned.
    pub async fn add_gallery_project(
        &self,
        id: &GalleryId,
        data: &CreateGalleryProjectData,
    ) -> Result<GalleryProjectMetadata, error::Error> {
        let url = format!("/galleries/id/{id}");

        let response = self
            .request(Method::POST, &url)
            .json(&data)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let project = response
            .json::<GalleryProjectMetadata>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(project)
    }

    /// Asynchronously retrieves metadata for a specific project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the project
    /// within the gallery.
    ///
    /// # Returns
    ///
    /// * `Result<GalleryProjectMetadata, error::Error>` -
    ///   * On success, returns an instance of the retrieved `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error parsing
    /// the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the `/galleries/id/{id}/project/{prid}`
    /// endpoint, where `{id}` and `{prid}` are replaced with the specified gallery ID
    /// and project ID respectively.
    /// It then checks the response for any errors and attempts to parse the response body
    /// as JSON into a `GalleryProjectMetadata` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `GalleryProjectMetadata` object
    /// and returned.
    pub async fn view_gallery_project(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
    ) -> Result<GalleryProjectMetadata, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let project = response
            .json::<GalleryProjectMetadata>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(project)
    }

    /// Asynchronously retrieves the thumbnail of a specific project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of
    /// the project within the gallery.
    /// * `aspect_ratio` - An optional reference to an `f32` representing the desired
    /// aspect ratio of the thumbnail.
    ///
    /// # Returns
    ///
    /// * `Result<Bytes, error::Error>` -
    ///   * On success, returns the thumbnail as `Bytes`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the request.
    /// * `error::Error::ParseJsonFailedError` - Returned if there is an error
    /// parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the
    /// `/galleries/id/{id}/project/{prid}/thumbnail`
    /// endpoint, where `{id}` and `{prid}` are replaced with the specified gallery ID
    /// and project ID
    /// respectively. If an `aspect_ratio` is provided, it is appended as a query parameter.
    ///
    /// It then checks the response for any errors and attempts to retrieve the response
    /// body as `Bytes`.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, the thumbnail is returned as `Bytes`.
    pub async fn view_gallery_project_thumbnail(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
        aspect_ratio: &Option<f32>,
    ) -> Result<Bytes, error::Error> {
        let base = format!("/galleries/id/{id}/project/{prid}/thumbnail");

        let url = if let Some(ratio) = aspect_ratio {
            format!("{base}?aspect_ratio={ratio}")
        } else {
            base
        };

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let thumbnail = response
            .bytes()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(thumbnail)
    }
    /// Asynchronously retrieves all projects within a specified gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<GalleryProjectMetadata>, error::Error>` -
    ///   * On success, returns a vector of `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the `/galleries/id/{id}/projects`
    /// endpoint, where `{id}` is replaced with the specified gallery ID.
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a vector of `GalleryProjectMetadata`.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a vector of
    /// `GalleryProjectMetadata` and returned.
    pub async fn view_gallery_projects(
        &self,
        id: &GalleryId,
    ) -> Result<Vec<GalleryProjectMetadata>, error::Error> {
        let url = format!("/galleries/id/{id}/projects");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let projects = response
            .json::<Vec<GalleryProjectMetadata>>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(projects)
    }

    /// Asynchronously adds a new version to a project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the
    ///   project within the gallery.
    /// * `xml` - A string slice containing the XML data for the new version.
    ///
    /// # Returns
    ///
    /// * `Result<GalleryProjectMetadata, error::Error>` -
    ///   * On success, returns an instance of the updated `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP POST request to the `/galleries/id/{id}/project/{prid}`
    /// endpoint, where `{id}` and `{prid}` are replaced with the specified gallery ID
    /// and project ID respectively.
    /// It sends the `xml` data as JSON in the request body to add the new version
    /// to the project.
    ///
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a `GalleryProjectMetadata` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `GalleryProjectMetadata`
    /// object and returned.
    pub async fn add_gallery_project_version(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
        xml: &str,
    ) -> Result<GalleryProjectMetadata, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}");

        let response = self
            .request(Method::POST, &url)
            .json(xml)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let project = response
            .json::<GalleryProjectMetadata>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(project)
    }

    /// Asynchronously retrieves the XML data for a specific project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the
    ///   project within the gallery.
    ///
    /// # Returns
    ///
    /// * `Result<String, error::Error>` -
    ///   * On success, returns the XML data as a `String`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the response text.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the `/galleries/id/{id}/project/{prid}/xml`
    /// endpoint, where `{id}` and `{prid}` are replaced with the specified gallery ID
    /// and project ID respectively.
    /// It then checks the response for any errors and attempts to retrieve the
    /// response body as text.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, the XML data is returned as a `String`.
    pub async fn view_gallery_project_xml(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
    ) -> Result<String, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}/xml");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let xml = response
            .text()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(xml)
    }

    /// Asynchronously retrieves the XML data for a specific version of a project
    /// within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the
    ///   project within the gallery.
    /// * `version` - A reference to a `usize` representing the version number of
    ///   the project.
    ///
    /// # Returns
    ///
    /// * `Result<String, error::Error>` -
    ///   * On success, returns the XML data as a `String`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the response text.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP GET request to the
    /// `/galleries/id/{id}/project/{prid}/version/{version}/xml` endpoint, where
    /// `{id}`, `{prid}`, and `{version}` are replaced with the specified gallery
    /// ID, project ID, and version number respectively.
    /// It then checks the response for any errors and attempts to retrieve the
    /// response body as text.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, the XML data for the specified version is
    /// returned as a `String`.
    pub async fn view_gallery_project_xml_version(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
        version: &usize,
    ) -> Result<String, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}/version/{version}/xml");

        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let xml = response
            .text()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(xml)
    }

    /// Asynchronously deletes a specific project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the
    ///   project within the gallery.
    ///
    /// # Returns
    ///
    /// * `Result<GalleryProjectMetadata, error::Error>` -
    ///   * On success, returns the metadata of the deleted `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP DELETE request to the
    /// `/galleries/id/{id}/project/{prid}` endpoint, where `{id}` and `{prid}` are
    /// replaced with the specified gallery ID and project ID respectively.
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a `GalleryProjectMetadata` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `GalleryProjectMetadata`
    /// object and returned.
    pub async fn delete_gallery_project(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
    ) -> Result<GalleryProjectMetadata, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}");

        let response = self
            .request(Method::DELETE, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let project = response
            .json::<GalleryProjectMetadata>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(project)
    }

    /// Asynchronously deletes a specific version of a project within a gallery.
    ///
    /// # Arguments
    ///
    /// * `id` - A reference to a `GalleryId` struct representing the ID of the
    ///   gallery.
    /// * `prid` - A reference to a `ProjectId` struct representing the ID of the
    ///   project within the gallery.
    /// * `version` - A reference to a `usize` representing the version number of
    ///   the project to delete.
    ///
    /// # Returns
    ///
    /// * `Result<GalleryProjectMetadata, error::Error>` -
    ///   * On success, returns the metadata of the updated `GalleryProjectMetadata`.
    ///   * On failure, returns an `error::Error`.
    ///
    /// # Errors
    ///
    /// * `error::Error::RequestError` - Returned if there is an error sending the
    ///   request.
    /// * `error::Error::ParseResponseFailedError` - Returned if there is an error
    ///   parsing the JSON response.
    /// * If server responds with an error, we return the error.
    ///
    /// # Notes
    ///
    /// This function makes an HTTP DELETE request to the
    /// `/galleries/id/{id}/project/{prid}/version/{version}` endpoint, where `{id}`,
    /// `{prid}`, and `{version}` are replaced with the specified gallery ID, project
    /// ID, and version number respectively.
    /// It then checks the response for any errors and attempts to parse the
    /// response body as JSON into a `GalleryProjectMetadata` struct.
    ///
    /// The `check_response` function is called to handle potential HTTP errors.
    /// If the response is successful, it is parsed into a `GalleryProjectMetadata`
    /// object and returned.
    pub async fn delete_gallery_project_version(
        &self,
        id: &GalleryId,
        prid: &ProjectId,
        version: &usize,
    ) -> Result<GalleryProjectMetadata, error::Error> {
        let url = format!("/galleries/id/{id}/project/{prid}/version/{version}");

        let response = self
            .request(Method::DELETE, &url)
            .send()
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;

        let response = check_response(response).await?;

        let project = response
            .json::<GalleryProjectMetadata>()
            .await
            .map_err(|e| error::Error::ParseResponseFailedError(e.to_string()))?;

        Ok(project)
    }
}

pub struct MessageChannel {
    pub id: String,
    pub stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl MessageChannel {
    // TODO: do we need a method for sending other types?
    // TODO: sending a generic struct (implementing Deserialize)
    pub async fn send_json(
        &mut self,
        addr: &str,
        r#type: &str,
        data: &Value,
    ) -> Result<(), error::Error> {
        let msg = json!({
            "type": "message",
            "dstId": addr,
            "msgType": r#type,
            "content": data
        });
        let msg_text = serde_json::to_string(&msg).unwrap();
        self.stream
            .send(Message::Text(msg_text))
            .await
            .map_err(|e| error::Error::WebSocketError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed\n  left: 4\n right: 5")]
    fn it_does_not_work() {
        let result = 2 + 2;
        assert_eq!(result, 5);
    }
}
