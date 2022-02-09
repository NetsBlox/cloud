static APP_NAME: &str = "netsblox-cli";

use std::fs;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use inquire::{Confirm, Password, PasswordDisplayMode};
use netsblox_api::{
    Client, Config, Credentials, FriendLinkState, InvitationState, LibraryPublishState, ServiceHost,
};
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Subcommand, Debug)]
enum Users {
    Create {
        username: String,
        email: String,
        #[clap(short, long)]
        password: Option<String>,
        #[clap(short, long)]
        group: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
        #[clap(short, long)]
        admin: bool,
    },
    Delete {
        username: String,
        #[clap(short, long)]
        no_confirm: bool,
    },
    View {
        #[clap(short, long)]
        user: Option<String>,
    },
    SetPassword {
        password: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    List, // TODO: add verbose option?
    //
    Link {
        account: String,
        password: String,
        #[clap(short, long, default_value = "snap")]
        strategy: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unlink {
        account: String,
        #[clap(short, long, default_value = "snap")]
        strategy: String,
        #[clap(short, long)]
        user: Option<String>,
    },
}

//    - projects create-role?
#[derive(Subcommand, Debug)]
enum Projects {
    Export {
        project: String,
        #[clap(short, long)]
        latest: bool,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    List {
        #[clap(short, long)]
        shared: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    Publish {
        project: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unpublish {
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
    RespondToInvite {
        project: String,
        username: String,
        #[clap(arg_enum)]
        response: InviteResponse,

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
    List {
        #[clap(short, long)]
        community: bool,
        #[clap(short, long)]
        approval_needed: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    Save {
        /// The path to the exported blocks to save
        filename: String,
        #[clap(long, default_value = "")]
        notes: String,
        #[clap(short, long)]
        name: Option<String>,
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
//    - libraries list --community --approval-needed
//    - libraries import <name> <xmlPath>

//    - network send {"some": "content"} --type MSG_TYPE --listen
#[derive(Subcommand, Debug)]
enum Network {
    List {
        #[clap(short, long)] // TODO: Add an --all flag??
        external: bool,
    },
    View {
        // FIXME: or should we just accept the address?
        #[clap(short, long)]
        room: Option<String>,
        #[clap(short, long)]
        app: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    /// Connect to NetsBlox and listen for messages
    Connect {
        #[clap(short, long, default_value = "project")]
        address: String,
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

#[derive(clap::ArgEnum, Clone, Debug)]
enum InviteResponse {
    APPROVE,
    REJECT,
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
    RespondTo {
        sender: String,
        #[clap(arg_enum)]
        response: InviteResponse,
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

fn prompt_credentials() -> Credentials {
    // FIXME: can't delete w/ backspace???
    let username = inquire::Text::new("Username:")
        .prompt()
        .expect("Unable to prompt username");

    let password = Password::new("Password:")
        .with_display_toggle_enabled()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()
        .expect("Unable to prompt password");

    Credentials { username, password }
}

#[tokio::main]
async fn main() -> Result<(), confy::ConfyError> {
    let mut cfg: Config = confy::load(&APP_NAME)?;
    cfg.app_id = Some("NetsBloxCLI".to_owned());
    println!("Using config: {:?}", &cfg);

    let args = Cli::parse();
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
        let credentials = prompt_credentials();
        let token = netsblox_api::login(&cfg, &credentials)
            .await
            .expect("Login failed");
        cfg.token = Some(token);
        cfg.username = Some(credentials.username.to_owned());
        confy::store(&APP_NAME, &cfg)?;
    }
    let current_user = cfg.username.as_ref().unwrap().clone();
    let client = Client::new(cfg.clone());

    // TODO: login if cookie is invalid. Or just throw an error for user to re-login
    match &args.cmd {
        Command::Login => {}
        Command::Logout => {
            cfg.token = None;
            cfg.username = None;
            confy::store(&APP_NAME, &cfg)?;
        }
        Command::Users(cmd) => match &cmd.subcmd {
            Users::Create {
                username,
                email,
                password,
                admin,
                group,
                user,
            } => {
                let group_id = if let Some(group_name) = group {
                    let username = user.clone().unwrap_or(current_user);
                    let groups = client.list_groups(&username).await;
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
                        admin,
                    )
                    .await;
            }
            Users::SetPassword { password, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.set_password(&username, &password).await;
            }
            Users::List => {
                for user in client.list_users().await {
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
                    client.delete_user(&username).await;
                    println!("deleted {}", username);
                }
            }
            Users::View { user } => {
                let username = user.clone().unwrap_or(current_user);
                let user = client.view_user(&username).await;
                println!("{:?}", user);
            }
            Users::Link {
                account,
                password,
                strategy,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                client
                    .link_account(&username, account, password, strategy)
                    .await;
                todo!();
            }
            Users::Unlink {
                account,
                strategy,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                client.unlink_account(&username, account, strategy).await;
            } // TODO: list linked accounts?
        },
        Command::Projects(cmd) => match &cmd.subcmd {
            Projects::Export {
                project,
                role,
                latest,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                let xml = if let Some(role) = role {
                    client
                        .export_role(&username, &project, &role, latest)
                        .await
                        .to_xml()
                } else {
                    // TODO: Should this output the Project or an xml?
                    // maybe the Project which contains a toXML method?
                    client
                        .export_project(&username, &project, latest)
                        .await
                        .to_xml()
                };
                println!("{}", xml);
            }
            Projects::List { user, shared } => {
                let username = user.clone().unwrap_or(current_user);
                let projects = if *shared {
                    client.list_shared_projects(&username).await
                } else {
                    client.list_projects(&username).await
                };

                for project in projects {
                    println!("{:?}", project);
                }
            }
            Projects::Publish { project, user } => {
                let username = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&username, &project).await;
                let project_id = metadata.id;

                client.publish_project(&project_id).await;
                // TODO: add moderation here, too?
            }
            Projects::Unpublish { project, user } => {
                let username = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&username, &project).await;
                let project_id = metadata.id;

                client.unpublish_project(&project_id).await;
            }
            Projects::InviteCollaborator {
                project,
                username,
                user,
            } => {
                let owner = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&owner, &project).await;
                let project_id = metadata.id;
                client.invite_collaborator(&project_id, &username).await;
            }
            Projects::ListInvites { user } => {
                let username = user.clone().unwrap_or(current_user);
                let invites = client.list_collaboration_invites(&username).await;
                for invite in invites {
                    println!("{:?}", invite);
                }
            }
            Projects::RespondToInvite {
                project,
                username,
                response,
                user,
            } => {
                let invites = client.list_collaboration_invites(&username).await;
                let sender = user.clone().unwrap_or(current_user);
                let invite = invites
                    .iter()
                    .find(|inv| inv.sender == sender)
                    .expect("Invitation not found.");

                let state = match response {
                    InviteResponse::APPROVE => InvitationState::ACCEPTED,
                    InviteResponse::REJECT => InvitationState::REJECTED,
                };
                client
                    .respond_to_collaboration_invite(&invite.id, &state)
                    .await;
            }
            Projects::ListCollaborators { project, user } => {
                let owner = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&owner, &project).await;
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
                let metadata = client.get_project_metadata(&owner, &project).await;
                client.remove_collaborator(&metadata.id, &username).await;
            }
            Projects::Delete {
                project,
                role,
                user,
            } => {
                let owner = user.clone().unwrap_or(current_user);
                let metadata = client.get_project_metadata(&owner, &project).await;
                if let Some(role_name) = role {
                    let role_id = metadata
                        .roles
                        .into_iter()
                        .find(|(id, role)| role.name == *role_name)
                        .map(|(id, _role)| id)
                        .expect("Role not found.");

                    client.delete_role(&metadata.id, &role_id).await;
                } else {
                    client.delete_project(&metadata.id).await;
                }
            }
            Projects::Rename {
                project,
                new_name,
                role,
                user,
            } => {
                todo!();
            }
        },
        Command::Network(cmd) => match &cmd.subcmd {
            Network::List { external } => {
                for network in client.list_networks().await {
                    println!("{}", network);
                }
            }
            Network::View { room, app, user } => {
                todo!();
            }
            Network::Connect { address } => {
                // TODO: request a client_id
                // TODO: connect to NetsBlox
                // let config = client.connect().await;

                // TODO: do we need a client ID for anything else?
                // Maybe this can be kept internal to the client?
                // setting the client state...
                // probably can still be kept internal
                let channel = client.connect(address).await;
                println!(
                    "Listening for messages at {}@{}#NetsBloxCLI",
                    address,
                    cfg.username.unwrap_or(channel.id)
                );
                // channel.stream.filter_map(|msg| {
                //     future::ready(match msg {
                //         Ok(Message::Text(txt)) => Some(txt),
                //         _ => None,
                //     })
                // });

                // FIXME:
                channel
                    .stream
                    // .read
                    .for_each(|msg| async {
                        let data = msg.unwrap().into_data();
                        let message = std::str::from_utf8(&data).unwrap();
                        println!("{}", &message);
                        // tokio::io::stdout().write_all(&data).await.unwrap();
                    })
                    .await;
            }
        },
        Command::Friends(cmd) => match &cmd.subcmd {
            Friends::List { online, user } => {
                let username = user.clone().unwrap_or(current_user);
                let friends = if *online {
                    client.list_online_friends(&username).await
                } else {
                    client.list_friends(&username).await
                };

                for friend in friends {
                    println!("{}", friend);
                }
            }

            Friends::ListInvites { user } => {
                let username = user.clone().unwrap_or(current_user);
                for invite in client.list_friend_invites(&username).await {
                    println!("{:?}", invite);
                }
            }
            Friends::Block { username, user } => {
                let requestor = user.clone().unwrap_or(current_user);
                client.block_user(&requestor, username).await;
            }
            Friends::Unblock { username, user } => {
                let requestor = user.clone().unwrap_or(current_user);
                client.unblock_user(&requestor, username).await;
            }
            Friends::Remove { username, user } => {
                let owner = user.clone().unwrap_or(current_user);
                client.unfriend(&owner, username).await;
            }
            Friends::SendInvite { username, user } => {
                let sender = user.clone().unwrap_or(current_user);
                client.send_friend_invite(&sender, &username).await;
            }
            Friends::RespondTo {
                sender,
                response,
                user,
            } => {
                let recipient = user.clone().unwrap_or(current_user);
                let state = match response {
                    InviteResponse::APPROVE => FriendLinkState::APPROVED,
                    InviteResponse::REJECT => FriendLinkState::REJECTED,
                };
                client
                    .respond_to_friend_invite(&recipient, sender, state)
                    .await;
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
                    client.list_user_hosts(&username).await
                } else if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await;
                    let group_id = groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                        .unwrap();
                    client.list_group_hosts(&group_id).await
                } else {
                    client.list_hosts(&username).await
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
                    let groups = client.list_groups(&username).await;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                let mut service_hosts = if let Some(group_id) = group_id.clone() {
                    client.list_group_hosts(&group_id).await
                } else {
                    client.list_user_hosts(&username).await
                };

                service_hosts.push(ServiceHost {
                    url: url.to_owned(),
                    categories: categories.split(",").map(|s| s.to_owned()).collect(),
                });

                if let Some(group_id) = group_id {
                    client.set_group_hosts(&group_id, service_hosts).await;
                } else {
                    client.set_user_hosts(&username, service_hosts).await;
                }
            }
            ServiceHosts::Remove { url, group, user } => {
                let username = user.clone().unwrap_or(current_user);
                let group_id = if let Some(group_name) = group {
                    let groups = client.list_groups(&username).await;
                    groups
                        .into_iter()
                        .find(|g| g.name == *group_name)
                        .map(|group| group.id)
                } else {
                    None
                };
                let mut service_hosts = if let Some(group_id) = group_id.clone() {
                    client.list_group_hosts(&group_id).await
                } else {
                    client.list_user_hosts(&username).await
                };

                let index = service_hosts
                    .iter()
                    .position(|host| host.url == *url)
                    .unwrap();

                service_hosts.swap_remove(index);

                if let Some(group_id) = group_id {
                    client.set_group_hosts(&group_id, service_hosts).await;
                } else {
                    client.set_user_hosts(&username, service_hosts).await;
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
                    client.get_public_libraries().await
                } else if *approval_needed {
                    client.get_submitted_libraries().await
                } else {
                    client.get_libraries(&username).await
                };

                for library in libraries {
                    println!("{}", library.name);
                }
            }
            Libraries::Save {
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
                client.save_library(&username, &name, &blocks, notes).await;
            }
            Libraries::Export { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                let xml = client.get_library(&username, &library).await;
                println!("{}", xml);
            }
            Libraries::Delete { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.delete_library(&username, &library).await;
            }
            Libraries::Publish { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.publish_library(&username, &library).await;
            }
            Libraries::Unpublish { library, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.unpublish_library(&username, &library).await;
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
                client.approve_library(&username, &library, &state).await;
            }
        },
        Command::Groups(cmd) => match &cmd.subcmd {
            Groups::List { user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await;
                for group in groups {
                    println!("{}", group.name);
                }
            }
            Groups::Create { name, user } => {
                let username = user.clone().unwrap_or(current_user);
                client.create_group(&username, &name).await;
            }
            Groups::Delete { group, user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.delete_group(&group_id).await;
            }
            Groups::Members { group, user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap(); // FIXME

                for member in client.list_members(&group_id).await {
                    println!("{:?}", member);
                }
            }
            Groups::Rename {
                group,
                new_name,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                client.rename_group(&group_id, &new_name).await;
            }
            Groups::View { group, user } => {
                let username = user.clone().unwrap_or(current_user);
                let groups = client.list_groups(&username).await;
                let group_id = groups
                    .into_iter()
                    .find(|g| g.name == *group)
                    .map(|group| group.id)
                    .unwrap();

                let group = client.view_group(&group_id).await;
                println!("{:?}", group);
            }
        },
    }

    Ok(())
}
