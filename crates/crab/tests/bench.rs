use anyhow::Result;
use berm_api::host;
use berm_build::fixture;
use crab_agent::cmd::bench::{Case, Report};
use crabtalk_sdk::system::meta::{self, Metering, Rates};
use serde_json::json;
use std::fs;

fn metering() -> Metering {
    Metering {
        rates: Rates::default(),
        included_cpu: meta::INCLUDED_CPU,
        max_log: berm_api::MAX_LOG,
    }
}

#[tokio::test]
async fn measures_the_image_and_recommends_an_unclipped_budget() -> Result<()> {
    let case = serde_json::from_value(json!({"name":"echo", "args":{"query":"hello"}}))?;
    let report = Report::measure(
        &fixture::image("berm-fixture")?,
        "echo",
        &[case],
        3,
        false,
        &metering(),
        host::DATABASE_SIZE,
    )
    .await?;
    assert!(report.passed, "{report:?}");
    assert_eq!(report.cases[0].samples.len(), 3);
    assert!(report.statistics.own_memory_bytes.max > 0);
    assert!(report.recommended.unwrap().cpu.unwrap() >= report.statistics.own_cpu_us.max);
    for sample in &report.cases[0].samples {
        assert_eq!(sample.chain.runs, 1);
        assert!(sample.complete);
    }
    Ok(())
}

#[tokio::test]
async fn fixture_waits_hold_the_declared_memory_and_are_priced_by_the_sdk() -> Result<()> {
    let case = serde_json::from_value(
        json!({"name":"fetch", "args":{"url":"http://127.0.0.1/page"}, "http":[{"method":"GET", "url":"http://127.0.0.1/page", "status":200, "headers":{"content-type":"text/html"}, "body":"hello", "delay_ms":20}]}),
    )?;
    let rates = metering();
    let report = Report::measure(
        &fixture::image("berm-fixture")?,
        "fetch",
        &[case],
        1,
        false,
        &rates,
        host::DATABASE_SIZE,
    )
    .await?;
    assert!(report.passed, "{report:?}");
    let sample = &report.cases[0].samples[0];
    assert!(sample.chain.held >= 128 * 20_000);
    assert_eq!(
        report.statistics.max_cost_micros_decimal,
        rates.rates.cost_decimal(
            &sample.chain,
            sample.chain.cpu.saturating_sub(rates.included_cpu)
        )
    );
    Ok(())
}

#[tokio::test]
async fn a_missing_fixture_cannot_be_accepted_as_an_expected_tool_error() -> Result<()> {
    let case = serde_json::from_value(
        json!({"name":"missing", "args":{"url":"http://127.0.0.1/page"}, "expected_error":"no response arranged"}),
    )?;
    let report = Report::measure(
        &fixture::image("berm-fixture")?,
        "fetch",
        &[case],
        1,
        false,
        &metering(),
        host::DATABASE_SIZE,
    )
    .await?;
    assert!(!report.passed);
    assert!(report.recommended.is_none());
    Ok(())
}

#[tokio::test]
async fn tool_errors_are_measured_but_budget_stopped_calls_are_incomplete() -> Result<()> {
    let image = fixture::image("berm-fixture")?;
    let returned = serde_json::from_value(
        json!({"name":"returned", "args":{}, "expected_error":"boom, as requested"}),
    )?;
    let report = Report::measure(
        &image,
        "boom",
        &[returned],
        1,
        false,
        &metering(),
        host::DATABASE_SIZE,
    )
    .await?;
    assert!(report.passed, "{report:?}");
    assert!(report.recommended.is_some());
    let stopped = serde_json::from_value(
        json!({"name":"stopped", "args":{"rounds":u64::MAX}, "expected_error":"budget"}),
    )?;
    let report = Report::measure(
        &image,
        "spin",
        &[stopped],
        1,
        false,
        &metering(),
        host::DATABASE_SIZE,
    )
    .await?;
    assert!(!report.passed);
    assert!(!report.cases[0].samples[0].complete);
    assert!(
        report.cases[0].samples[0]
            .error
            .as_ref()
            .unwrap()
            .contains("service trapped in spin")
    );
    assert!(report.statistics.own_cpu_us.max > 0);
    assert!(report.recommended.is_none());
    Ok(())
}

#[test]
fn body_files_are_relative_to_the_cases_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let cases = dir.path().join("cases.jsonl");
    fs::write(&cases, json!({"name":"page", "args":{}, "http":[{"method":"GET", "url":"https://example.com", "status":200, "body_file":"page.html"}]}).to_string())?;
    assert_eq!(
        Case::read(&cases)?[0].http[0].body_file,
        Some(dir.path().join("page.html"))
    );
    Ok(())
}
