pub mod core;
pub mod error;

use crate::core::*;
use futures_util::stream::SplitStream;
use netsblox_core::{CreateGroupData, UpdateGroupData};
use reqwest::{self, Method, RequestBuilder, Response};
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

async fn check_response(response: Response) -> Result<Response, error::Error> {
    let status_code = response.status().as_u16();
    let is_error = status_code > 399;
    if is_error {
        let msg = response
            .text()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        match status_code {
            400 => Err(error::Error::BadRequestError(msg)),
            401 => Err(error::Error::LoginRequiredError),
            403 => Err(error::Error::PermissionsError(msg)),
            404 => Err(error::Error::NotFoundError(msg)),
            _ => panic!("Unknown status code"), // FIXME: Use error instead?
        }
    } else {
        Ok(response)
    }
}

pub type Token = String;
pub async fn login(cfg: &mut Config, credentials: &LoginRequest) -> Result<(), error::Error> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/users/login", cfg.url))
        .json(&credentials)
        .send()
        .await
        .map_err(|err| error::Error::RequestError(err))?;

    let response = check_response(response).await?;
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

    // User management
    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: Option<&str>, // TODO: Make these CreateUserOptions
        group_id: Option<&str>,
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
            .map_err(|err| error::Error::RequestError(err))?;

        println!(
            "status {} {}",
            response.status(),
            response.text().await.unwrap()
        );
        Ok(())
        // TODO: return the user data?
    }

    pub async fn list_users(&self) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(Method::GET, "/users/")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn delete_user(&self, username: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/delete", username))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_user(&self, username: &str) -> Result<User, error::Error> {
        let response = self
            .request(Method::GET, &format!("/users/{}", username))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn link_account(
        &self,
        username: &str,
        credentials: &core::Credentials,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/link/", username))
            .json(&credentials)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn ban_user(&self, username: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/users/{}/ban", username))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    // Project management
    pub async fn list_projects(&self, owner: &str) -> Result<Vec<ProjectMetadata>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/projects/user/{}", &owner))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn rename_role(
        &self,
        id: &ProjectId,
        role_id: &str,
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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}", id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn delete_role(&self, id: &ProjectId, role_id: &str) -> Result<(), error::Error> {
        let response = self
            .request(Method::DELETE, &format!("/projects/id/{}/{}", id, role_id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn publish_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/publish", id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;

        Ok(())
    }

    pub async fn unpublish_project(&self, id: &ProjectId) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/projects/id/{}/unpublish", id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Project>().await.unwrap())
    }

    pub async fn get_role(
        &self,
        id: &ProjectId,
        role_id: &str,
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
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<RoleData>().await.unwrap())
    }

    // Project collaborators
    pub async fn list_collaborators(&self, project_id: &str) -> Result<Vec<String>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/id/{}/collaborators/", project_id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<String>>().await.unwrap())
    }

    pub async fn list_online_friends(&self, username: &str) -> Result<Vec<String>, error::Error> {
        let path = &format!("/friends/{}/online", username);
        let response = self
            .request(Method::GET, path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unfriend(&self, username: &str, friend: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unfriend/{}", username, friend);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn block_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/block/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn unblock_user(&self, username: &str, other_user: &str) -> Result<(), error::Error> {
        let path = format!("/friends/{}/unblock/{}", username, other_user);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_submitted_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/mod/pending")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_public_libraries(&self) -> Result<Vec<LibraryMetadata>, error::Error> {
        let response = self
            .request(Method::GET, "/libraries/community")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<LibraryMetadata>>().await.unwrap())
    }

    pub async fn get_library(&self, username: &str, name: &str) -> Result<String, error::Error> {
        let path = format!("/libraries/user/{}/{}", username, name); // TODO: URI escape?
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}", username, library);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn publish_library(&self, username: &str, library: &str) -> Result<(), error::Error> {
        let path = format!("/libraries/user/{}/{}/publish", username, library);
        let response = self
            .request(Method::POST, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn approve_library(
        &self,
        username: &str,
        library: &str,
        state: &LibraryPublishState,
    ) -> Result<(), error::Error> {
        let path = format!("/libraries/mod/{}/{}", username, library);
        let response = self
            .request(Method::POST, &path)
            .json(&state)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    // Group management
    pub async fn list_groups(&self, username: &str) -> Result<Vec<Group>, error::Error> {
        let path = format!("/groups/user/{}", username);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<Group>>().await.unwrap())
    }

    pub async fn create_group(&self, owner: &str, name: &str) -> Result<(), error::Error> {
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
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn delete_group(&self, id: &str) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::DELETE, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn list_members(&self, id: &str) -> Result<Vec<User>, error::Error> {
        let path = format!("/groups/id/{}/members", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;
        Ok(response.json::<Vec<User>>().await.unwrap())
    }

    pub async fn rename_group(&self, id: &str, name: &str) -> Result<(), error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::PATCH, &path)
            .json(&UpdateGroupData {
                name: name.to_owned(),
            })
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn view_group(&self, id: &str) -> Result<Group, error::Error> {
        let path = format!("/groups/id/{}", id);
        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Group>().await.unwrap())
    }

    // Service host management
    pub async fn list_user_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/user/{}", username))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn list_group_hosts(&self, group_id: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/group/{}", group_id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn list_hosts(&self, username: &str) -> Result<Vec<ServiceHost>, error::Error> {
        let response = self
            .request(Method::GET, &format!("/service-hosts/all/{}", username))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ServiceHost>>().await.unwrap())
    }

    pub async fn set_user_hosts(
        &self,
        username: &str,
        hosts: Vec<ServiceHost>,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/service-hosts/user/{}", username))
            .json(&hosts)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn set_group_hosts(
        &self,
        group_id: &str,
        hosts: Vec<ServiceHost>,
    ) -> Result<(), error::Error> {
        let response = self
            .request(Method::POST, &format!("/service-hosts/group/{}", group_id))
            .json(&hosts)
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    // NetsBlox network capabilities
    pub async fn list_external_clients(&self) -> Result<Vec<ExternalClient>, error::Error> {
        let response = self
            .request(Method::GET, "/network/external")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ExternalClient>>().await.unwrap())
    }

    pub async fn list_networks(&self) -> Result<Vec<ProjectId>, error::Error> {
        let response = self
            .request(Method::GET, "/network/")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<Vec<ProjectId>>().await.unwrap())
    }

    pub async fn get_room_state(&self, id: &ProjectId) -> Result<RoomState, error::Error> {
        let response = self
            .request(Method::GET, &format!("/network/id/{}", id))
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        Ok(response.json::<RoomState>().await.unwrap())
    }

    pub async fn evict_occupant(&self, client_id: &ClientID) -> Result<(), error::Error> {
        let response = self
            .request(
                Method::POST,
                &format!("/network/clients/{}/evict", client_id),
            )
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        check_response(response).await?;
        Ok(())
    }

    pub async fn connect(&self, address: &str) -> Result<MessageChannel, error::Error> {
        let response = self
            .request(Method::GET, "/configuration")
            .send()
            .await
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

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
            .map_err(|err| error::Error::RequestError(err))?;

        let response = check_response(response).await?;

        println!("status {}", response.status());

        Ok(MessageChannel {
            id: config.client_id,
            stream: ws_stream,
        })
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
