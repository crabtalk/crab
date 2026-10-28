//! `crab secret fill`'s form: one step per credential, laid out as crabtalk's
//! `/secrets` page lays them out, drawn in the terminal.

pub mod guide;
mod view;

use crate::utils;
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use berm_api::{Credential, CredentialSecret};
use crabtalk_sdk::{client::Client, sandbox::service::Hosts};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    text::Text,
};
use regex::Regex;
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

/// The form for one service.
pub struct Form<'a> {
    client: &'a Client,
    service: String,
    /// Where values go, as the person reads it.
    target: String,
    steps: Vec<Step>,
    /// The step on screen.
    at: usize,
    /// The row focused on it: its copy values, then its fields.
    focus: usize,
    /// What the last action said, and whether it went wrong.
    said: Option<(String, bool)>,
    /// The titles saved, in order, for the record left on leaving.
    saved: Vec<String>,
}

/// One credential.
struct Step {
    title: String,
    guide: Text<'static>,
    /// Each value the person enters at the provider, beside its label.
    copy: Vec<(String, String)>,
    fields: Vec<Field>,
    /// The hosts its secrets are sent to, as typed: names apart by spaces or
    /// commas.
    hosts: String,
    saved: bool,
}

impl Step {
    /// The hosts typed, each named once.
    fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = Vec::new();
        for host in self.hosts.split([' ', ',']).filter(|host| !host.is_empty()) {
            if !hosts.iter().any(|one| one == host) {
                hosts.push(host.to_owned());
            }
        }
        hosts
    }
}

/// One secret of a credential.
struct Field {
    secret: CredentialSecret,
    typed: String,
    /// Whether it is set on the host and sent somewhere.
    set: bool,
    pattern: Option<Regex>,
}

impl Field {
    /// What the provider calls it.
    fn label(&self) -> &str {
        match self.secret.label.is_empty() {
            true => &self.secret.name,
            false => &self.secret.label,
        }
    }

    /// Whether the typed value misses the declared pattern.
    fn off(&self) -> bool {
        !self.typed.is_empty()
            && self
                .pattern
                .as_ref()
                .is_some_and(|pattern| !pattern.is_match(&self.typed))
    }
}

impl<'a> Form<'a> {
    /// `credentials` of `service`, set on the host `client` talks to; `copy`
    /// holds the values their copy keys take.
    pub fn new(
        client: &'a Client,
        service: &str,
        credentials: Vec<(String, Credential)>,
        copy: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let target = match &client.owner {
            None => "the local host on this machine".to_owned(),
            Some(owner) => format!("Cloud, the account {}", owner.name()),
        };
        let set = utils::wait(client.secrets(service))?;
        let steps = credentials
            .into_iter()
            .map(|(id, credential)| Step {
                hosts: credential
                    .secrets
                    .iter()
                    .find_map(|secret| {
                        set.iter()
                            .find(|one| one.name == secret.name && one.sent())
                            .map(|one| &one.hosts)
                    })
                    .unwrap_or(&credential.hosts)
                    .join(", "),
                title: match credential.title.is_empty() {
                    true => id,
                    false => credential.title.clone(),
                },
                guide: guide::render(&credential.guide),
                copy: credential
                    .copy
                    .iter()
                    .filter_map(|(key, label)| Some((label.clone(), copy.get(key)?.clone())))
                    .collect(),
                fields: credential
                    .secrets
                    .into_iter()
                    .map(|secret| Field {
                        typed: String::new(),
                        set: set.iter().any(|one| one.name == secret.name && one.sent()),
                        pattern: secret.pattern.as_deref().and_then(|p| Regex::new(p).ok()),
                        secret,
                    })
                    .collect(),
                saved: false,
            })
            .collect();
        let mut form = Self {
            client,
            service: service.to_owned(),
            target,
            steps,
            at: 0,
            focus: 0,
            said: None,
            saved: Vec::new(),
        };
        form.go(0);
        Ok(form)
    }

    /// Run the form until the person leaves it, then say what was saved.
    pub fn run(mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let ran = self.ask(&mut terminal);
        ratatui::restore();
        match self.saved.is_empty() {
            true => eprintln!("nothing saved for {}", self.service),
            false => eprintln!(
                "saved {} for {}, on {}",
                self.saved.join(", "),
                self.service,
                self.target
            ),
        }
        ran
    }

