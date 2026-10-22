use anyhow::Result;
use berm_api::{
    Budget,
    host::{Caller, User},
};
use clap::{Args as Arguments, Parser, Subcommand, ValueEnum};
use crab_agent::{cmd, context::Contexts, utils};
use crabtalk_sdk::{
    account::billing::{PortalFlow, Purchase},
    client::{Client, Owner},
};
use std::{io::IsTerminal, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "crab",
    version,
    about = "Deploy and inspect programs on bermd or Cloud, and run a Cloud account"
)]
struct Args {
    /// The context to talk to, in place of the one in use.
    #[arg(long, global = true)]
    context: Option<String>,

    /// Act for this org, by handle or id, in place of the context's account.
    #[arg(long, global = true)]
    org: Option<String>,

    /// Print JSON. On whenever stdout is not a terminal.
    #[arg(long, global = true)]
    json: bool,

    /// Answer yes to every confirmation. Without a terminal, a command that
    /// asks refuses unless this is given.
    #[arg(long, short, global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sign in to Cloud in the browser, or with a code emailed to the address,
    /// and use it from now on.
    Login {
        /// The Cloud to sign in to.
        #[arg(default_value = cmd::login::CLOUD)]
        host: String,
        /// Sign in with GitHub in place of Google.
        #[arg(long)]
        github: bool,
        /// Sign in with a code emailed to this address, without a browser.
        #[arg(long, conflicts_with = "github")]
        email: Option<String>,
        /// The code emailed to `--email`. Without it, a code is sent and the
        /// command exits; a second run gives it here.
        #[arg(long, requires = "email")]
        code: Option<String>,
        /// The context to keep the session as.
        #[arg(long, default_value = "cloud")]
        name: String,
    },
    /// Forget the session of the context in use, or of `--context`.
    Logout,
    /// Where `crab` is talking, as whom, and on which plan.
    Status,
    /// Scaffold a program crate.
    New { name: String },
    /// List deployed programs.
    Ls,
    /// Build the program in a directory and print the path of its image.
    Build {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
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
    /// Remove a program and what it committed.
    Rm { name: String },
    /// Print what a program emitted, oldest first.
    Logs {
        name: String,
        /// Only logs after this sequence number.
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
    /// What a program used over the last 30 days, a day each; with no
    /// program, what the account's services used and how much of its plan.
    Usage { name: Option<String> },
    /// A service's calls over the last 30 days, newest first, on Cloud.
    Calls {
        name: String,
        /// Only calls of this tool.
        #[arg(long)]
        tool: Option<String>,
        /// Only calls that ended so: done, failed, denied or refused.
        #[arg(long)]
        outcome: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// What a service cost in a UTC month, by resource, on Cloud.
    Cost {
        name: String,
        /// The month, `YYYY-MM`; this one by default.
        #[arg(long)]
        month: Option<String>,
    },
    /// A program's cron tools: listed, set, stopped.
    #[command(subcommand)]
    Cron(CronCommand),
    /// The wakes armed to call a program's tools.
    Wakes { name: String },
    /// Who a service answers on Cloud, or with an argument, set it.
    Visibility {
        name: String,
        #[arg(value_enum)]
        set: Option<Shown>,
    },
    /// Stop serving a deprecated tool from a UTC day, or with no day, keep
    /// serving it.
    Sunset {
        name: String,
        tool: String,
        /// `YYYY-MM-DD`.
        on: Option<String>,
    },
    /// Set the most one call of a tool may use, from its next call.
    Budget {
        name: String,
        tool: String,
        /// CPU time, in microseconds.
        #[arg(long)]
        cpu: u64,
        #[arg(long)]
        rows: u64,
    },
    /// A program's secrets on Cloud: set, listed by name, revoked.
    #[command(subcommand)]
    Secret(SecretCommand),
    /// The account's plan, credit and payments; with no subcommand, where it
    /// stands.
    Billing {
        #[command(subcommand)]
        action: Option<BillingCommand>,
    },
    /// The account's API keys, which deploy from CI.
    #[command(subcommand)]
    Key(KeyCommand),
    /// The apps a website calls the account's services through.
    #[command(subcommand)]
    App(AppCommand),
    /// Orgs, their members and invitations.
    #[command(subcommand)]
    Org(OrgCommand),
    /// The orgs that invited you; with `accept`, join one.
    Invites {
        #[command(subcommand)]
        action: Option<InvitesCommand>,
    },
    /// The plans Cloud sells, and what each includes.
    Plans,
    /// What use past a plan's included amounts costs.
    Rates,
    /// Name the hosts `crab` talks to, and pick one.
    #[command(subcommand)]
    Context(ContextCommand),
    /// Replace this `crab` with the latest release, when it is newer.
    Upgrade,
}

#[derive(Clone, Copy, ValueEnum)]
enum Shown {
    Public,
    Private,
}

#[derive(Subcommand)]
enum CronCommand {
    /// List a program's cron tools, each with its expression and next run.
    Ls { name: String },
    /// Set a cron tool's expression, five fields in UTC, from its next firing.
    Set {
        name: String,
        tool: String,
        cron: String,
    },
    /// Stop a cron tool's calls until it is set again.
    Stop { name: String, tool: String },
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
enum BillingCommand {
    /// Ask a person to put the account on a plan: a code whose page they pay
    /// at. From a trial, the plan starts as they confirm it.
    Plan {
        slug: String,
        /// What it is for, shown to the person who pays.
        #[arg(long, default_value = "")]
        context: String,
    },
    /// Ask the person to start the free trial: a code whose page takes a card.
    Trial {
        /// What it is for, shown to the person who starts it.
        #[arg(long, default_value = "")]
        context: String,
    },
    /// Ask a person to buy credit: a code whose page takes the amount.
    Credit {
        /// What it is for, shown to the person who pays.
        #[arg(long, default_value = "")]
        context: String,
    },
    /// Where a payment request stands.
    Request {
        /// Its code, as `WDJB-MJHT`.
        code: String,
        /// Return once it is no longer pending.
        #[arg(long)]
        wait: bool,
        /// Seconds `--wait` waits at most.
        #[arg(long, default_value_t = 600)]
        timeout: u64,
    },
    /// Refill credit when it runs low; a cap of 0 turns it off.
    Refill {
        /// USD one refill buys, 20 or more.
        #[arg(long, default_value_t = 20)]
        amount: u32,
        /// The most USD refills may buy in a UTC month.
        #[arg(long)]
        cap: u32,
    },
    /// The account's invoices, newest first.
    Invoices,
    /// Open the Stripe portal.
    Portal {
        /// Open it on changing the card.
        #[arg(long, conflicts_with = "cancel")]
        card: bool,
        /// Open it on cancelling the plan at the period's end.
        #[arg(long)]
        cancel: bool,
    },
}

#[derive(Subcommand)]
enum KeyCommand {
    /// List the account's keys, active and revoked.
    Ls,
    /// Mint a deploy key. Its token is printed once.
    Create {
        label: String,
        /// Days until it expires; never by default.
        #[arg(long)]
        days: Option<u32>,
    },
    /// Revoke a key.
    Rm { id: String },
}

#[derive(Arguments)]
struct AppFields {
    name: String,
    /// An origin a browser may call from, `scheme://host[:port]`, or `*`.
    #[arg(long = "origin", required = true)]
    origins: Vec<String>,
    /// A service it may call, `{owner}/{name}`.
    #[arg(long = "service", required = true)]
    services: Vec<String>,
    /// An identity provider whose tokens name its users, `ISSUER=AUDIENCE`.
    #[arg(long = "issuer")]
    issuers: Vec<String>,
    /// Calls with no token reach the services' `#[tool(anonymous)]` tools.
    #[arg(long)]
    anonymous: bool,
}

#[derive(Subcommand)]
enum AppCommand {
    /// List the account's apps.
    Ls,
    /// Make an app.
    Create(AppFields),
    /// Replace an app.
    Update {
        id: String,
        #[command(flatten)]
        fields: AppFields,
    },
    /// Delete an app.
    Rm { id: String },
}

#[derive(Subcommand)]
enum OrgCommand {
    /// List the orgs you belong to.
    Ls,
    /// Make an org you own.
    Create {
        handle: String,
        /// Its display name; the handle by default.
        #[arg(long)]
        name: Option<String>,
    },
    /// One org, as much of it as you may see.
    Info { org: String },
    /// Change an org's handle or name.
    Update {
        org: String,
        #[arg(long)]
        handle: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete an org.
    Rm { org: String },
    /// List an org's members.
    Members { org: String },
    /// Add someone with an account to an org, by email.
    Add {
        org: String,
        email: String,
        /// As an owner.
        #[arg(long)]
        owner: bool,
    },
    /// Make a member an owner, or with `--member`, a member.
    Role {
        org: String,
        member_id: String,
        #[arg(long, conflicts_with = "member")]
        owner: bool,
        #[arg(long)]
        member: bool,
    },
    /// Remove a member from an org, or leave it.
    Remove { org: String, member_id: String },
    /// Invite an address to an org. Prints the token to send them.
    Invite {
        org: String,
        email: String,
        /// As an owner.
        #[arg(long)]
        owner: bool,
    },
    /// An org's pending invitations.
    Invites { org: String },
    /// Revoke a pending invitation.
    Uninvite { org: String, token: String },
}

#[derive(Subcommand)]
enum InvitesCommand {
    /// Join the org an invitation is for.
    Accept { token: String },
}

#[derive(Subcommand)]
enum ContextCommand {
    /// Name a host: a bermd, or Cloud with a token and the account to deploy
    /// under (`--org`, or `--user`).
    Add {
        name: String,
        host: String,
        /// A Cloud key with `deploy` scope.
        #[arg(long, conflicts_with = "token_stdin")]
        token: Option<String>,
        /// Read the key from stdin, keeping it out of the process arguments.
        #[arg(long)]
        token_stdin: bool,
        /// A person, by handle or id.
        #[arg(long)]
        user: Option<String>,
    },
    /// Talk to `name` from now on.
    Use { name: String },
    /// List the contexts, marking the one in use.
    Ls,
    /// Forget a context.
    Rm { name: String },
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let json = args.json || !std::io::stdout().is_terminal();
    let yes = args.yes;
    let client = || {
        let client = Client::from(Contexts::load()?.get(args.context.as_deref())?);
        Ok::<_, anyhow::Error>(match &args.org {
            Some(org) => client.owner(Owner::Org(org.clone())),
            None => client,
        })
    };

    match &args.command {
        Command::Login {
            host,
            github: _,
            email: Some(email),
            code,
            name,
        } => cmd::login::email(name, host, email, code.as_deref())?,
        Command::Login {
            host, github, name, ..
        } => cmd::login::run(name, host, *github)?,
        Command::Logout => cmd::logout::run(args.context.as_deref())?,
        Command::Status => cmd::status::run(&client()?, args.context.as_deref(), json)?,
        Command::Context(ContextCommand::Add {
            name,
            host,
            token,
            token_stdin,
            user,
        }) => cmd::context::add(
            name,
            host,
            token.as_deref(),
            *token_stdin,
            args.org.as_deref(),
            user.as_deref(),
        )?,
        Command::Context(ContextCommand::Use { name }) => cmd::context::use_(name)?,
        Command::Context(ContextCommand::Ls) => cmd::context::ls(json)?,
        Command::Context(ContextCommand::Rm { name }) => cmd::context::rm(name)?,
        Command::New { name } => cmd::new::run(name)?,
        Command::Ls => cmd::ls::run(&client()?, json)?,
        Command::Build { dir } => cmd::build::run(dir)?,
        Command::Deploy {
            dir,
            image,
            force,
            pull,
        } => cmd::deploy::run(&client()?, dir, image.as_deref(), *force, *pull)?,
        Command::Inspect { name } => cmd::inspect::run(&client()?, name, json)?,
        Command::Idl { name, out } => cmd::idl::run(&client()?, name, out)?,
        Command::Rm { name } => cmd::rm::run(&client()?, name, yes)?,
        Command::Logs { name, after } => cmd::logs::run(&client()?, name, *after, json)?,
        Command::Usage { name: Some(name) } => cmd::usage::run(&client()?, name, json)?,
        Command::Usage { name: None } => cmd::usage::account(&client()?, json)?,
        Command::Calls {
            name,
            tool,
            outcome,
            limit,
        } => cmd::calls::run(
            &client()?,
            name,
            tool.as_deref(),
            outcome.as_deref(),
            *limit,
            json,
        )?,
        Command::Cost { name, month } => cmd::cost::run(&client()?, name, month.as_deref(), json)?,
        Command::Cron(CronCommand::Ls { name }) => cmd::cron::ls(&client()?, name, json)?,
        Command::Cron(CronCommand::Set { name, tool, cron }) => {
            cmd::cron::set(&client()?, name, tool, Some(cron))?
        }
        Command::Cron(CronCommand::Stop { name, tool }) => {
            cmd::cron::set(&client()?, name, tool, None)?
        }
        Command::Wakes { name } => cmd::wakes::run(&client()?, name, json)?,
        Command::Visibility { name, set } => cmd::visibility::run(
            &client()?,
            name,
            set.map(|shown| matches!(shown, Shown::Private)),
            json,
        )?,
        Command::Sunset { name, tool, on } => {
            cmd::sunset::run(&client()?, name, tool, on.as_deref())?
        }
        Command::Secret(SecretCommand::Put { name, secret }) => {
            cmd::secret::put(&client()?, name, secret)?
        }
        Command::Secret(SecretCommand::Ls { name }) => cmd::secret::ls(&client()?, name, json)?,
        Command::Secret(SecretCommand::Rm { name, secret }) => {
            cmd::secret::rm(&client()?, name, secret, yes)?
        }
        Command::Budget {
            name,
            tool,
            cpu,
            rows,
        } => {
            utils::wait(client()?.budget(
                name,
                tool,
                Budget {
                    cpu: *cpu,
                    rows: *rows,
                },
            ))?;
            eprintln!("{name}.{tool}: at most {cpu}µs of CPU and {rows} rows a call");
        }
        Command::Billing { action: None } => cmd::billing::status(&client()?, json)?,
        Command::Billing {
            action: Some(action),
        } => {
            let client = client()?;
            match action {
                BillingCommand::Plan { slug, context } => {
                    cmd::billing::request(&client, Purchase::Plan, Some(slug), context, json)?
                }
                BillingCommand::Trial { context } => {
                    cmd::billing::request(&client, Purchase::Trial, None, context, json)?
                }
                BillingCommand::Credit { context } => {
                    cmd::billing::request(&client, Purchase::Credit, None, context, json)?
                }
                BillingCommand::Request {
                    code,
                    wait,
                    timeout,
                } => cmd::billing::status_of(&client, code, *wait, *timeout, json)?,
                BillingCommand::Refill { amount, cap } => {
                    cmd::billing::refill(&client, *amount, *cap)?
                }
                BillingCommand::Invoices => cmd::billing::invoices(&client, json)?,
                BillingCommand::Portal { card, cancel } => {
                    let flow = match (card, cancel) {
                        (true, _) => Some(PortalFlow::PaymentMethodUpdate),
                        (_, true) => Some(PortalFlow::SubscriptionCancel),
                        _ => None,
                    };
                    cmd::billing::portal(&client, flow)?
                }
            }
        }
        Command::Key(action) => {
            let client = client()?;
            match action {
                KeyCommand::Ls => cmd::key::ls(&client, json)?,
                KeyCommand::Create { label, days } => {
                    cmd::key::create(&client, label, *days, json)?
                }
                KeyCommand::Rm { id } => cmd::key::rm(&client, id, yes)?,
            }
        }
        Command::App(action) => {
            let client = client()?;
            let request = |fields: &AppFields| {
                cmd::app::request(
                    &fields.name,
                    &fields.origins,
                    &fields.services,
                    &fields.issuers,
                    fields.anonymous,
                )
            };
            match action {
                AppCommand::Ls => cmd::app::ls(&client, json)?,
                AppCommand::Create(fields) => cmd::app::create(&client, &request(fields)?, json)?,
                AppCommand::Update { id, fields } => {
                    cmd::app::update(&client, id, &request(fields)?, json)?
                }
                AppCommand::Rm { id } => cmd::app::rm(&client, id, yes)?,
            }
        }
        Command::Org(action) => {
            let client = client()?;
            match action {
                OrgCommand::Ls => cmd::org::ls(&client, json)?,
                OrgCommand::Create { handle, name } => {
                    cmd::org::create(&client, handle, name.as_deref(), json)?
                }
                OrgCommand::Info { org } => cmd::org::info(&client, org, json)?,
                OrgCommand::Update { org, handle, name } => {
                    cmd::org::update(&client, org, handle.as_deref(), name.as_deref())?
                }
                OrgCommand::Rm { org } => cmd::org::rm(&client, org, yes)?,
                OrgCommand::Members { org } => cmd::org::members(&client, org, json)?,
                OrgCommand::Add { org, email, owner } => {
                    cmd::org::add(&client, org, email, *owner)?
                }
                OrgCommand::Role {
                    org,
                    member_id,
                    owner,
                    member,
                } => {
                    if !owner && !member {
                        anyhow::bail!("say `--owner` or `--member`");
                    }
                    cmd::org::set_role(&client, org, member_id, *owner)?
                }
                OrgCommand::Remove { org, member_id } => {
                    cmd::org::remove(&client, org, member_id, yes)?
                }
                OrgCommand::Invite { org, email, owner } => {
                    cmd::org::invite(&client, org, email, *owner, json)?
                }
                OrgCommand::Invites { org } => cmd::org::invites(&client, org, json)?,
                OrgCommand::Uninvite { org, token } => cmd::org::uninvite(&client, org, token)?,
            }
        }
        Command::Invites { action: None } => cmd::invites::ls(&client()?, json)?,
        Command::Invites {
            action: Some(InvitesCommand::Accept { token }),
        } => cmd::invites::accept(&client()?, token)?,
        Command::Plans => cmd::plans::run(&client()?, json)?,
        Command::Rates => cmd::rates::run(&client()?, json)?,
        Command::Upgrade => cmd::upgrade::run(yes)?,
        // The one command whose exit code carries a result rather than a
        // verdict on berm.
        Command::Call {
            program,
            tool,
            arguments,
            caller,
            issuer,
            user,
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
            return cmd::call::run(
                &client()?,
                program,
                tool,
                arguments.as_deref(),
                caller.as_ref(),
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}
