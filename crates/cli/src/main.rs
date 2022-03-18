static APP_NAME: &str = "netsblox-cli";

use std::fs;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use inquire::{Confirm, Password, PasswordDisplayMode};
use netsblox_api::core::{
    Credentials, FriendLinkState, InvitationState, LibraryPublishState, LinkedAccount, ServiceHost,
    UserRole,
};
use netsblox_api::{Client, Config};
use std::path::Path;

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
        #[clap(short, long, default_value = "user")]
        role: UserRole,
    },
    Delete {
        username: String,
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
    List, // TODO: add verbose option?
    // TODO: add ban
    /// Ban a given user. Email address will also be blacklisted
    Ban {
        /// NetsBlox user to ban
        username: String,
    },
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
    Delete {
        project: String,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    Rename {
        project: String,
        new_name: String,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    InviteCollaborator {
        project: String,
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    ListInvites {
        #[clap(short, long)]
        user: Option<String>,
    },
    AcceptInvite {
        project: String,
        username: String,

        #[clap(long)]
        reject: bool,

        #[clap(short, long)]
        user: Option<String>,
    },
    ListCollaborators {
        project: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    RemoveCollaborator {
        project: String,
        username: String,
        #[clap(short, long)]
        user: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ServiceHosts {
    List {
        #[clap(long)]
        user_only: bool,
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    Add {
        url: String,
        categories: String, // TODO: Should this be optional?
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    Remove {
        url: String,
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
}

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
    Export {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Delete {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Publish {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unpublish {
        library: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Approve {
        library: String,
        #[clap(long)]
        reject: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
}

//    - network send {"some": "content"} --type MSG_TYPE --listen
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
    /// Connect to NetsBlox and listen for messages
    Connect {
        #[clap(short, long, default_value = "project")]
        address: String,
    },
    /// Evict a client from their current role
    Evict { client_id: String },
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
struct LibraryCommand {
    #[clap(subcommand)]
    subcmd: Libraries,
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
    Libraries(LibraryCommand),
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
    let current_user = cfg.username.as_ref().unwrap().clone();
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
                    let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
                client.set_password(&username, &password).await?;
            }
            Users::List => {
                for user in client.list_users().await? {
                    println!("{}", user);
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
                let username = user.clone().unwrap_or(current_user);
                let user = client.view_user(&username).await?;
                println!("{:?}", user);
            }
            Users::Link {
                username,
                password,
                user,
            } => {
                let as_user = user.clone().unwrap_or(current_user);
                let creds = netsblox_api::core::Credentials::Snap {
                    username: username.to_owned(),
                    password: password.to_owned(),
                };
                client.link_account(&as_user, &creds).await?;
            }
            Users::Unlink { username, user } => {
                let as_user = user.clone().unwrap_or(current_user);
                let account = LinkedAccount {
                    username: username.to_owned(),
                    strategy: "snap".to_owned(), // FIXME: add to linked account impl?
                };
                client.unlink_account(&as_user, &account).await?;
            }
            Users::Ban { username } => {
                client.ban_user(&username).await?;
            }
        },
        Command::Projects(cmd) => match &cmd.subcmd {
            Projects::Import {
                filename,
                name,
                user,
            } => {
                todo!();
            }
            Projects::Export {
                project,
                role,
                latest,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&username, &project).await?;
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
                let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&username, &project).await?;
                let project_id = metadata.id;

                client.publish_project(&project_id).await?;
                // TODO: add moderation here, too?
            }
            Projects::Unpublish { project, user } => {
                let username = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&username, &project).await?;
                let project_id = metadata.id;

                client.unpublish_project(&project_id).await?;
            }
            Projects::InviteCollaborator {
                project,
                username,
                user,
            } => {
                let owner = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&owner, &project).await?;
                let project_id = metadata.id;
                client.invite_collaborator(&project_id, &username).await?;
            }
            Projects::ListInvites { user } => {
                let username = user.clone().unwrap_or(current_user);
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
                let receiver = user.clone().unwrap_or(current_user);
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
                let owner = user.clone().unwrap_or(current_user);
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
                let owner = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&owner, &project).await?;
                client.remove_collaborator(&metadata.id, &username).await?;
            }
            Projects::Delete {
                project,
                role,
                user,
            } => {
                let owner = user.clone().unwrap_or(current_user);
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
                let owner = user.clone().unwrap_or(current_user);
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
                    project.to_owned()
                } else {
                    let owner = user.clone().unwrap_or(current_user);
                    client.get_project_metadata(&owner, &project).await?.id
                };
                let state = client.get_room_state(&project_id).await?;
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
        },
        Command::Friends(cmd) => match &cmd.subcmd {
            Friends::List { online, user } => {
                let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
                for invite in client.list_friend_invites(&username).await? {
                    println!("{:?}", invite);
                }
            }
            Friends::Block { username, user } => {
                let requestor = user.clone().unwrap_or(current_user);
                client.block_user(&requestor, username).await?;
            }
            Friends::Unblock { username, user } => {
                let requestor = user.clone().unwrap_or(current_user);
                client.unblock_user(&requestor, username).await?;
            }
            Friends::Remove { username, user } => {
                let owner = user.clone().unwrap_or(current_user);
                client.unfriend(&owner, username).await?;
            }
            Friends::SendInvite { username, user } => {
                let sender = user.clone().unwrap_or(current_user);
                client.send_friend_invite(&sender, &username).await?;
            }
            Friends::AcceptInvite {
                sender,
                reject,
                user,
            } => {
                let recipient = user.clone().unwrap_or(current_user);
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
                user_only,
                group,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
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
            ServiceHosts::Add {
                url,
                categories,
                group,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
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
            ServiceHosts::Remove { url, group, user } => {
                let username = user.clone().unwrap_or(current_user);
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
        },
        Command::Libraries(cmd) => match &cmd.subcmd {
            Libraries::List {
                community,
                user,
                approval_needed,
            } => {
                let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
                let xml = client.get_library(&username, &library).await?;
                println!("{}", xml);
            }
            Libraries::Delete { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.delete_library(&username, &library).await?;
            }
            Libraries::Publish { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.publish_library(&username, &library).await?;
            }
            Libraries::Unpublish { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.unpublish_library(&username, &library).await?;
            }
            Libraries::Approve {
                library,
                user,
                reject,
            } => {
                let username = user.clone().unwrap_or(current_user);
                let state = if *reject {
                    LibraryPublishState::ApprovalDenied
                } else {
                    LibraryPublishState::Public
                };
                client.approve_library(&username, &library, &state).await?;
            }
        },
        Command::Groups(cmd) => match &cmd.subcmd {
            Groups::List { user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await?;
                for group in groups {
                    println!("{}", group.name);
                }
            }
            Groups::Create { name, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.create_group(&username, &name).await?;
            }
            Groups::Delete { group, user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.delete_group(&group_id).await?;
            }
            Groups::Members { group, user } => {
                let username = user.clone().unwrap_or(current_user);
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
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await?;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.rename_group(&group_id, &new_name).await?;
            }
            Groups::View { group, user } => {
                let username = user.clone().unwrap_or(current_user);
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
    }

    Ok(())
}
