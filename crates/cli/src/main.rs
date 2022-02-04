static APP_NAME: &str = "netsblox-cli";

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use inquire::{Confirm, Password, PasswordDisplayMode};
use netsblox_api::{Client, Config, Credentials, FriendLinkState};
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
        #[clap(short, long)]
        user_only: bool,
        #[clap(short, long)]
        group_only: bool,
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
        // TODO: add an --all option?
        #[clap(short, long)]
        community: bool,
        #[clap(short, long)]
        approval_needed: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    Export {
        name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Delete {
        name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Publish {
        name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Unpublish {
        name: String,
        #[clap(short, long)]
        user: Option<String>,
    },
    Approve {
        name: String,
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
                // TODO: resolve group name to ID
                println!("Creating user: {:?}", username);
                client
                    .create_user(&username, email, password.as_deref(), None, admin)
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
                todo!();
            }
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
                // TODO: respect shared flag
                for project in client.list_projects(&username).await {
                    println!("{}", project.name);
                }
            }
            Projects::Publish { project, user } => {
                let username = user.clone().unwrap_or(current_user);
                //client.publish_project(user, project).await;
                todo!();
            }
            Projects::Unpublish { project, user } => {
                todo!();
            }
            Projects::InviteCollaborator {
                project,
                username,
                user,
            } => {
                todo!();
            }
            Projects::ListInvites { user } => {
                todo!();
            }
            Projects::RespondToInvite {
                project,
                username,
                response,
                user,
            } => {
                todo!();
            }
            Projects::ListCollaborators { project, user } => {
                todo!();
            }
            Projects::RemoveCollaborator {
                project,
                username,
                user,
            } => {
                todo!();
            }
            Projects::Delete {
                project,
                role,
                user,
            } => {
                todo!();
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

                // FIXME:
                // channel
                //     .read
                //     .for_each(|msg| async {
                //         println!("received message");
                //         let data = msg.unwrap().into_data();
                //         println!("{:?}", &data);
                //         tokio::io::stdout().write_all(&data).await.unwrap();
                //     })
                //     .await;
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
                group_only,
                user,
            } => {
                todo!();
            }
            ServiceHosts::Add {
                url,
                categories,
                group,
                user,
            } => {
                todo!();
            }
            ServiceHosts::Remove { url, group, user } => {
                todo!();
            }
        },
        Command::Libraries(cmd) => match &cmd.subcmd {
            Libraries::List {
                community,
                user,
                approval_needed,
            } => {
                todo!();
            }
            Libraries::Export { name, user } => {
                todo!();
            }
            Libraries::Delete { name, user } => {
                todo!();
            }
            Libraries::Publish { name, user } => {
                todo!();
            }
            Libraries::Unpublish { name, user } => {
                todo!();
            }
            Libraries::Approve { name, user } => {
                todo!();
            }
        },
    }

    Ok(())
}