    fn ask(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| view::draw(self, frame))?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Press && !self.key(key)? {
                return Ok(());
            }
        }
    }

    /// Act on `key`; `false` to leave.
    fn key(&mut self, key: KeyEvent) -> Result<bool> {
        let copies = self.steps[self.at].copy.len();
        let fields = self.steps[self.at].fields.len();
        let rows = copies + fields + 1;
        match key.code {
            KeyCode::Esc => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false);
            }
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % rows,
            KeyCode::BackTab | KeyCode::Up => self.focus = (self.focus + rows - 1) % rows,
            KeyCode::Left => self.go(self.at.saturating_sub(1)),
            KeyCode::Right => self.go((self.at + 1).min(self.steps.len() - 1)),
            KeyCode::Enter | KeyCode::Char('c') if self.focus < copies => self.copy(self.focus)?,
            KeyCode::Enter => self.save()?,
            KeyCode::Char(c) if self.focus >= copies => self.typed().push(c),
            KeyCode::Backspace if self.focus >= copies => {
                self.typed().pop();
            }
            _ => {}
        }
        Ok(true)
    }

    /// What the focused row holds as typed: a field's value, or the hosts
    /// after the last field.
    fn typed(&mut self) -> &mut String {
        let step = &mut self.steps[self.at];
        let row = self.focus - step.copy.len();
        match step.fields.get_mut(row) {
            Some(field) => &mut field.typed,
            None => &mut step.hosts,
        }
    }

    /// Show step `at`, focused on its first field.
    fn go(&mut self, at: usize) {
        self.at = at;
        self.focus = self.steps[at].copy.len();
        self.said = None;
    }

    /// Put copy value `row` of the step on the clipboard, through the
    /// terminal.
    fn copy(&mut self, row: usize) -> Result<()> {
        let (label, value) = &self.steps[self.at].copy[row];
        let mut out = io::stdout();
        write!(out, "\x1b]52;c;{}\x07", STANDARD.encode(value))?;
        out.flush()?;
        self.said = Some((format!("copied the {label}"), false));
        Ok(())
    }

    /// Save the step on screen, then show the next one not saved: each value
    /// typed, and the hosts on every secret of it.
    fn save(&mut self) -> Result<()> {
        let step = &self.steps[self.at];
        if step
            .fields
            .iter()
            .any(|field| !field.set && field.typed.is_empty())
        {
            self.said = Some((
                "type a value for each field that is not set".to_owned(),
                true,
            ));
            return Ok(());
        }
        let hosts = step.hosts();
        if let Err(why) = Hosts::check(&hosts) {
            self.said = Some((why, true));
            return Ok(());
        }
        if hosts.is_empty() {
            self.said = Some(("type the hosts its secrets are sent to".to_owned(), true));
            return Ok(());
        }
        let fields: Vec<(String, String, bool)> = step
            .fields
            .iter()
            .map(|field| (field.secret.name.clone(), field.typed.clone(), field.set))
            .collect();
        for (name, typed, set) in &fields {
            let written = (|| -> Result<()> {
                match (typed.is_empty(), *set) {
                    (true, _) => {
                        utils::wait(self.client.put_secret_hosts(&self.service, name, &hosts))?
                    }
                    (false, true) => {
                        utils::wait(self.client.delete_secret(&self.service, name))?;
                        utils::wait(self.client.put_secret(&self.service, name, typed, &hosts))?;
                    }
                    (false, false) => {
                        utils::wait(self.client.put_secret(&self.service, name, typed, &hosts))?
                    }
                }
                Ok(())
            })();
            if let Err(error) = written {
                self.said = Some((format!("{error:#}"), true));
                return Ok(());
            }
        }
        let step = &mut self.steps[self.at];
        for field in &mut step.fields {
            field.set = true;
            field.typed.clear();
        }
        step.saved = true;
        let title = step.title.clone();
        self.saved.push(title.clone());
        self.next();
        self.said = Some((format!("saved {title}"), false));
        Ok(())
    }

    /// Show the first step after this one that is not saved, wrapping round.
    fn next(&mut self) {
        let count = self.steps.len();
        if let Some(at) = (1..count)
            .map(|offset| (self.at + offset) % count)
            .find(|at| !self.steps[*at].saved)
        {
            self.go(at);
        }
    }
}
