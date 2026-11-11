//! Local WebAssembly workload measurements.

use crate::{
    format,
    host::{self, secrets::Grants},
    project, runtime, terminal,
};
use anyhow::{Context, Result, ensure};
use berm::{
    Budget, Callsite, Client as OutsideClient, Config as RuntimeConfig, Meter, Output,
    syscall::http::{Request, Response},
};
use berm_api::{Limits, Manifest, digest, host::Deploy};
use berm_host::{
    Alone, Config, Denied, Edges, Env, Host, Layers, Observe, Stored, System, fixture::Http,
};
use clap::Args;
use crabtalk_sdk::{
    client::Client,
    format as display,
    system::meta::{self, Metering, Rates},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::ExitCode,
    sync::Arc,
    task::{self, Poll},
    time::{Duration, Instant},
};
use tower::Service;

#[derive(Args)]
pub struct Bench {
    /// Directory containing the service's crab.toml.
    pub dir: PathBuf,
    pub tool: String,
    /// JSON Lines workloads, with arguments and optional HTTP fixtures.
    #[arg(long)]
    pub cases: PathBuf,
    /// Repetitions per case; each case starts with a fresh database.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    pub runs: u32,
    /// Send real HTTP requests instead of using fixtures.
    #[arg(long)]
    pub live_http: bool,
    /// Price with SDK defaults without requesting Cloud's current rates.
    #[arg(long)]
    pub offline: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: String,
    pub args: Value,
    #[serde(default)]
    pub http: Vec<HttpFixture>,
    #[serde(default)]
    pub expected_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpFixture {
    pub method: String,
    pub url: String,
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub body_file: Option<PathBuf>,
    #[serde(default)]
    pub delay_ms: u64,
}

impl Case {
    pub fn read(path: &Path) -> Result<Vec<Self>> {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        let mut cases = Vec::new();
        for (line, text) in text
            .lines()
            .enumerate()
            .filter(|(_, text)| !text.trim().is_empty())
        {
            let mut case: Self = serde_json::from_str(text)
                .with_context(|| format!("{}:{}", path.display(), line + 1))?;
            ensure!(
                !case.name.is_empty(),
                "case name is empty at line {}",
                line + 1
            );
            ensure!(
                !cases.iter().any(|prior: &Self| prior.name == case.name),
                "duplicate case {:?}",
                case.name
            );
            ensure!(
                !case.expected_error.as_ref().is_some_and(String::is_empty),
                "case {:?} has an empty expected_error",
                case.name
            );
            for response in &mut case.http {
                ensure!(
                    response.body.is_none() || response.body_file.is_none(),
                    "case {:?}: choose body or body_file",
                    case.name
                );
                if let Some(path) = &mut response.body_file {
                    *path = base.join(&path);
                }
            }
            cases.push(case);
        }
        ensure!(!cases.is_empty(), "{} contains no cases", path.display());
        Ok(cases)
    }
}

impl HttpFixture {
    fn arrange(&self, http: &Http) -> Result<()> {
        let body = match &self.body_file {
            Some(path) => {
                fs::read(path).with_context(|| format!("reading fixture {}", path.display()))?
            }
            None => self.body.as_deref().unwrap_or("").as_bytes().to_vec(),
        };
        http.respond(
            &self.method,
            &self.url,
            Response {
                status: self.status,
                headers: self
                    .headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                body,
            },
            Duration::from_millis(self.delay_ms),
        );
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Sample {
    pub own: Meter,
    pub chain: Meter,
    pub elapsed_us: u64,
    pub error: Option<String>,
    pub matched: bool,
    pub complete: bool,
    pub status: Option<u16>,
    pub explain: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct MeasuredCase {
    pub name: String,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub median: u64,
    pub max: u64,
}

impl TryFrom<Vec<u64>> for Distribution {
    type Error = anyhow::Error;

    fn try_from(mut values: Vec<u64>) -> Result<Self> {
        ensure!(!values.is_empty(), "no measurements");
        values.sort_unstable();
        Ok(Self {
            median: values[(values.len() - 1) / 2],
            max: *values.last().unwrap(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Statistics {
    pub own_cpu_us: Distribution,
    pub own_read: Distribution,
    pub own_written: Distribution,
    pub own_memory_bytes: Distribution,
    pub elapsed_us: Distribution,
    /// Whole-chain cost in USD micros, before plan inclusions.
    pub median_cost_micros_decimal: String,
    pub max_cost_micros_decimal: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub service: String,
    pub tool: String,
    pub digest: String,
    pub http: &'static str,
    pub rates_source: String,
    pub declared: Budget,
    pub ceilings: Budget,
    pub cases: Vec<MeasuredCase>,
    pub statistics: Statistics,
    pub recommended: Option<Limits>,
    pub exceeds: Option<&'static str>,
    pub passed: bool,
}

#[derive(Clone, Default)]
struct Own(Arc<Mutex<Used>>);

#[derive(Default)]
struct Used {
    own: Meter,
    chain: Meter,
}

impl Observe for Own {
    fn invoked(&self, at: &Callsite, _: Option<u64>, used: &Meter) {
        let mut measured = self.0.lock();
        measured.chain += *used;
        if at.parent.is_none() {
            measured.own = *used;
        }
    }
}

#[derive(Clone)]
enum Fetch {
    Fixtures(Http),
    Live(host::Fetch),
}

impl Service<Request> for Fetch {
    type Response = Response;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<()>> {
        match self {
            Self::Fixtures(http) => http.poll_ready(cx),
            Self::Live(http) => http.poll_ready(cx),
        }
    }

    fn call(&mut self, request: Request) -> Self::Future {
        match self {
            Self::Fixtures(http) => http.call(request),
            Self::Live(http) => Box::pin(http.call(request)),
        }
    }
}

#[derive(Clone)]
struct Local;

impl Env for Local {
    type Fetch = Fetch;
    type Images = Stored;
    type Peers = Alone;
    type Secrets = Grants;
    type Observe = Own;
    type Clock = System;
}

impl Report {
    pub async fn measure(
        image: &[u8],
        tool: &str,
        cases: &[Case],
        runs: u32,
        live_http: bool,
        metering: &Metering,
        size: u64,
    ) -> Result<Self> {
        ensure!(
            runs > 0 && !cases.is_empty(),
            "bench needs at least one case and one run"
        );
        let manifest = Manifest::try_from(image)?;
        let spec = manifest
            .tools
            .iter()
            .find(|spec| spec.name == tool)
            .with_context(|| format!("no tool {tool:?} in this image"))?;
        let defaults = RuntimeConfig::default();
        let declared = spec
            .budget
            .map_or(defaults.budget, |budget| budget.over(defaults.budget));
        let root = tempfile::tempdir()?;
        let mut measured = Vec::new();
        for case in cases {
            ensure!(
                !live_http || case.http.is_empty(),
                "case {:?} has HTTP fixtures; omit --live-http to use them",
                case.name
            );
            let directory = tempfile::tempdir_in(root.path())?;
            let fixtures = Http::default();
            for response in &case.http {
                response.arrange(&fixtures)?;
            }
            let fetch = if live_http {
                Fetch::Live(host::Fetch::new(None)?)
            } else {
                Fetch::Fixtures(fixtures.clone())
            };
            let images = Stored::open(directory.path());
            let digest = images.put(image).await?;
            let own = Own::default();
            let host = Host::<Local>::open(
                &directory.path().join("host.db"),
                &Config {
                    databases: directory.path().join("databases"),
                    cache_dir: Some(root.path().join("cache")),
                    ..Default::default()
                },
                Edges {
                    fetch: fetch.clone(),
                    proxy: Some(fetch),
                    images,
                    peers: Alone,
                    secrets: Grants::open(directory.path().join("secrets.toml")),
                    layers: Layers::new(tower::layer::util::Identity::new()),
                    enter: None,
                    observe: own.clone(),
                    clock: System,
                },
            )
            .await?;
            host.deploy(
                &manifest.name,
                Deploy {
                    reference: digest.clone(),
                    digest,
                    force: true,
                    size,
                    stopped: false,
                    check_secrets: false,
                    project: None,
                    exempt: Vec::new(),
                    vars: Default::default(),
                },
                None,
            )
            .await?;
            let mut samples = Vec::new();
            let args = serde_json::to_vec(&case.args)?;
            for _ in 0..runs {
                *own.0.lock() = Used::default();
                let started = Instant::now();
                let called = host
                    .execute(
                        &manifest.name,
                        tool,
                        args.clone(),
                        Some("local"),
                        None,
                        OutsideClient::default(),
                    )
                    .await;
                let elapsed_us = started.elapsed().as_micros() as u64;
                let complete = called.is_ok();
                let refusal = called
                    .as_ref()
                    .err()
                    .and_then(|error| error.downcast_ref::<Denied>());
                let status = refusal.map(|error| error.status.as_u16());
                let explain = refusal.and_then(|error| error.explain);
                let mut error = match called {
                    Ok(invoked) => match invoked.result {
                        Output::Done(_) => None,
                        Output::Failed(error) => Some(error),
                    },
                    Err(error) => Some(format!("{error:#}")),
                };
                let missing = fixtures.unanswered();
                if let Some(request) = missing.first() {
                    error = Some(format!(
                        "no response arranged for {} {}",
                        request.method, request.url
                    ));
                }
                let matched = match (&case.expected_error, &error) {
                    (None, None) => true,
                    (Some(expected), Some(error)) => error.contains(expected),
                    _ => false,
                } && missing.is_empty();
                let used = own.0.lock();
                samples.push(Sample {
                    own: used.own,
                    chain: used.chain,
                    elapsed_us,
                    error,
                    matched,
                    complete,
                    status,
                    explain,
                });
            }
            measured.push(MeasuredCase {
                name: case.name.clone(),
                samples,
            });
        }
        let samples: Vec<_> = measured.iter().flat_map(|case| &case.samples).collect();
        let cpu = Distribution::try_from(
            samples
                .iter()
                .map(|sample| sample.own.cpu)
                .collect::<Vec<_>>(),
        )?;
        let read = Distribution::try_from(
            samples
                .iter()
                .map(|sample| sample.own.read)
                .collect::<Vec<_>>(),
        )?;
        let written = Distribution::try_from(
            samples
                .iter()
                .map(|sample| sample.own.written)
                .collect::<Vec<_>>(),
        )?;
        let memory = Distribution::try_from(
            samples
                .iter()
                .map(|sample| sample.own.memory)
                .collect::<Vec<_>>(),
        )?;
        let elapsed = Distribution::try_from(
            samples
                .iter()
                .map(|sample| sample.elapsed_us)
                .collect::<Vec<_>>(),
        )?;
        let mut costs: Vec<_> = samples
            .iter()
            .map(|sample| {
                let overage = sample.chain.cpu.saturating_sub(metering.included_cpu);
                (
                    metering.rates.call_charge(&sample.chain, overage),
                    metering.rates.cost_decimal(&sample.chain, overage),
                )
            })
            .collect();
        costs.sort_by_key(|(cost, _)| *cost);
        let complete = samples
            .iter()
            .all(|sample| sample.matched && sample.complete && sample.own.memory > 0);
        let recommended = complete.then(|| {
            Limits::for_usage(Some(cpu.max), Some(read.max), Some(written.max), memory.max)
        });
        let exceeds =
            recommended.and_then(|limits| limits.over(defaults.budget).past(&defaults.most()));
        Ok(Self {
            service: manifest.name,
            tool: tool.to_owned(),
            digest: digest::of(image),
            http: if live_http { "live" } else { "fixtures" },
            rates_source: "supplied rates".to_owned(),
            declared,
            ceilings: defaults.most(),
            statistics: Statistics {
                own_cpu_us: cpu,
                own_read: read,
                own_written: written,
                own_memory_bytes: memory,
                elapsed_us: elapsed,
                median_cost_micros_decimal: costs[(costs.len() - 1) / 2].1.clone(),
                max_cost_micros_decimal: costs.last().unwrap().1.clone(),
            },
            cases: measured,
            recommended,
            exceeds,
            passed: complete && exceeds.is_none(),
        })
    }
}

pub fn run(cloud: &Client, bench: &Bench, json: bool) -> Result<ExitCode> {
    let cases = Case::read(&bench.cases)?;
    let project = project::read(&bench.dir)?;
    let image = fs::read(berm_build::build(&bench.dir, None)?)?;
    let (metering, source) = if bench.offline {
        (None, "SDK defaults (--offline)")
    } else {
        match runtime::wait(cloud.rates()) {
            Ok(rates) => (Some(rates), "Cloud rates"),
            Err(error) => {
                eprintln!("Cloud rates unavailable: {error}; using SDK defaults");
                (None, "SDK defaults (Cloud unavailable)")
            }
        }
    };
    let metering = metering.unwrap_or(Metering {
        rates: Rates::default(),
        included_cpu: meta::INCLUDED_CPU,
        max_log: berm_api::MAX_LOG,
    });
    let mut report = runtime::wait(Report::measure(
        &image,
        &bench.tool,
        &cases,
        bench.runs,
        bench.live_http,
        &metering,
        project.database.size(),
    ))?;
    report.rates_source = source.to_owned();
    if json {
        terminal::json(&report)?;
    } else {
        println!(
            "{}.{} · {} · {}",
            report.service, report.tool, report.http, report.rates_source
        );
        println!(
            "Local measurements; ceilings are runtime defaults. Costs are whole-chain estimates before plan inclusions."
        );
        println!(
            "Initial memory reservation: {}. Rerun after changing budgets.",
            display::bytes(report.declared.memory)
        );
        println!("{:<20} {:>14} {:>14}", "RESOURCE", "MEDIAN", "MAXIMUM");
        for (label, value) in [
            ("own CPU (µs)", &report.statistics.own_cpu_us),
            ("own rows read", &report.statistics.own_read),
            ("own rows written", &report.statistics.own_written),
            ("own memory (bytes)", &report.statistics.own_memory_bytes),
            ("elapsed (µs)", &report.statistics.elapsed_us),
        ] {
            println!("{label:<20} {:>14} {:>14}", value.median, value.max);
        }
        println!(
            "estimated cost       {} median, {} maximum",
            format::usd(report.statistics.median_cost_micros_decimal.parse()?),
            format::usd(report.statistics.max_cost_micros_decimal.parse()?)
        );
        for case in &report.cases {
            let failed = case.samples.iter().filter(|sample| !sample.matched).count();
            let incomplete = case
                .samples
                .iter()
                .filter(|sample| !sample.complete)
                .count();
            println!(
                "{}: {} measured, {failed} unexpected failures, {incomplete} incomplete",
                case.name,
                case.samples.len()
            );
            for error in case
                .samples
                .iter()
                .filter(|sample| !sample.matched || !sample.complete)
                .filter_map(|sample| sample.error.as_deref())
                .take(1)
            {
                println!("  {error}");
            }
        }
        if let Some(limits) = report.recommended {
            println!("suggested budget = {limits}");
            if let Some(key) = report.exceeds {
                println!("does not fit runtime defaults: {key} exceeds the ceiling");
            }
        } else {
            println!(
                "No budget suggestion: a case failed or did not produce a complete measurement."
            );
        }
    }
    Ok(if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    })
}
