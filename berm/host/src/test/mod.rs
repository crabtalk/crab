//! A host for tests: a service's built image runs under the real engine,
//! against edges the test holds. Fetches answer what the test arranged, images
//! are bytes it handed in, there are no peers, secrets are what it granted,
//! each service signs with a key generated on first use, and the clock stands
//! where it set it. Wakes are fired by the test, not a timer.
//!
//! ```ignore
//! let host = TestHost::open(0).await?;
//! host.arrange("GET", "https://example.com/feed", 200, feed);
//! host.deploy("feeds", &berm_host::test::build("feeds")?).await?;
//! host.call("feeds", "start", b"{}").await?;
//! for (wake, ran) in host.fire().await? { /* … */ }
//! let logs = host.logs("feeds", Some("Item")).await?;
//! ```

pub use berm::User;

use crate::{
    api::outside::Layers,
    env::{Clock, Edges},
    host::{self, Host},
    state::service,
    wake::table,
};
use anyhow::{Context, Result, bail};
use berm::{
    Manifest, Meter, Program,
    syscall::{
        http::{Request, Response},
        sql::Statement,
    },
    tape::Taping,
};
use berm_api::{
    Logged, Output,
    host::{DATABASE_SIZE, Deploy, Page},
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{self, Poll},
    time::Duration,
};

mod clock;
mod env;
mod images;
mod peers;
mod secrets;

/// The edges of a [`TestHost`].
#[derive(Clone)]
pub struct Test;

/// A host whose state lives in a directory of its own, removed when it drops.
pub struct TestHost {
    host: Host<Test>,
    fetch: Arranged,
    images: Given,
    secrets: Granted,
    clock: Manual,
    dir: PathBuf,
}

/// How one call ended.
#[derive(Debug)]
pub struct Ran {
    /// What the tool answered, or the failure it reported.
    pub result: Output,
    /// `result` is [`Output::Failed`] because the tool refused its caller.
    pub denied: bool,
    /// What the chain used, over every service it reached.
    pub meter: Meter,
}

/// A wake the service armed.
#[derive(Debug, Clone)]
pub struct Wake {
    /// The service that armed it.
    pub service: String,
    /// The service it calls, by name in the arming one's scope.
    pub program: String,
    pub tool: String,
    pub args: String,
}

impl TestHost {
    /// A host with nothing deployed, its clock at `now` in milliseconds since
    /// the Unix epoch.
    pub async fn open(now: u64) -> Result<Self> {
        Self::open_with(now, crate::Config::default().cpu).await
    }

    /// [`TestHost::open`], each call spending at most `cpu` of CPU.
    pub async fn open_with(now: u64, cpu: Duration) -> Result<Self> {
        static OPENED: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "berm-test-{}-{}",
            std::process::id(),
            OPENED.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let (fetch, images, secrets) = (Arranged::default(), Given::default(), Granted::default());
        let clock = Manual(Arc::new(AtomicU64::new(now)));
        let config = crate::Config {
            databases: dir.join("databases"),
            cpu,
            ..crate::Config::default()
        };
        let host = Host::open(
            &dir.join("host.db"),
            &config,
            Edges {
                fetch: fetch.clone(),
                proxy: Some(fetch.clone()),
                images: images.clone(),
                peers: Nowhere,
                secrets: secrets.clone(),
                layers: Layers::new(tower::layer::util::Identity::new()),
                observe: (),
                clock: clock.clone(),
            },
        )
        .await?;
        Ok(Self {
            host,
            fetch,
            images,
            secrets,
            clock,
            dir,
        })
    }

    /// The host itself, for what this does not wrap.
    pub fn host(&self) -> &Host<Test> {
        &self.host
    }

    /// Answer `method` on `url` with `status` and `body`, from now on.
    pub fn arrange(&self, method: &str, url: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.fetch.0.lock().answers.insert(
            (method.to_owned(), url.to_owned()),
            Response {
                status,
                headers: Vec::new(),
                body: body.into(),
            },
        );
    }

    /// Every request sent so far, in order, with its secrets substituted.
    pub fn fetched(&self) -> Vec<Request> {
        self.fetch.0.lock().sent.clone()
    }

    /// Grant `service` the secret `secret` for `host`.
    pub fn grant(&self, service: &str, secret: &str, host: &str, value: &str) {
        self.secrets.0.lock().insert(
            (service.to_owned(), secret.to_owned(), host.to_owned()),
            value.to_owned(),
        );
    }

