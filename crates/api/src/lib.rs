pub mod common;
pub mod error;

use crate::common::*;
use futures_util::SinkExt;
use netsblox_api_common::{
    CreateGroupData, CreateMagicLinkData, Name, ProjectName, ServiceHostScope, UpdateGroupData,
};
use reqwest::{self, Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
pub use serde_json;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub app_id: Option<AppId>,
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
            url: "https://cloud.netsblox.org".to_owned(),
        }
    }
}

async fn check_response(response: Response) -> Result<Response, error::Error> {
    let status_code = response.status().as_u16();
    let is_error = status_code > 399;
    if is_error {
        let msg = response.text().await.map_err(error::Error::RequestError)?;

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

pub type Token = String;
pub async fn login(mut cfg: Config, credentials: &LoginRequest) -> Result<Config, error::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/users/login", cfg.url))
        .json(&credentials)
        .send()
        .await
        .map_err(error::Error::RequestError)?;

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

#[derive(Serialize)]
struct UserData<'a> {
    username: &'a str,
    email: &'a str,
    role: &'a UserRole,
    group_id: Option<&'a GroupId>,
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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_user(&self, username: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/delete", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_user(&self, username: &str) -> Result<User, error::Error> {
        let response = self
            .request(Method::GET, &format!("/users/{}", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn ban_user(&self, username: &str) -> Result<BannedAccount, error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/ban", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.json::<BannedAccount>().await.unwrap())
    }

    pub async fn unban_user(&self, username: &str) -> Result<BannedAccount, error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/unban", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.json::<ProjectMetadata>().await.unwrap())
    }

    pub async fn list_projects(&self, owner: &str) -> Result<Vec<ProjectMetadata>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/projects/user/{}", &owner))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<ProjectMetadata>().await.unwrap())
    }

    pub async fn rename_project(&self, id: &ProjectId, name: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::PATCH, &format!("/projects/id/{}", &id))
            .json(&UpdateProjectData {
                name: ProjectName::new(name),
                client_id: None,
            })
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
                name: RoleName::new(name),
                client_id: None,
            })
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}", id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_role(&self, id: &ProjectId, role_id: &RoleId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}/{}", id, role_id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn publish_project(&self, id: &ProjectId) -> Result<PublishState, error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/publish", id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<PublishState>().await.unwrap())
    }

    pub async fn unpublish_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/unpublish", id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<RoleData>().await.unwrap())
    }

    // Project collaborators
    pub async fn list_collaborators(&self, project_id: &str) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/id/{}/collaborators/", project_id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn list_online_friends(&self, username: &str) -> Result<Vec<String>, error::Error> {
        let path = &format!("/friends/{}/online", username);
        let response = self
            .request(Method::GET, path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unfriend(&self, username: &str, friend: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unfriend/{}", username, friend);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn block_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/block/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unblock_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unblock/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_submitted_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/mod/pending")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_public_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/community/")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_library(&self, username: &str, name: &str) -> Result<String, error::Error> {
        let path = format!("/libraries/user/{}/{}", username, name); // TODO: URI escape?
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}", username, library);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn publish_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}/publish", username, library);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<Group>>().await.unwrap())
    }

    pub async fn create_group(&self, owner: &str, name: &str) -> Result<(), error::Error> {
        let path = format!("/groups/user/{}/", owner);
        let group = CreateGroupData {
            name: Name::new(name),
            services_hosts: None,
        };
        let response = self
            .request(Method::POST, &path)
            .json(&group)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_group(&self, id: &GroupId) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_members(&self, id: &GroupId) -> Result<Vec<User>, error::Error> {
        let path = format!("/groups/id/{}/members", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<User>>().await.unwrap())
    }

    pub async fn rename_group(&self, id: &GroupId, name: &str) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::PATCH, &path)
            .json(&UpdateGroupData {
                name: Name::new(name),
            })
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_group(&self, id: &GroupId) -> Result<Group, error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Group>().await.unwrap())
    }

    // Service host management
    pub async fn list_user_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/services/hosts/user/{}", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn list_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/services/hosts/all/{}", username))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_authorized_hosts(&self) -> Result<Vec<AuthorizedServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, "/services/hosts/authorized/")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;
        Ok(response.text().await.unwrap())
    }
    // NetsBlox network capabilities
    pub async fn list_external_clients(&self) -> Result<Vec<ExternalClient>, error::Error> {
        let response = self
            .request(Method::GET, "/network/external")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ExternalClient>>().await.unwrap())
    }

    pub async fn list_networks(&self) -> Result<Vec<ProjectId>, error::Error> {
        let response = self
            .request(Method::GET, "/network/")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ProjectId>>().await.unwrap())
    }

    pub async fn get_room_state(&self, id: &ProjectId) -> Result<RoomState, error::Error> {
        let response = self
            .request(Method::GET, &format!("/network/id/{}", id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn connect(&self, address: &str) -> Result<MessageChannel, error::Error> {
        let response = self
            .request(Method::GET, "/configuration")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

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
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<oauth::CreatedClientData>().await.unwrap())
    }

    pub async fn remove_oauth_client(&self, id: &oauth::ClientId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/oauth/clients/{}", id))
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_oauth_clients(&self) -> Result<Vec<oauth::Client>, error::Error> {
        let response = self
            .request(Method::GET, "/oauth/clients/")
            .send()
            .await
            .map_err(error::Error::RequestError)?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<oauth::Client>>().await.unwrap())
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
            .map_err(error::Error::WebSocketSendError)?;

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
}
