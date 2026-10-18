use anyhow::{Context, Result};
use berm_api::{
    Budget, Dry,
    host::{Caller, User},
};
use clap::{Args as Arguments, Parser, Subcommand};
use crab_agent::{cmd, context::Contexts, utils};
use crabtalk_sdk::client::{Client, Owner};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "crab",
    version,
    about = "Deploy and inspect programs on bermd or Cloud"
)]
struct Args {
    /// The context to talk to, in place of the one in use.
    #[arg(long, global = true)]
    context: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a program crate.
    New { name: String },
    /// List deployed programs.
    Ls,
    /// Build the project in a directory and deploy it as its `berm.toml`
    /// names it, replacing whatever holds the name.
    Deploy {
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// The registry repository to push to, in place of `image` in
        /// `berm.toml`.
        #[arg(long)]
        image: Option<String>,
        /// Deploy even when it breaks what callers used, or undoes a cron its
        /// owners set.
        #[arg(long)]
        force: bool,
        /// Write the crons its owners set into `[crons]` in `berm.toml` before
        /// deploying.
        #[arg(long, conflicts_with = "force")]
        pull: bool,
    },
    /// Call one of a program's tools.
    Call {
        program: String,
        tool: String,
        /// The argument object. Read from stdin when omitted.
        arguments: Option<String>,
        /// On bermd, the caller the program is told called it.
        #[arg(long)]
        caller: Option<String>,
        /// On bermd, the end user's issuer the program is told it is called for.
        #[arg(long, requires = "user", requires = "caller")]
        issuer: Option<String>,
        /// On bermd, the end user's subject the program is told it is called
        /// for.
        #[arg(long, requires = "issuer")]
        user: Option<String>,
        /// On bermd, keep nothing the call changes.
        #[arg(long)]
        dry: bool,
        /// With `--dry`, a JSON array of `{"method", "url", "status",
        /// "headers", "body"}` a request is answered from in place of being
        /// sent.
        #[arg(long, requires = "dry")]
        answers: Option<PathBuf>,
        /// With `--dry`, refuse a request no answer matches.
        #[arg(long, requires = "dry")]
        offline: bool,
    },
    /// Show a program's tools and their arguments.
    Inspect { name: String },
    /// Write a deployed program's IDL, which `declare_program!` reads.
    Idl {
        name: String,
        /// Where to write `{name}.json`.
        #[arg(long, default_value = "idls")]
        out: PathBuf,
    },
    /// Remove a program.
    Rm { name: String },
    /// Print what a program emitted, oldest first.
    Logs {
        name: String,
        /// Only logs after this sequence number.
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
    /// Show what a program used over the last 30 days, a day each.
    Usage { name: String },
    /// Set the most one call of a tool may use, from its next call.
    Budget {
        name: String,
        tool: String,
        #[arg(long)]
        instructions: u64,
        #[arg(long)]
        rows: u64,
    },
    /// A program's secrets on Cloud: set, listed by name, revoked.
    #[command(subcommand)]
    Secret(SecretCommand),
    /// Name the hosts `crab` talks to, and pick one.
    #[command(subcommand)]
    Context(ContextCommand),
}

#[derive(Subcommand)]
enum SecretCommand {
    /// Set a secret of a program, its value read from stdin. A set secret is
    /// changed only by revoking it and setting it again.
    Put { name: String, secret: String },
    /// List a program's secrets, by name.
    Ls { name: String },
    /// Revoke a secret of a program.
    Rm { name: String, secret: String },
}

#[derive(Subcommand)]
enum ContextCommand {
    /// Name a host: a bermd, or Cloud with a token and the account to deploy
    /// under.
    Add {
        name: String,
        host: String,
        /// A Cloud key with `deploy` scope.
        #[arg(long)]
        token: Option<String>,
        #[command(flatten)]
        owner: OwnerArgs,
    },
    /// Talk to `name` from now on.
    Use { name: String },
    /// List the contexts, marking the one in use.
    Ls,
    /// Forget a context.
    Rm { name: String },
}

/// The account a Cloud context deploys under.
#[derive(Arguments)]
#[group(multiple = false)]
struct OwnerArgs {
    /// An org, by handle or id.
    #[arg(long)]
    org: Option<String>,
    /// A person, by handle or id.
    #[arg(long)]
    user: Option<String>,
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let client = || {
        Ok::<_, anyhow::Error>(Client::from(
            Contexts::load()?.get(args.context.as_deref())?,
        ))
    };

    match &args.command {
        Command::Context(ContextCommand::Add {
            name,
            host,
            token,
            owner,
        }) => {
            let owner = match (&owner.org, &owner.user) {
                (Some(org), _) => Some(Owner::Org(org.clone())),
                (_, Some(user)) => Some(Owner::User(user.clone())),
                _ => None,
            };
            cmd::context::add(name, host, token.as_deref(), owner)?
        }
        Command::Context(ContextCommand::Use { name }) => cmd::context::use_(name)?,
        Command::Context(ContextCommand::Ls) => cmd::context::ls()?,
        Command::Context(ContextCommand::Rm { name }) => cmd::context::rm(name)?,
        Command::New { name } => cmd::new::run(name)?,
        Command::Ls => cmd::ls::run(&client()?)?,
        Command::Deploy {
            dir,
            image,
            force,
            pull,
        } => cmd::deploy::run(&client()?, dir, image.as_deref(), *force, *pull)?,
        Command::Inspect { name } => cmd::inspect::run(&client()?, name)?,
        Command::Idl { name, out } => cmd::idl::run(&client()?, name, out)?,
        Command::Rm { name } => cmd::rm::run(&client()?, name)?,
        Command::Logs { name, after } => cmd::logs::run(&client()?, name, *after)?,
        Command::Usage { name } => cmd::usage::run(&client()?, name)?,
        Command::Secret(SecretCommand::Put { name, secret }) => {
            cmd::secret::put(&client()?, name, secret)?
        }
        Command::Secret(SecretCommand::Ls { name }) => cmd::secret::ls(&client()?, name)?,
        Command::Secret(SecretCommand::Rm { name, secret }) => {
            cmd::secret::rm(&client()?, name, secret)?
        }
        Command::Budget {
            name,
            tool,
            instructions,
            rows,
        } => {
            utils::wait(client()?.budget(
                name,
                tool,
                Budget {
                    instructions: *instructions,
                    rows: *rows,
                },
            ))?;
            println!("{name}.{tool}: at most {instructions} instructions and {rows} rows a call");
        }
        // The one command whose exit code carries a result rather than a
        // verdict on berm.
        Command::Call {
            program,
            tool,
            arguments,
            caller,
            issuer,
            user,
            dry,
            answers,
            offline,
        } => {
            let caller = caller.as_ref().map(|id| Caller {
                id: id.clone(),
                app: None,
                user: issuer
                    .clone()
                    .zip(user.clone())
                    .map(|(issuer, subject)| User { issuer, subject }),
                client: None,
                agent: None,
            });
            let dry = match dry {
                true => Some(Dry {
                    answers: match answers {
                        Some(path) => serde_json::from_slice(
                            &std::fs::read(path)
                                .with_context(|| format!("reading {}", path.display()))?,
                        )
                        .with_context(|| format!("{} is not a list of answers", path.display()))?,
                        None => Vec::new(),
                    },
                    offline: *offline,
                }),
                false => None,
            };
            return cmd::call::run(
                &client()?,
                program,
                tool,
                arguments.as_deref(),
                caller.as_ref(),
                dry,
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}