    /// Milliseconds since the Unix epoch, as the clock stands.
    pub fn now(&self) -> u64 {
        self.clock.now()
    }

    /// Stand the clock at `now`.
    pub fn set_now(&self, now: u64) {
        self.clock.0.store(now, Ordering::Relaxed);
    }

    /// Move the clock `by` milliseconds on.
    pub fn advance(&self, by: u64) {
        self.clock.0.fetch_add(by, Ordering::Relaxed);
    }

    /// Run `image` as `service`, every public tool confirmed and each cron tool
    /// without an expression.
    pub async fn deploy(&self, service: &str, image: &[u8]) -> Result<Arc<Program>> {
        self.seeded(service, image, None).await
    }

    /// [`TestHost::deploy`], its database started from the SQLite file at
    /// `seed`, which is moved.
    pub async fn seeded(
        &self,
        service: &str,
        image: &[u8],
        seed: Option<&Path>,
    ) -> Result<Arc<Program>> {
        self.installed(service, image, seed, DATABASE_SIZE, false)
            .await
    }

    /// [`TestHost::deploy`], its database held to `size` bytes.
    pub async fn sized(&self, service: &str, image: &[u8], size: u64) -> Result<Arc<Program>> {
        self.installed(service, image, None, size, false).await
    }

    /// [`TestHost::deploy`], installed stopped.
    pub async fn pushed(&self, service: &str, image: &[u8]) -> Result<Arc<Program>> {
        self.installed(service, image, None, DATABASE_SIZE, true)
            .await
    }

    async fn installed(
        &self,
        service: &str,
        image: &[u8],
        seed: Option<&Path>,
        size: u64,
        stopped: bool,
    ) -> Result<Arc<Program>> {
        let manifest = Manifest::from_image(image)?;
        let digest = format!("sha256:{}", berm::digest(image));
        let reference = format!("test/{service}");
        self.images
            .0
            .lock()
            .insert(reference.clone(), image.to_vec());
        Ok(self
            .host
            .deploy(
                service,
                Deploy {
                    reference,
                    digest,
                    public: manifest
                        .tools
                        .iter()
                        .filter(|tool| tool.public)
                        .map(|tool| tool.name.clone())
                        .collect(),
                    force: true,
                    crons: None,
                    size,
                    stopped,
                },
                seed,
            )
            .await?)
    }

    /// Run `tool` of `service` with no caller, as a wake does, public or not,
    /// and commit what it did.
    pub async fn call(&self, service: &str, tool: &str, args: &[u8]) -> Result<Ran> {
        self.run(service, tool, args, None).await
    }

    /// [`TestHost::call`], for `user`.
    pub async fn call_for(
        &self,
        service: &str,
        tool: &str,
        args: &[u8],
        user: User,
    ) -> Result<Ran> {
        self.run(service, tool, args, Some(user)).await
    }

    async fn run(&self, service: &str, tool: &str, args: &[u8], user: Option<User>) -> Result<Ran> {
        let program = self
            .host
            .get(service)
            .with_context(|| format!("no service named {service:?} is deployed"))?;
        let invoked = self
            .host
            .invoke(
                &program,
                self.host.calls.start().id,
                None,
                user,
                None,
                tool,
                args.to_vec(),
                Taping::Off,
            )
            .await?;
        Ok(Ran {
            result: invoked.result,
            denied: invoked.denied,
            meter: invoked.meter,
        })
    }

    /// Write the calls and logs held in memory, as the host's timer does.
    pub async fn flush(&self) -> Result<()> {
        self.host.flush().await
    }

    /// Every pending wake beside when it is due, in milliseconds since the
    /// Unix epoch, soonest first.
    pub async fn wakes(&self) -> Result<Vec<(Wake, u64)>> {
        Ok(table::wakes(&self.host.store)
            .await?
            .into_iter()
            .map(|(wake, due)| (Wake::from(&wake), due as u64))
            .collect())
    }

