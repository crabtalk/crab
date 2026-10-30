use anyhow::Result;
use clap::{Args as Arguments, Parser, Subcommand, ValueEnum};
use crab_agent::{cmd, context::Contexts};
use crabtalk_sdk::{
    account::{billing::PortalFlow, request::Purchase},
    client::{Client, Error, Owner},
};
use std::{io::IsTerminal, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "crab",
    version,
    about = "Run the local host, deploy and inspect services on it or Cloud, and run a Cloud account"
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
    /// Sign in to Cloud in the browser, and use it from now on.
    Login {
        /// The Cloud to sign in to.
        #[arg(default_value = cmd::login::CLOUD)]
        host: String,
        /// Sign in with GitHub in place of Google.
        #[arg(long)]
        github: bool,
        /// Print a code for a person to approve at `/device` in place of
        /// opening a browser here, as when none opens.
        #[arg(long, conflicts_with = "github")]
        device: bool,
        /// The context to keep the session as.
        #[arg(long, default_value = "cloud")]
        name: String,
    },
    /// Forget the session of the context in use, or of `--context`.
    Logout,
    /// Where a request asked of a person stands: a payment or a secret.
    Request {
        /// Its code, as `WDJB-MJHT`.
        code: String,
        /// Return once it is no longer pending.
        #[arg(long)]
        wait: bool,
        /// End it now, as if its time had run out: its code stops working.
        #[arg(long, conflicts_with = "wait")]
        expire: bool,
        /// Seconds `--wait` waits at most.
        #[arg(long, default_value_t = 600)]
        timeout: u64,
    },
    /// Where `crab` is talking, as whom, and on which plan.
    Status,
    /// Run the local host: deploy services to it and serve their tools over
    /// HTTP and MCP, in the foreground until Ctrl-C or `crab dev stop`.
    #[command(args_conflicts_with_subcommands = true)]
    Dev {
        #[command(subcommand)]
        command: Option<DevCommand>,
        #[command(flatten)]
        daemon: cmd::dev::Daemon,
        /// Run it in the background and return once it answers.
        #[arg(short, long)]
        detach: bool,
    },
    /// Scaffold a service crate.
    New { name: String },
    /// List deployed services.
    Ls,
    /// Build the service in a directory and print the path of its image.
    Build {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Build services and deploy each as its `crab.toml` names it, replacing
    /// whatever holds the name, and serve it: the one whose directory this is,
    /// or at a workspace's root every one not saying `deploy = false`. On
    /// Cloud it waits for a person to approve it at the link it prints last;
    /// `--yes` deploys without asking.
    Deploy(cmd::deploy::Deploy),
    /// `crab deploy`, installing each stopped, to serve once started.
    Push(cmd::deploy::Deploy),
    /// Serve stopped services again: those named, or those `crab deploy`
    /// would deploy here.
    Start { names: Vec<String> },
    /// Stop serving services, keeping what they committed: those named, or
    /// those `crab deploy` would deploy here.
    Stop { names: Vec<String> },
    /// Call one of a service's tools.
    Call(cmd::call::Call),
    /// Run a call that failed again, read from the context in use: here, the
    /// image it ran answered from its tape, or with `--on` on that context's
    /// host, against its state.
    Replay {
        service: String,
        /// The call's id, as `crab calls` lists it.
        id: u64,
        /// The context to run it on, against that host's state.
        #[arg(long)]
        on: Option<String>,
    },
    /// Show a service's README, tools and their arguments.
    Inspect {
        /// The service: a name on the context's account, or `owner/name` on
        /// Cloud for another account's public service.
        name: String,
    },
    /// Write a deployed service's IDL, which `declare_service!` reads.
    Idl {
        name: String,
        /// Where to write `{name}.json`.
        #[arg(long, default_value = "idls")]
        out: PathBuf,
    },
    /// Remove services and what they committed: those named, or those `crab
    /// deploy` would deploy here.
    Rm { names: Vec<String> },
    /// Print what a service emitted, oldest first.
    Logs {
        name: String,
        /// Only logs after this sequence number.
        #[arg(long, default_value_t = 0)]
        after: u64,
    },
    /// What a service used over the last 30 days, a day each; with no
    /// service, what the account's services used and how much of its plan.
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
    /// What the account's priced tools earned on Cloud, a UTC month each:
    /// revenue, the cut, compute and net, by service and tool; what is owed,
    /// and the payouts.
    Earnings {
        #[command(subcommand)]
        action: Option<EarningsCommand>,
    },
    /// A service's cron tools: listed, set, stopped.
    #[command(subcommand)]
    Cron(CronCommand),
    /// A service's vars: listed, set, returned to their defaults.
    #[command(subcommand)]
    Var(VarCommand),
    /// The wakes armed to call a service's tools.
    Wakes { name: String },
    /// A service's last 100 deploys on Cloud, newest first, refused and
    /// failed ones included.
    Deploys { name: String },
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
    /// The most one call of a tool may use, what its runs of the last 30
    /// days used, and the `budget` for its `crab.toml` they fit.
    Budget { name: String, tool: String },
    /// A service's secrets on Cloud: set, listed by name, revoked.
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
    /// What an error code means and what to do about it, as `E0001`.
    Explain { code: String },
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
    /// List a service's cron tools, each with its expression and next run.
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
enum VarCommand {
    /// List a service's vars, each with the value it holds or its default.
    Ls { name: String },
    /// Set a var, read from the service's next call.
    Set {
        name: String,
        var: String,
        value: String,
    },
    /// Return a var to its default.
    Unset { name: String, var: String },
}

#[derive(Subcommand)]
enum SecretCommand {
    /// Set a secret of a service, its value read from stdin and sent only to
    /// the hosts given, or to none. A set secret's value is changed only by
    /// revoking it and setting it again.
    Put {
        name: String,
        secret: String,
        /// A host its value is sent to, named in full, such as
        /// `api.stripe.com`. Repeat it for each host.
        #[arg(long = "host")]
        hosts: Vec<String>,
    },
    /// Send a secret of a service to these hosts alone, replacing those it
    /// had.
    Hosts {
        name: String,
        secret: String,
        /// A host its value is sent to, named in full, such as
        /// `api.stripe.com`. Repeat it for each host.
        #[arg(long = "host", required = true)]
        hosts: Vec<String>,
    },
    /// List a service's secrets, by name, with the hosts each is sent to.
    Ls { name: String },
    /// Revoke a secret of a service.
    Rm { name: String, secret: String },
    /// Set a service's credentials at this terminal, typed by the person
    /// here: in `secrets.toml` under the local host's root against the local
    /// host, on the account against Cloud. Refused when not run at a terminal.
    Fill {
        name: String,
        /// A value the credentials have the person enter at the provider,
        /// such as `callback=https://example.com/auth/callback`.
        #[arg(long = "copy", value_parser = cmd::secret::copy)]
        copies: Vec<(String, String)>,
    },
    /// Ask a person to set a service's unset required secrets, or one
    /// credential's, optional or not: a
    /// code whose page they set them at, signed in, so no value passes
    /// through here.
    Request {
        name: String,
        /// Ask for this credential, set or not, and no other.
        #[arg(long)]
        credential: Option<String>,
        /// A value the credentials have the person enter at the provider,
        /// such as `callback=https://example.com/auth/callback`.
        #[arg(long = "copy", value_parser = cmd::secret::copy)]
        copies: Vec<(String, String)>,
        /// What they are for, shown to the person who sets them.
        #[arg(long, default_value = "")]
        r#for: String,
    },
}

#[derive(Subcommand)]
enum EarningsCommand {
    /// Open Stripe's onboarding for the account's payout account, a US one,
    /// creating it the first time.
    Connect,
    /// Open the payout account's Stripe dashboard.
    Dashboard,
}

#[derive(Subcommand)]
enum BillingCommand {
    /// Ask a person to put the account on a plan: a code whose page they pay
    /// at.
    Plan {
        slug: String,
        /// What it is for, shown to the person who pays.
        #[arg(long, default_value = "")]
        r#for: String,
    },
    /// Verify the account on GitHub, which lets it deploy on Hobby: here when
    /// the check passes, else a code whose page the person verifies at.
    Verify {
        /// What it is for, shown to the person who verifies.
        #[arg(long, default_value = "")]
        r#for: String,
    },
    /// Ask a person to buy credit: a code whose page takes the amount.
    Credit {
        /// What it is for, shown to the person who pays.
        #[arg(long, default_value = "")]
        r#for: String,
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
enum DevCommand {
    /// Whether the local host runs, and where.
    Status,
    /// Stop the local host.
    Stop,
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
        /// Key permissions; auto allows operations without approval.
        #[arg(long, default_value = "deploy")]
        scope: Vec<crabtalk_sdk::account::key::Scope>,
    },
    /// Revoke a key.
    Rm { id: String },
    /// Replace a key's token, keeping its scope, budget, grants and expiry.
    /// The new token is printed once; the old one stops working at once.
    Roll { id: String },
    /// Ask an owner of the account to let the key `crab` signs in with call
    /// other accounts' priced services, or spend more a day. Prints the link
    /// to send them.
    Ask {
        /// A priced service to call, `{owner}/{name}`.
        #[arg(long = "service")]
        services: Vec<String>,
        /// USD a UTC day the key may spend on tool prices.
        #[arg(long)]
        budget: Option<u32>,
        /// What it is for, shown to them.
        #[arg(long, default_value = "")]
        context: String,
    },
    /// Let a key call another account's priced service, `{owner}/{name}`.
    Grant { id: String, service: String },
    /// Stop a key calling another account's priced service.
    Ungrant { id: String, service: String },
    /// Set the USD a key may spend on tool prices in a UTC day.
    Budget { id: String, usd: u32 },
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
    /// Calls with no token reach the services' `access = "anonymous"` tools.
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
    /// Name a host: a local host, or Cloud with a token and the account to deploy
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

fn main() -> ExitCode {
    let e = match run() {
        Ok(code) => return code,
        Err(e) => e,
    };
    eprintln!("Error: {e:?}");
    let refusal = e.chain().find_map(|cause| match cause.downcast_ref() {
        Some(Error::Api {
            explain, request, ..
        }) => Some((explain, request)),
        _ => None,
    });
    if let Some((_, Some(request))) = refusal {
        eprintln!(
            "\nSend {} to a person who can approve it; `crab request {} --wait` returns once they do.",
            request.url, request.code
        );
    }
    if let Some((Some(code), _)) = refusal {
        eprintln!("\nFor more information about this error, try `crab explain {code}`.");
    }
    ExitCode::FAILURE
}

fn run() -> Result<ExitCode> {
    let args = Args::parse();
    let json = args.json || !std::io::stdout().is_terminal();
    let yes = args.yes;
    let client = || {
        let client = Client::from(Contexts::load()?.get(args.context.as_deref())?);
        if yes
            && client.owner.is_some()
            && !client
                .token
                .as_deref()
                .is_some_and(|token| token.starts_with("ck_"))
        {
            anyhow::bail!("--yes on Cloud requires an API key with auto scope [E0022]");
        }
        Ok::<_, anyhow::Error>(match &args.org {
            Some(org) => client.owner(Owner::Org(org.clone())),
            None => client,
        })
    };
    // The Cloud whose rates price the context's calls.
    let cloud = || Ok::<_, anyhow::Error>(Contexts::load()?.get(args.context.as_deref())?.cloud());

    match &args.command {
        Command::Login {
            host,
            github,
            device,
            name,
        } => cmd::login::run(name, host, *github, *device)?,
        Command::Logout => cmd::logout::run(args.context.as_deref())?,
        Command::Request {
            code, expire: true, ..
        } => cmd::request::expire(&client()?, code, json)?,
        Command::Request {
            code,
            wait,
            timeout,
            ..
        } => cmd::request::run(&client()?, code, *wait, *timeout, json)?,
        Command::Status => cmd::status::run(&client()?, args.context.as_deref(), json)?,
        Command::Dev {
            command: Some(DevCommand::Status),
            ..
        } => cmd::dev::status(json)?,
        Command::Dev {
            command: Some(DevCommand::Stop),
            ..
        } => cmd::dev::stop()?,
        Command::Dev {
            command: None,
            daemon,
            detach: true,
        } => cmd::dev::detach(daemon)?,
        Command::Dev {
            command: None,
            daemon,
            detach: false,
        } => cmd::dev::run(daemon)?,
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
        Command::Deploy(deploy) => cmd::deploy::run(&client()?, deploy, false, yes)?,
        Command::Push(deploy) => cmd::deploy::run(&client()?, deploy, true, yes)?,
        Command::Start { names } => cmd::start::run(&client()?, names)?,
        Command::Stop { names } => cmd::stop::run(&client()?, names)?,
        Command::Inspect { name } => cmd::inspect::run(&client()?, &cloud()?, name, json)?,
        Command::Idl { name, out } => cmd::idl::run(&client()?, name, out)?,
        Command::Rm { names } => cmd::rm::run(&client()?, names, yes)?,
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
        Command::Earnings { action: None } => cmd::earnings::run(&client()?, json)?,
        Command::Earnings {
            action: Some(EarningsCommand::Connect),
        } => cmd::earnings::connect(&client()?)?,
        Command::Earnings {
            action: Some(EarningsCommand::Dashboard),
        } => cmd::earnings::dashboard(&client()?)?,
        Command::Cron(CronCommand::Ls { name }) => cmd::cron::ls(&client()?, name, json)?,
        Command::Cron(CronCommand::Set { name, tool, cron }) => {
            cmd::cron::set(&client()?, name, tool, Some(cron))?
        }
        Command::Cron(CronCommand::Stop { name, tool }) => {
            cmd::cron::set(&client()?, name, tool, None)?
        }
        Command::Var(VarCommand::Ls { name }) => cmd::var::ls(&client()?, name, json)?,
        Command::Var(VarCommand::Set { name, var, value }) => {
            cmd::var::set(&client()?, name, var, Some(value))?
        }
        Command::Var(VarCommand::Unset { name, var }) => {
            cmd::var::set(&client()?, name, var, None)?
        }
        Command::Wakes { name } => cmd::wakes::run(&client()?, name, json)?,
        Command::Deploys { name } => cmd::deploys::run(&client()?, name, json)?,
        Command::Visibility { name, set } => cmd::visibility::run(
            &client()?,
            name,
            set.map(|shown| matches!(shown, Shown::Private)),
            json,
        )?,
        Command::Sunset { name, tool, on } => {
            cmd::sunset::run(&client()?, name, tool, on.as_deref())?
        }
        Command::Secret(SecretCommand::Put {
            name,
            secret,
            hosts,
        }) => cmd::secret::put(&client()?, name, secret, hosts)?,
        Command::Secret(SecretCommand::Hosts {
            name,
            secret,
            hosts,
        }) => cmd::secret::hosts(&client()?, name, secret, hosts)?,
        Command::Secret(SecretCommand::Ls { name }) => cmd::secret::ls(&client()?, name, json)?,
        Command::Secret(SecretCommand::Rm { name, secret }) => {
            cmd::secret::rm(&client()?, name, secret, yes)?
        }
        Command::Secret(SecretCommand::Fill { name, copies }) => {
            cmd::secret::fill(&client()?, name, copies)?
        }
        Command::Secret(SecretCommand::Request {
            name,
            credential,
            copies,
            r#for,
        }) => cmd::secret::request(&client()?, name, credential.as_deref(), copies, r#for, json)?,
        Command::Budget { name, tool } => {
            cmd::budget::show(&client()?, &cloud()?, name, tool, json)?
        }
        Command::Billing { action: None } => cmd::billing::status(&client()?, json)?,
        Command::Billing {
            action: Some(action),
        } => {
            let client = client()?;
            match action {
                BillingCommand::Plan { slug, r#for } => {
                    cmd::billing::ask(&client, Purchase::Plan, Some(slug), r#for, json)?
                }
                BillingCommand::Verify { r#for } => cmd::billing::verify(&client, r#for, json)?,
                BillingCommand::Credit { r#for } => {
                    cmd::billing::ask(&client, Purchase::Credit, None, r#for, json)?
                }

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
                KeyCommand::Create { label, days, scope } => {
                    cmd::key::create(&client, label, *days, scope, json)?
                }
                KeyCommand::Rm { id } => cmd::key::rm(&client, id, yes)?,
                KeyCommand::Roll { id } => cmd::key::roll(&client, id, yes, json)?,
                KeyCommand::Ask {
                    services,
                    budget,
                    context,
                } => cmd::key::ask(&client, services, *budget, context, json)?,
                KeyCommand::Grant { id, service } => cmd::key::grant(&client, id, service)?,
                KeyCommand::Ungrant { id, service } => cmd::key::ungrant(&client, id, service)?,
                KeyCommand::Budget { id, usd } => cmd::key::budget(&client, id, *usd, json)?,
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
        Command::Explain { code } => cmd::explain::run(&client()?, code, json)?,
        Command::Upgrade => cmd::upgrade::run(yes)?,
        // The one command whose exit code carries the call's result.
        Command::Call(call) => return cmd::call::run(&client()?, &cloud()?, call),
        Command::Replay { service, id, on } => {
            return match on {
                Some(on) => {
                    let on = Contexts::load()?.get(Some(on))?;
                    let cloud = on.cloud();
                    cmd::replay::on(&client()?, &Client::from(on), &cloud, service, *id)
                }
                None => cmd::replay::tape(&client()?, service, *id),
            };
        }
    }
    Ok(ExitCode::SUCCESS)
}
