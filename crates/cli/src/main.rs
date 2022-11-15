static APP_NAME: &str = "netsblox-cli";

use std::fs;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use inquire::{Confirm, Password, PasswordDisplayMode};
use netsblox_api::core::{
    oauth, ClientID, CreateProjectData, Credentials, FriendLinkState, InvitationState,
    LinkedAccount, ProjectId, PublishState, RoleData, SaveState, ServiceHost, UserRole,
};
use netsblox_api::{Client, Config};
use std::path::Path;
use xmlparser::{Token, Tokenizer};

#[derive(Subcommand, Debug)]
enum Users {
    /// Create a new NetsBlox user
    Create {
        username: String,
        email: String,
        /// Password for new user. If unset, user will need to manually reset password before logging in
        #[clap(short, long)]
        password: Option<String>,
        /// Make the new user a member of the given group
        #[clap(short, long)]
        group: Option<String>,
        /// Perform the operation as this user
        #[clap(short, long)]
        user: Option<String>,
        /// Set the user role (eg, admin, moderator)
        #[clap(short, long, default_value = "user")]
        role: UserRole,
    },
    /// Delete an existing NetsBlox account
    Delete {
        username: String,
        /// Skip confirmation prompts and delete the user
        #[clap(short, long)]
        no_confirm: bool,
    },
    /// View the current user
    View {
        /// Perform the operation as this user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Change the current user's password
    SetPassword {
        password: String,
        /// Perform the operation as this user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// List NetsBlox users
    List, // TODO: add verbose option?
    /// Ban a given user. Email address will also be blacklisted
    Ban {
        /// NetsBlox user to ban
        username: String,
    },
    /// Link an account to a Snap! account (for login)
    Link {
        /// Snap! username to link to NetsBlox account
        username: String,
        /// Snap! password
        password: String,
        // #[clap(short, long, default_value = "Snap")]
        // strategy: String,
        /// Perform the operation as this user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Unlink a Snap! account from a NetsBlox account
    Unlink {
        /// Snap! username to unlink from NetsBlox account
        username: String,
        // #[clap(short, long, default_value = "Snap!")]
        // strategy: String,
        /// Perform the operation as this user
        #[clap(short, long)]
        user: Option<String>,
    },
}

/// Manage projects (or roles)
#[derive(Subcommand, Debug)]
enum Projects {
    /// Import a project into NetsBlox
    Import {
        /// The path to the project to import
        filename: String,
        /// Project name (default is the filename)
        #[clap(short, long)]
        name: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Export a project from NetsBlox
    Export {
        /// Name of project to export
        project: String,
        /// Export a single role from the project instead
        #[clap(short, long)]
        role: Option<String>,
        /// Include unsaved changes (from opened projects)
        #[clap(short, long)]
        latest: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// List the user's projects
    List {
        /// List the projects shared with the current user
        #[clap(short, long)]
        shared: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Publish a project
    Publish {
        /// Name of project to publish
        project: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Unpublish a project
    Unpublish {
        /// Name of project to unpublish
        project: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Delete a project or role
    Delete {
        project: String,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Rename a project or role
    Rename {
        project: String,
        new_name: String,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Invite a collaborator to share the project
    InviteCollaborator {
        project: String,
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// List collaboration invitations
    ListInvites {
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Accept a collaboration invitation
    AcceptInvite {
        project: String,
        username: String,

        #[clap(long)]
        reject: bool,

        #[clap(short, long)]
        user: Option<String>,
    },
    /// List all collaborators on a given project
    ListCollaborators {
        project: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Remove a collaborator from a project
    RemoveCollaborator {
        project: String,
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
}

/// Register (and authorize) NetsBlox service/RPC providers
#[derive(Subcommand, Debug)]
enum ServiceHosts {
    /// List service hosts registered for a given user/group or all authorized hosts
    List {
        /// List all authorized service hosts. Overrides other options.
        #[clap(long)]
        authorized: bool,
        /// List service hosts registered to the user (ignore any hosts registered to groups)
        #[clap(long)]
        user_only: bool,
        /// List service hosts registered to the given group
        #[clap(short, long)]
        group: Option<String>,
        /// Perform this action on behalf of another user
        #[clap(short, long)]
        user: Option<String>,
    },
    Register {
        url: String,
        categories: String, // TODO: Should this be optional?
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unregister {
        url: String,
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Authorize a service host to send messages and query user info from NetsBlox
    Authorize {
        url: String,
        client_id: String,
        /// Enable this service host for all users
        #[clap(short, long)]
        public: bool,
    },
    /// Revoke the service host's authorization
    Unauthorize { url: String },
}

#[derive(Subcommand, Debug)]
enum ServiceSettings {
    /// List hosts that have custom settings for the given user/group
    List {
        /// List hosts that have custom settings for the given group
        #[clap(short, long)]
        group: Option<String>,
        /// Perform this action on behalf of another user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// View the service settings for a given host
    View {
        /// Service host ID
        host: String,
        /// View settings for the given group
        #[clap(short, long)]
        group: Option<String>,
        /// List all the available settings (user, member, groups) for the user
        #[clap(short, long)]
        all: bool,
        /// Perform this action on behalf of another user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Delete the service settings for a given user/group
    Delete {
        /// Service host ID
        host: String,
        /// Delete settings for the given group
        #[clap(short, long)]
        group: Option<String>,
        /// Perform this action on behalf of another user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Set the service settings for a given user/group. Overwrites existing settings.
    Set {
        /// Service host ID
        host: String,
        /// New settings for the given user/group
        settings: String,
        /// Set settings for the given group
        #[clap(short, long)]
        group: Option<String>,
        /// Perform this action on behalf of another user
        #[clap(short, long)]
        user: Option<String>,
    },
}

/// Manage libraries saved to the cloud
#[derive(Subcommand, Debug)]
enum Libraries {
    /// List available libraries. Lists own libraries by default.
    List {
        /// List community libraries
        #[clap(short, long)]
        community: bool,
        /// List libraries that require moderator approval for publishing
        #[clap(short, long)]
        approval_needed: bool,
        /// List libraries owned by the given user
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Import a file of exported blocks as a library
    Import {
        /// The path to the exported blocks to import
        filename: String,
        /// Notes describing the new library
        #[clap(long, default_value = "")]
        notes: String,
        /// Name of the library (filename used by default)
        #[clap(short, long)]
        name: Option<String>,
        /// User to save the library for (logged in user by default)
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Download a library from the cloud
    Export {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Delete a library from the cloud
    Delete {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Make library publicly available
    Publish {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Make a public library private again
    Unpublish {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Approve libraries with potentially questionable content
    Approve {
        library: String,
        #[clap(long)]
        reject: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
}

/// Manage OAuth registered clients
#[derive(Subcommand, Debug)]
enum Oauth {
    /// Authorize an OAuth client for a user
    Authorize {
        client_id: oauth::ClientId,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Revoke authorization for an OAuth client
    Revoke {
        client: String, // TODO: should we use an ID or name?
        #[clap(short, long)]
        user: Option<String>,
    },
    /// List all OAuth clients
    List,
    /// Register new OAuth client with NetsBlox
    AddClient { name: String },
    /// Remove registered OAuth client from NetsBlox
    RemoveClient { id: oauth::ClientId },
}

/// Connect to the NetsBlox network
#[derive(Subcommand, Debug)]
enum Network {
    /// List the active NetsBlox rooms or external clients
    List {
        #[clap(short, long)]
        external: bool,
    },
    /// View the network state of a given project
    View {
        project: String,
        /// Interpret <project> argument as a project ID rather than name
        #[clap(short, long)]
        as_id: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// View the state of a given connected client
    ViewClient { client_id: ClientID },
    /// Connect to NetsBlox and listen for messages
    Connect {
        #[clap(short, long, default_value = "project")]
        address: String,
    },
    /// Evict a client from their current role
    Evict { client_id: ClientID },
    /// Send a NetsBlox message
    Send {
        /// Address of the intended recipient
        address: String,
        /// Message body to send (JSON)
        #[clap(short, long, default_value = "{}")]
        data: String,
        /// Message type to send
        #[clap(short, long, default_value = "message")]
        r#type: String,
    },
}

#[derive(Subcommand, Debug)]
enum Groups {
    Create {
        name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    List {
        #[clap(short, long)]
        user: Option<String>,
    },
    View {
        group: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Delete {
        group: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Members {
        group: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Rename {
        group: String,
        new_name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum Friends {
    List {
        #[clap(short, long)]
        online: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    Remove {
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Block {
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unblock {
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    ListInvites {
        #[clap(short, long)]
        user: Option<String>,
    },
    SendInvite {
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    AcceptInvite {
        sender: String,
        #[clap(long)]
        reject: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
}

#[derive(Parser, Debug)]
struct UserCommand {
    #[clap(subcommand)]
    subcmd: Users,
}

#[derive(Parser, Debug)]
struct ProjectCommand {
    #[clap(subcommand)]
    subcmd: Projects,
}

#[derive(Parser, Debug)]
struct NetworkCommand {
    #[clap(subcommand)]
    subcmd: Network,
}

#[derive(Parser, Debug)]
struct FriendCommand {
    #[clap(subcommand)]
    subcmd: Friends,
}

#[derive(Parser, Debug)]
struct GroupCommand {
    #[clap(subcommand)]
    subcmd: Groups,
}

#[derive(Parser, Debug)]
struct ServiceHostCommand {
    #[clap(subcommand)]
    subcmd: ServiceHosts,
}

#[derive(Parser, Debug)]
struct ServiceSettingsCommand {
    #[clap(subcommand)]
    subcmd: ServiceSettings,
}

#[derive(Parser, Debug)]
struct LibraryCommand {
    #[clap(subcommand)]
    subcmd: Libraries,
}

#[derive(Parser, Debug)]
struct OauthCommand {
    #[clap(subcommand)]
    subcmd: Oauth,
}

#[derive(Parser, Debug)]
enum Command {
    Login,
    Logout,
    Users(UserCommand),
    Projects(ProjectCommand),
    Network(NetworkCommand),
    Groups(GroupCommand),
    Friends(FriendCommand),
    ServiceHosts(ServiceHostCommand),
    ServiceSettings(ServiceSettingsCommand),
    Libraries(LibraryCommand),
    Oauth(OauthCommand),
}

#[derive(Parser, Debug)]
struct Cli {
    #[clap(subcommand)]
    cmd: Command,
}

fn prompt_credentials() -> (String, String, bool) {
    // FIXME: can't delete w/ backspace???
    let use_snap = inquire::Confirm::new("Would you like to login using Snap?")
        .with_default(false)
        .prompt()
        .expect("Unable to prompt for credentials");

    let username = inquire::Text::new("Username:")
        .prompt()
        .expect("Unable to prompt username");

    let password = Password::new("Password:")
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()
        .expect("Unable to prompt password");

    (username, password, use_snap)
}

fn get_current_user(cfg: &Config) -> String {
    cfg.username.as_ref().unwrap().clone()
}

#[tokio::main]
async fn main() {
    let mut cfg: Config = confy::load(&APP_NAME).expect("Unable to load configuration.");
    cfg.app_id = Some("NetsBloxCLI".to_owned());

    let args = Cli::parse();
    if let Err(err) = do_command(cfg, args).await {
        let code = match err {
            netsblox_api::error::Error::RequestError(..) => exitcode::NOHOST,
            _ => exitcode::USAGE,
        };
        eprintln!("{}", err);
        std::process::exit(code);
    }
}

async fn do_command(mut cfg: Config, args: Cli) -> Result<(), netsblox_api::error::Error> {
    let is_logged_in = !(cfg.token.is_none() || cfg.username.is_none());
    let login_required = match &args.cmd {
        Command::Login => true,
        Command::Logout => false,
        Command::Users(cmd) => match &cmd.subcmd {
            Users::Create { .. } => false,
            _ => !is_logged_in,
        },
        _ => !is_logged_in,
    };

    if login_required {
        let (username, password, use_snap) = prompt_credentials();
        let credentials = if use_snap {
            Credentials::Snap { username, password }
        } else {
            Credentials::NetsBlox { username, password }
        };
        let request = netsblox_api::core::LoginRequest {
            credentials,
            client_id: None,
        };
        netsblox_api::login(&mut cfg, &request)
            .await
            .expect("Login failed");

        confy::store(&APP_NAME, &cfg).expect("Unable to save configuration file.");
    }
    let client = Client::new(cfg.clone());

    match &args.cmd {
        Command::Login { .. } => {}
        Command::Logout => {
            cfg.token = None;
            cfg.username = None;
            confy::store(&APP_NAME, &cfg).expect("Unable to save configuration file.");
        }
        Command::Users(cmd) => match &cmd.subcmd {
            Users::Create {
                username,
                email,
                password,
                role,
                group,
                user,
            } => {
                let group_id = if let Some(group_name) = group {
                    let username = user.clone().unwrap_or(get_current_user(&cfg));
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };

                client
                    .create_user(
                        &username,
                        email,
                        password.as_deref(),
                        group_id.as_deref(),
                        role.to_owned(),
                    )
                    .await?;
            }
            Users::SetPassword { password, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                client.set_password(&username, &password).await?;
            }
            Users::List => {
                for user in client.list_users().await? {
                    println!("{}", user.username);
                }
            }
            Users::Delete {
                username,
                no_confirm,
            } => {
                let confirmed = if *no_confirm {
                    true
                } else {
                    Confirm::new(&format!("Are you sure you want to delete {}?", username))
                        .prompt()
                        .unwrap_or(false)
                };
                if confirmed {
                    client.delete_user(&username).await?;
                    println!("deleted {}", username);
                }
            }
            Users::View { user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let user = client.view_user(&username).await?;
                println!("{:?}", user);
            }
            Users::Link {
                username,
                password,
                user,
            } => {
                let as_user = user.clone().unwrap_or(get_current_user(&cfg));
                let creds = netsblox_api::core::Credentials::Snap {
                    username: username.to_owned(),
                    password: password.to_owned(),
                };
                client.link_account(&as_user, &creds).await?;
            }
            Users::Unlink { username, user } => {
                let as_user = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let account = LinkedAccount {
                    username: username.to_owned(),
                    strategy: "snap".to_owned(), // FIXME: add to linked account impl?
                };
                client.unlink_account(&as_user, &account).await?;
            }
            Users::Ban { username } => {
                client.ban_user(username).await?;
            }
        },
        Command::Projects(cmd) => match &cmd.subcmd {
            Projects::Import {
                filename,
                name,
                user,
            } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let project_xml = fs::read_to_string(filename).expect("Unable to read file");

                let mut found_role = false;
                let mut role_spans: Vec<RoleSpan> = Vec::new();
                let mut role_start = None;
                let mut media_start = None;
                let mut role_name: Option<&str> = None;
                for token in Tokenizer::from(project_xml.as_str()) {
                    match token {
                        Ok(Token::ElementStart { local, .. }) => {
                            let is_role = local.as_str() == "role";
                            if found_role {
                                role_start = Some(local.start() - 1);
                            }

                            found_role = is_role;

                            let is_media = local.as_str() == "media";
                            if is_media {
                                media_start = Some(local.start() - 1);
                            }
                        }
                        Ok(Token::ElementEnd { span, .. }) => {
                            if span.as_str().contains("media") {
                                let media_end = span.end();
                                if let (Some(name), Some(start), Some(media_start), end) =
                                    (role_name, role_start, media_start, media_end)
                                {
                                    role_spans.push(RoleSpan::new(
                                        name.to_owned(),
                                        start,
                                        media_start,
                                        end,
                                    ));
                                }
                            }
                        }
                        Ok(Token::Attribute { local, value, .. }) => {
                            if found_role && local.as_str() == "name" {
                                role_name = Some(value.as_str());
                            }
                        }
                        _ => {}
                    }
                }
                let roles: Vec<_> = role_spans
                    .into_iter()
                    .map(|rspan| rspan.into_role(&project_xml))
                    .collect();

                let project_data = CreateProjectData {
                    owner: Some(username),
                    name: name.to_owned().unwrap_or_else(|| {
                        Path::new(filename)
                            .file_stem()
                            .expect("Could not determine default name. Try passing --name")
                            .to_str()
                            .unwrap()
                            .to_owned()
                    }),
                    roles: Some(roles),
                    save_state: Some(SaveState::SAVED),
                    client_id: None,
                };
                client.create_project(&project_data).await?;
            }
            Projects::Export {
                project,
                role,
                latest,
                user,
            } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let metadata = client.get_project_metadata(&username, project).await?;
                let project_id = metadata.id;
                let xml = if let Some(role) = role {
                    let role_id = metadata
                        .roles
                        .into_iter()
                        .find(|(_id, role_md)| role_md.name == *role)
                        .map(|(id, _role_md)| id)
                        .expect("Role not found");

                    client
                        .get_role(&project_id, &role_id, latest)
                        .await?
                        .to_xml()
                } else {
                    client.get_project(&project_id, latest).await?.to_xml()
                };
                println!("{}", xml);
            }
            Projects::List { user, shared } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let projects = if *shared {
                    client.list_shared_projects(&username).await?
                } else {
                    client.list_projects(&username).await?
                };

                for project in projects {
                    println!("{:?}", project);
                }
            }
            Projects::Publish { project, user } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let metadata = client.get_project_metadata(&username, &project).await?;
                let project_id = metadata.id;

                client.publish_project(&project_id).await?;
                // TODO: add moderation here, too?
            }
            Projects::Unpublish { project, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&username, &project).await?;
                let project_id = metadata.id;

                client.unpublish_project(&project_id).await?;
            }
            Projects::InviteCollaborator {
                project,
                username,
                user,
            } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&owner, &project).await?;
                let project_id = metadata.id;
                client.invite_collaborator(&project_id, &username).await?;
            }
            Projects::ListInvites { user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let invites = client.list_collaboration_invites(&username).await?;
                for invite in invites {
                    println!("{:?}", invite);
                }
            }
            Projects::AcceptInvite {
                project,
                username,
                reject,
                user,
            } => {
                let receiver = user.clone().unwrap_or(get_current_user(&cfg));
                let invites = client.list_collaboration_invites(&receiver).await?;
                let project_id = client.get_project_metadata(&username, &project).await?.id;
                let invite = invites
                    .iter()
                    .find(|inv| inv.sender == *username && inv.project_id == project_id)
                    .expect("Invitation not found.");

                let state = if *reject {
                    InvitationState::REJECTED
                } else {
                    InvitationState::ACCEPTED
                };
                client
                    .respond_to_collaboration_invite(&invite.id, &state)
                    .await?;
            }
            Projects::ListCollaborators { project, user } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&owner, &project).await?;
                for user in metadata.collaborators {
                    println!("{}", user);
                }
            }
            Projects::RemoveCollaborator {
                project,
                username,
                user,
            } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&owner, &project).await?;
                client.remove_collaborator(&metadata.id, &username).await?;
            }
            Projects::Delete {
                project,
                role,
                user,
            } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&owner, &project).await?;
                if let Some(role_name) = role {
                    let role_id = metadata
                        .roles
                        .into_iter()
                        .find(|(_id, role)| role.name == *role_name)
                        .map(|(id, _role)| id)
                        .expect("Role not found.");

                    client.delete_role(&metadata.id, &role_id).await?;
                } else {
                    client.delete_project(&metadata.id).await?;
                }
            }
            Projects::Rename {
                project,
                new_name,
                role,
                user,
            } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                let metadata = client.get_project_metadata(&owner, &project).await?;
                if let Some(role_name) = role {
                    let role_id = metadata
                        .roles
                        .into_iter()
                        .find(|(_id, role)| role.name == *role_name)
                        .map(|(id, _role)| id)
                        .expect("Role not found.");

                    client.rename_role(&metadata.id, &role_id, new_name).await?;
                } else {
                    client.rename_project(&metadata.id, new_name).await?;
                }
            }
        },
        Command::Network(cmd) => match &cmd.subcmd {
            Network::List { external } => {
                if *external {
                    for client in client.list_external_clients().await? {
                        println!("{:?}", client);
                    }
                } else {
                    for project_id in client.list_networks().await? {
                        println!("{}", project_id);
                    }
                }
            }
            Network::View {
                project,
                as_id,
                user,
            } => {
                let project_id = if *as_id {
                    ProjectId::new(project.to_owned())
                } else {
                    let owner = user.clone().unwrap_or(get_current_user(&cfg));
                    client.get_project_metadata(&owner, project).await?.id
                };
                let state = client.get_room_state(&project_id).await?;
                println!("{:?}", state);
            }
            Network::ViewClient { client_id } => {
                let state = client.get_client_state(client_id).await?;
                println!("{:?}", state);
            }
            Network::Connect { address } => {
                let channel = client.connect(address).await?;
                println!(
                    "Listening for messages at {}@{}#NetsBloxCLI",
                    address,
                    cfg.username.unwrap_or(channel.id)
                );
                channel
                    .stream
                    .for_each(|msg| async {
                        let data = msg.unwrap().into_data();
                        let message = std::str::from_utf8(&data).unwrap();
                        println!("{}", &message);
                    })
                    .await;
            }
            Network::Evict { client_id } => {
                client.evict_occupant(client_id).await?;
            }
            Network::Send {
                address,
                r#type,
                data,
            } => {
                todo!();
            }
        },
        Command::Friends(cmd) => match &cmd.subcmd {
            Friends::List { online, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let friends = if *online {
                    client.list_online_friends(&username).await?
                } else {
                    client.list_friends(&username).await?
                };

                for friend in friends {
                    println!("{}", friend);
                }
            }

            Friends::ListInvites { user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                for invite in client.list_friend_invites(&username).await? {
                    println!("{:?}", invite);
                }
            }
            Friends::Block { username, user } => {
                let requestor = user.clone().unwrap_or(get_current_user(&cfg));
                client.block_user(&requestor, username).await?;
            }
            Friends::Unblock { username, user } => {
                let requestor = user.clone().unwrap_or(get_current_user(&cfg));
                client.unblock_user(&requestor, username).await?;
            }
            Friends::Remove { username, user } => {
                let owner = user.clone().unwrap_or(get_current_user(&cfg));
                client.unfriend(&owner, username).await?;
            }
            Friends::SendInvite { username, user } => {
                let sender = user.clone().unwrap_or(get_current_user(&cfg));
                client.send_friend_invite(&sender, &username).await?;
            }
            Friends::AcceptInvite {
                sender,
                reject,
                user,
            } => {
                let recipient = user.clone().unwrap_or(get_current_user(&cfg));
                let state = if *reject {
                    FriendLinkState::REJECTED
                } else {
                    FriendLinkState::APPROVED
                };
                client
                    .respond_to_friend_invite(&recipient, sender, state)
                    .await?;
            }
        },
        Command::ServiceHosts(cmd) => match &cmd.subcmd {
            ServiceHosts::List {
                authorized,
                user_only,
                group,
                user,
            } => {
                if *authorized {
                    for host in client.list_authorized_hosts().await? {
                        println!("{:?}", host);
                    }
                } else {
                    let username = user.clone().unwrap_or(get_current_user(&cfg));
                    let service_hosts = if *user_only {
                        client.list_user_hosts(&username).await?
                    } else if let Some(group_name) = group {
                        let groups = client.list_groups(&username).await?;
                        let group_id = groups
                            .into_iter()
                            .find(|g| g.name == *group_name)
                            .map(|group| group.id)
                            .unwrap();
                        client.list_group_hosts(&group_id).await?
                    } else {
                        client.list_hosts(&username).await?
                    };

                    for host in service_hosts {
                        println!("{:?}", host);
                    }
                }
            }
            ServiceHosts::Register {
                url,
                categories,
                group,
                user,
            } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                let mut service_hosts = if let Some(group_id) = group_id.clone() {
                    client.list_group_hosts(&group_id).await?
                } else {
                    client.list_user_hosts(&username).await?
                };

                service_hosts.push(ServiceHost {
                    url: url.to_owned(),
                    categories: categories.split(",").map(|s| s.to_owned()).collect(),
                });

                if let Some(group_id) = group_id {
                    client.set_group_hosts(&group_id, service_hosts).await?;
                } else {
                    client.set_user_hosts(&username, service_hosts).await?;
                }
            }
            ServiceHosts::Unregister { url, group, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                let mut service_hosts = if let Some(group_id) = group_id.clone() {
                    client.list_group_hosts(&group_id).await?
                } else {
                    client.list_user_hosts(&username).await?
                };

                let index = service_hosts
                    .iter()
                    .position(|host| host.url == *url)
                    .unwrap();

                service_hosts.swap_remove(index);

                if let Some(group_id) = group_id {
                    client.set_group_hosts(&group_id, service_hosts).await?;
                } else {
                    client.set_user_hosts(&username, service_hosts).await?;
                }
            }
            ServiceHosts::Authorize {
                url,
                client_id,
                public,
            } => {
                let secret = client.authorize_host(url, client_id, *public).await?;
                println!("{}", secret);
            }
            ServiceHosts::Unauthorize { url } => {
                let host = client
                    .list_authorized_hosts()
                    .await?
                    .into_iter()
                    .find(|host| &host.url == url)
                    .ok_or_else(|| {
                        netsblox_api::error::Error::NotFoundError(
                            "Authorized host not found.".to_string(),
                        )
                    })?;
                client.unauthorize_host(&host.id).await?;
            }
        },
        Command::ServiceSettings(cmd) => match &cmd.subcmd {
            ServiceSettings::List { group, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                    // TODO: throw error if not found (don't ignore and got to users)
                } else {
                    None
                };

                let service_hosts = if let Some(group_id) = group_id.clone() {
                    client.list_group_settings(&group_id).await?
                } else {
                    client.list_user_settings(&username).await?
                };

                for host in service_hosts {
                    println!("{}", host);
                }
            }
            ServiceSettings::View {
                group,
                host,
                all,
                user,
            } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));

                if *all {
                    let all_settings = client.get_all_settings(&username, &host).await?;
                    println!("{:?}", all_settings);
                } else {
                    let group_id = if let Some(group_name) = group {
                        let groups = client.list_groups(&username).await?;
                        groups
                            .into_iter()
                            .find(|g| g.name == *group_name)
                            .map(|group| group.id)
                    } else {
                        None
                    };
                    let settings = if let Some(group_id) = group_id.clone() {
                        client.get_group_settings(&group_id, &host).await?
                    } else {
                        client.get_user_settings(&username, &host).await?
                    };
                    println!("{}", settings);
                }
            }
            ServiceSettings::Set {
                host,
                settings,
                group,
                user,
            } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));

                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                let settings = if let Some(group_id) = group_id.clone() {
                    client
                        .set_group_settings(&group_id, &host, settings.to_owned())
                        .await?
                } else {
                    client
                        .set_user_settings(&username, &host, settings.to_owned())
                        .await?
                };
                println!("{}", settings);
            }
            ServiceSettings::Delete { host, group, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));

                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await?;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                if let Some(group_id) = group_id.clone() {
                    client.delete_group_settings(&group_id, &host).await?;
                } else {
                    client.delete_user_settings(&username, &host).await?;
                };
            }
        },
        Command::Libraries(cmd) => match &cmd.subcmd {
            Libraries::List {
                community,
                user,
                approval_needed,
            } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let libraries = if *community {
                    client.get_public_libraries().await?
                } else if *approval_needed {
                    client.get_submitted_libraries().await?
                } else {
                    client.get_libraries(&username).await?
                };

                for library in libraries {
                    println!("{}", library.name);
                }
            }
            Libraries::Import {
                filename,
                notes,
                name,
                user,
            } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let blocks = fs::read_to_string(filename).expect("Unable to read file");
                let name = name.clone().unwrap_or_else(|| {
                    Path::new(filename)
                        .file_stem()
                        .expect("Could not determine library name. Try passing --name")
                        .to_str()
                        .unwrap()
                        .to_owned()
                });
                client
                    .save_library(&username, &name, &blocks, notes)
                    .await?;
            }
            Libraries::Export { library, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let xml = client.get_library(&username, &library).await?;
                println!("{}", xml);
            }
            Libraries::Delete { library, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                client.delete_library(&username, &library).await?;
            }
            Libraries::Publish { library, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                client.publish_library(&username, &library).await?;
            }
            Libraries::Unpublish { library, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                client.unpublish_library(&username, &library).await?;
            }
            Libraries::Approve {
                library,
                user,
                reject,
            } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let state = if *reject {
                    PublishState::ApprovalDenied
                } else {
                    PublishState::Public
                };
                client.approve_library(&username, library, &state).await?;
            }
        },
        Command::Groups(cmd) => match &cmd.subcmd {
            Groups::List { user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let groups = client.list_groups(&username).await?;
                for group in groups {
                    println!("{}", group.name);
                }
            }
            Groups::Create { name, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                client.create_group(&username, &name).await?;
            }
            Groups::Delete { group, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.delete_group(&group_id).await?;
            }
            Groups::Members { group, user } => {
                let username = user.clone().unwrap_or(get_current_user(&cfg));
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap(); // FIXME

                for member in client.list_members(&group_id).await? {
                    println!("{:?}", member);
                }
            }
            Groups::Rename {
                group,
                new_name,
                user,
            } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.rename_group(&group_id, new_name).await?;
            }
            Groups::View { group, user } => {
                let username = user.clone().unwrap_or_else(|| get_current_user(&cfg));
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                let group = client.view_group(&group_id).await?;
                println!("{:?}", group);
            }
        },
        Command::Oauth(cmd) => match &cmd.subcmd {
            Oauth::List => {
                let clients = client.list_oauth_clients().await?;
                clients
                    .into_iter()
                    .for_each(|client| println!("{:?}", client));
            }
            Oauth::AddClient { name } => {
                let client_data = oauth::CreateClientData {
                    name: name.to_owned(),
                };
                let client_id = client.add_oauth_client(&client_data).await?;
                println!("{:?}", client_id);
            }
            Oauth::RemoveClient { id } => {
                client.remove_oauth_client(id).await?;
            }
            Oauth::Authorize { client_id, user } => {
                todo!();
            }
            Oauth::Revoke { client, user } => {
                todo!();
            }
        },
    }

    Ok(())
}

#[derive(Debug)]
struct RoleSpan {
    name: String,
    start: usize,
    media_start: usize,
    end: usize,
}

impl RoleSpan {
    pub(crate) fn new(name: String, start: usize, media_start: usize, end: usize) -> Self {
        Self {
            name,
            start,
            media_start,
            end,
        }
    }

    pub(crate) fn into_role(self, xml: &str) -> RoleData {
        let code = &xml[self.start..self.media_start];
        let media = &xml[self.media_start..self.end];

        RoleData {
            name: self.name,
            code: code.to_owned(),
            media: media.to_owned(),
        }
    }
}