    /// Fire every wake due as the clock stands, one at a time, and answer each
    /// beside how its call ended. A wake whose call fails stays armed, due
    /// again a minute on.
    pub async fn fire(&self) -> Result<Vec<(Wake, Ran)>> {
        let mut fired = Vec::new();
        for wake in self.host.due().await? {
            let woken = Wake::from(&wake);
            let Some(invoked) = self.host.fire(wake).await else {
                bail!("{}.{} did not run", woken.program, woken.tool);
            };
            fired.push((
                woken,
                Ran {
                    result: invoked.result,
                    denied: invoked.denied,
                    meter: invoked.meter,
                },
            ));
        }
        Ok(fired)
    }

    /// Every log `service` committed, oldest first, those carrying `topic`
    /// when one is given.
    pub async fn logs(&self, service: &str, topic: Option<&str>) -> Result<Vec<Logged>> {
        let mut logs: Vec<Logged> = Vec::new();
        loop {
            let after = logs.last().map_or(0, |log| log.seq as i64);
            let page = self
                .host
                .logs(
                    service,
                    Page {
                        topic: topic.map(str::to_owned),
                        call: None,
                        after,
                        limit: i64::MAX,
                    },
                )
                .await?
                .0;
            if page.is_empty() {
                return Ok(logs);
            }
            logs.extend(page);
        }
    }

    /// What `service` last committed under `key`.
    pub async fn state(&self, service: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let program = self
            .host
            .get(service)
            .with_context(|| format!("no service named {service:?} is deployed"))?;
        if host::keeps(&program) {
            return self
                .host
                .databases
                .get(service)?
                .get(0, key.to_owned())
                .await;
        }
        let (scope, name) = host::split(service);
        service::read(&self.host.store, scope, name, key.to_owned()).await
    }

    /// Run `sql`, which may only read, against `service`'s database as last
    /// committed, with `params` bound to its placeholders. Answers the rows as
    /// `berm.sql` does.
    pub async fn sql(
        &self,
        service: &str,
        sql: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let answer = self
            .host
            .databases
            .get(service)?
            .read(
                0,
                Statement {
                    sql: sql.to_owned(),
                    params: params.to_string(),
                },
            )
            .await?;
        Ok(serde_json::from_str(&answer.json)?)
    }
}

impl From<&table::Wake> for Wake {
    fn from(wake: &table::Wake) -> Self {
        Self {
            service: host::join(&wake.scope, &wake.name),
            program: wake.program.clone(),
            tool: wake.tool.clone(),
            args: wake.args.clone(),
        }
    }
}

impl Drop for TestHost {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What the test programs this builds call their project file.
const FIXTURE_FILE: &str = "crab.toml";

/// Build the program `package` of the workspace the current directory is in
/// as `crab build` does, and answer its finished image.
pub fn build(package: &str) -> Result<Vec<u8>> {
    let image = berm_build::build(&std::env::current_dir()?, Some(package), FIXTURE_FILE)?;
    std::fs::read(&image).with_context(|| format!("reading {}", image.display()))
}

/// Answers what the test arranged, by method and URL, and refuses the rest.
#[derive(Clone, Default)]
pub struct Arranged(Arc<Mutex<Fetches>>);

#[derive(Default)]
struct Fetches {
    answers: HashMap<(String, String), Response>,
    sent: Vec<Request>,
}

impl tower::Service<Request> for Arranged {
    type Response = Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response>> + Send>>;

    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let mut fetches = self.0.lock();
        let answer = match fetches
            .answers
            .get(&(request.method.clone(), request.url.clone()))
        {
            Some(response) => Ok(Response {
                status: response.status,
                headers: response.headers.clone(),
                body: response.body[..request
                    .limit
                    .map_or(response.body.len(), |limit| limit.min(response.body.len()))]
                    .to_vec(),
            }),
            None => Err(anyhow::anyhow!(
                "no response arranged for {} {}",
                request.method,
                request.url
            )),
        };
        fetches.sent.push(request);
        Box::pin(std::future::ready(answer))
    }
}

/// The images the test deployed, by reference.
#[derive(Clone, Default)]
pub struct Given(Arc<Mutex<HashMap<String, Vec<u8>>>>);

/// No peers: every service is deployed here or nowhere.
pub struct Nowhere;

/// The secrets the test granted, by service, secret and host.
#[derive(Clone, Default)]
pub struct Granted(Arc<Mutex<HashMap<Grant, String>>>);

/// A service, a secret and a host.
type Grant = (String, String, String);

/// A clock that stands where the test set it.
#[derive(Clone)]
pub struct Manual(Arc<AtomicU64>);
