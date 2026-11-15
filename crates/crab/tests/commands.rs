use std::process::Command;

#[test]
fn push_is_the_only_publication_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["push", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("Usage: crab push"), "{help}");
    assert!(help.contains("--notes"));
    assert!(!help.contains("--stopped"));

    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["publish", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("unrecognized subcommand 'publish'"),
        "{error}"
    );
}

#[test]
fn command_options_keep_publication_and_instances_separate() {
    for args in [
        ["push", "--stopped"],
        ["push", "--from"],
        ["push", "--auto-upgrade"],
        ["push", "--build"],
        ["deploy", "--notes"],
        ["deploy", "--stopped"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_crab"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("unexpected argument"), "{error}");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["deploy", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("--stopped")
    );
}
