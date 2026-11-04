use anyhow::Result;
use clap::{Parser, error::ErrorKind};
use crab_agent::cmd::call::Call;
use crabtalk_sdk::client::{Client, Owner};

#[derive(Parser)]
struct Command {
    #[command(flatten)]
    call: Call,
}

#[test]
fn local_calls_default_to_local_but_cloud_uses_its_credential() -> Result<()> {
    let call = Command::try_parse_from(["call", "notes", "list", "{}"])?.call;
    let local = Client::new("http://127.0.0.1:7777");
    let caller = call.caller(&local)?.unwrap();
    assert_eq!(caller.id, "local");
    assert!(caller.user.is_none());

    let cloud = local
        .owner(Owner::User("alice".to_owned()))
        .token("session");
    assert!(call.caller(&cloud)?.is_none());
    Ok(())
}

#[test]
fn local_caller_can_be_overridden() -> Result<()> {
    let call = Command::try_parse_from(["call", "notes", "list", "{}", "--caller", "alice"])?.call;
    let client = Client::new("http://127.0.0.1:7777");
    assert_eq!(call.caller(&client)?.unwrap().id, "alice");
    Ok(())
}

#[test]
fn anonymous_calls_have_no_caller() -> Result<()> {
    let call = Command::try_parse_from(["call", "notes", "list", "{}", "--anonymous"])?.call;
    let local = Client::new("http://127.0.0.1:7777");
    assert!(call.caller(&local)?.is_none());

    Ok(())
}

#[test]
fn end_users_can_use_the_default_local_caller() -> Result<()> {
    let call = Command::try_parse_from([
        "call",
        "notes",
        "list",
        "{}",
        "--issuer",
        "https://id.example",
        "--user",
        "42",
    ])?
    .call;
    let client = Client::new("http://127.0.0.1:7777");
    let caller = call.caller(&client)?.unwrap();
    assert_eq!(caller.id, "local");
    let user = caller.user.unwrap();
    assert_eq!(user.issuer, "https://id.example");
    assert_eq!(user.subject, "42");
    Ok(())
}

#[test]
fn anonymous_conflicts_with_identity_flags() {
    for identity in [
        vec!["--caller", "alice"],
        vec!["--issuer", "https://id.example", "--user", "42"],
    ] {
        let error = Command::try_parse_from(
            ["call", "notes", "list", "{}", "--anonymous"]
                .into_iter()
                .chain(identity),
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }
}

#[test]
fn cloud_rejects_local_identity_flags() -> Result<()> {
    let cloud = Client::new("https://api.example")
        .owner(Owner::User("alice".to_owned()))
        .token("session");
    for identity in [
        vec!["--anonymous"],
        vec!["--caller", "alice"],
        vec!["--issuer", "https://id.example", "--user", "42"],
    ] {
        let call =
            Command::try_parse_from(["call", "notes", "list", "{}"].into_iter().chain(identity))?
                .call;
        assert_eq!(
            call.caller(&cloud).unwrap_err().to_string(),
            "caller identity flags are only available on the local host; Cloud uses your credential"
        );
    }
    Ok(())
}
