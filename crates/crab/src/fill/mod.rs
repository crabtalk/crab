//! `crab secret fill`'s form: one step per credential, as crabtalk's
//! `/secrets` page lays them out, asked line by line at the terminal so the
//! guide and the copy values stay in its scrollback.

pub mod guide;
pub mod target;

use crate::fill::target::Target;
use anyhow::Result;
use berm_api::Credential;
use crabtalk_sdk::client::Client;
use regex::Regex;
use std::{collections::BTreeMap, io};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Ask for each of `service`'s `credentials` in turn and save each answered
/// to `target`, which holds `stored` now; `copy` holds the values their copy
/// keys take.
pub fn run(
    client: &Client,
    service: &str,
    target: &Target,
    credentials: &[(String, Credential)],
    mut stored: BTreeMap<String, Option<String>>,
    copy: &BTreeMap<String, String>,
) -> Result<()> {
    let title = |id: &str, credential: &Credential| match credential.title.is_empty() {
        true => id.to_owned(),
        false => credential.title.clone(),
    };
    let count = credentials.len();
    eprintln!("{DIM}SAVING TO{RESET}  {target}");
    eprintln!("{DIM}SERVICE{RESET}    {service}");
    eprintln!("{DIM}STEPS{RESET}");
    for (at, (id, credential)) in credentials.iter().enumerate() {
        let set = credential
            .secrets
            .iter()
            .all(|s| stored.contains_key(&s.name));
        let status = if set { "set" } else { "not set" };
        eprintln!(
            "  {DIM}{:02}{RESET} {}  {DIM}{status}{RESET}",
            at + 1,
            title(id, credential)
        );
    }
    eprintln!("{DIM}Enter on a field keeps it; enter on every field skips the step.{RESET}");

    let mut saved = Vec::new();
    for (at, (id, credential)) in credentials.iter().enumerate() {
        let title = title(id, credential);
        eprintln!(
            "\n{BOLD}{title}{RESET}  {DIM}step {} of {count}{RESET}\n",
            at + 1
        );
        if !credential.guide.trim().is_empty() {
            eprintln!("{}", guide::render(&credential.guide, "  "));
        }
        for (key, label) in &credential.copy {
            if let Some(value) = copy.get(key) {
                eprintln!("  {DIM}{}{RESET}\n  {value}\n", label.to_uppercase());
            }
        }
        let mut values = Vec::new();
        for secret in &credential.secrets {
            let mut label = match secret.label.is_empty() {
                true => secret.name.clone(),
                false => secret.label.clone(),
            };
            if label != secret.name {
                label = format!("{label} {DIM}{}{RESET}", secret.name);
            }
            let now = match stored.get(&secret.name) {
                Some(Some(value)) if !secret.masked => format!(" {DIM}[{value}]{RESET}"),
                Some(_) => format!(" {DIM}[set]{RESET}"),
                None => String::new(),
            };
            let pattern = secret.pattern.as_deref().and_then(|p| Regex::new(p).ok());
            loop {
                let prompt = format!("  {label}{now}: ");
                let value = match secret.masked {
                    true => rpassword::prompt_password(&prompt)?,
                    false => {
                        eprint!("{prompt}");
                        let mut line = String::new();
                        io::stdin().read_line(&mut line)?;
                        line
                    }
                };
                let value = value.trim().to_owned();
                let unchanged = matches!(stored.get(&secret.name), Some(Some(v)) if *v == value);
                if value.is_empty() || unchanged {
                    break;
                }
                if pattern.as_ref().is_some_and(|p| !p.is_match(&value))
                    && !confirm(&format!(
                        "  Doesn't look like {title}'s {}. Keep it?",
                        if secret.label.is_empty() {
                            &secret.name
                        } else {
                            &secret.label
                        }
                    ))?
                {
                    continue;
                }
                values.push((secret.name.clone(), value));
                break;
            }
        }
        if values.is_empty() {
            eprintln!("  {DIM}kept as it is{RESET}");
            continue;
        }
        if credential
            .secrets
            .iter()
            .any(|s| !stored.contains_key(&s.name) && !values.iter().any(|(n, _)| *n == s.name))
        {
            eprintln!("  {DIM}skipped: each field that is not set needs a value{RESET}");
            continue;
        }
        target.save(client, service, &values, &stored)?;
        for (name, value) in values {
            let masked = credential
                .secrets
                .iter()
                .any(|s| s.name == name && s.masked);
            stored.insert(name, (!masked).then_some(value));
        }
        eprintln!("  saved {title}");
        saved.push(title);
    }
    match saved.is_empty() {
        true => eprintln!("\nnothing saved for {service}"),
        false => eprintln!("\nsaved {} for {service}, in {target}", saved.join(", ")),
    }
    Ok(())
}

/// Ask `question` at the terminal; yes only for `y` or `yes`.
fn confirm(question: &str) -> Result<bool> {
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
