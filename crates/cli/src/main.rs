// Commands for the CLI:
//
//  - add --user option to try to act as another user
//
//  capabilities:
//    - login
//    - logout
//
//    - users create --admin  --group --password? TODO: add set-password endpoint
//    - users delete
//    - users view
//    - users link <snap_user>  <password> --strategy snap (include password?)
//    - users unlink <snap_user>  --strategy snap
//
//    - projects export --latest --role
//    - projects list --shared
//    - projects publish
//    - projects unpublish
//    - projects delete
//    - projects rename --role
//    - projects create-role?
//    - projects delete-role?
//    - project collaborators list
//    - project collaborators add
//    - project collaborators remove
//
//    - friends list --online
//    - friends remove <username>
//    - friends block <user>
//    - friends invites list
//    - friends invites send
//    - friends invites respond
//
//    - libraries list --community --approval-needed
//    - libraries export <name>
//    - libraries import <name> <xmlPath>
//    - libraries delete <name>
//    - libraries publish <name>
//    - libraries unpublish <name>
//    - libraries approve <id>
//
//    - service-hosts list --user-only --group-only
//    - service-hosts add <url> <categories> --group
//    - service-hosts remove <url> --group
//
//    - groups list
//    - groups view <name>
//    - groups members <name>
//    - groups create <name>
//    - groups rename <name> <new name>
//    - groups delete <name>
//
//    - collaboration invite <username> <project>
//    - collaboration list
//    - collaboration respond <invite/user> <response>
//
//    - network list --external
//    - network view --room <name> | --app
//    - network send {"some": "content"} --type MSG_TYPE --listen
//
static APP_NAME: &str = "netsblox-cli";

use clap::{Parser, Subcommand};
use inquire::{Confirm, Password, PasswordDisplayMode};
use netsblox_api::{Client, Config, Credentials};

#[derive(Subcommand, Debug)]
enum Users {
    Create {
        username: String,
        email: String,
        #[clap(short, long)]
        password: Option<String>,
        // #[clap(short, long)]
        // group: Option<String>,
        // #[clap(short, long)]
        // user: Option<String>,
        #[clap(short, long)]
        admin: bool,
    },
    Delete {
        username: String,
        #[clap(short, long)]
        no_confirm: bool,
    },
    View,
    List,
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

#[derive(Subcommand, Debug)]
enum Projects {
    Export {
        name: String,
        #[clap(short, long)]
        latest: bool,
        #[clap(short, long)]
        role: Option<String>,
        #[clap(short, long)]
        user: Option<String>,
    },
    List {
        #[clap(short, long)]
        user: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum Network {
    List {
        #[clap(short, long)]
        external: Option<String>,
    },
    // View {

    // }
}
//    - projects export --latest --role
//    - projects list --shared
//    - projects publish
//    - projects unpublish
//    - projects delete
#[derive(Subcommand, Debug)]
enum Friends {
    List {
        #[clap(short, long)]
        online: bool,
        #[clap(short, long)]
        user: Option<String>,
    },
    ListInvites {
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
enum Command {
    Login,
    Users(UserCommand),
    Projects(ProjectCommand),
    Network(NetworkCommand),
    Friends(FriendCommand),
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
    println!("Using config: {:?}", &cfg);

    let args = Cli::parse();
    let is_logged_in = !(cfg.token.is_none() || cfg.username.is_none());
    let login_required = match &args.cmd {
        Command::Login => true,
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
    let client = Client::new(cfg);

    // TODO: login if cookie is invalid
    match &args.cmd {
        Command::Login => {}
        Command::Users(cmd) => match &cmd.subcmd {
            Users::Create {
                username,
                email,
                password,
                admin,
            } => {
                // TODO: resolve group name to ID
                println!("Creating user: {:?}", username);
                client
                    .create_user(&username, email, password.as_deref(), None, admin)
                    .await;
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
            Users::View => {
                todo!();
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
            } //    - projects export --latest --role
              //    - projects list --shared
              //    - projects publish
              //    - projects unpublish
              //    - projects delete
              //    - projects rename --role
              //    - projects create-role?
              //    - projects delete-role?
              //    - project collaborators list
              //    - project collaborators add
              //    - project collaborators remove
        },
        Command::Projects(cmd) => match &cmd.subcmd {
            Projects::Export {
                name,
                role,
                latest,
                user,
            } => {
                let username = user.clone().unwrap_or(current_user);
                //let data = if let Some(role) = role {
                if let Some(role) = role {
                    client.export_role(&username, &name, &role, latest).await
                } else {
                    client.export_project(&username, &name, latest).await
                };
                //println!("{}", data);
            }
            Projects::List { user } => {
                let username = user.clone().unwrap_or(current_user);
                for project in client.list_projects(&username).await {
                    println!("{}", project);
                }
            }
        },
        Command::Network(cmd) => match &cmd.subcmd {
            Network::List { external } => {
                for network in client.list_networks().await {
                    println!("{}", network);
                }
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
                    println!("{}", invite);
                }
            }
        },
    }

    Ok(())
}
