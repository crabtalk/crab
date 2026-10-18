//! `crab login` — sign in to Cloud in the browser, and keep the session as a
//! context.

use crate::{
    context::{self, Context, Contexts},
    utils,
};
use anyhow::{Context as _, Result, bail};
use crabtalk_sdk::client::{Client, Owner};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

/// The Cloud `crab login` signs in to unless told another.
pub const CLOUD: &str = "https://api.crabtalk.ai";

/// Sign in to `host` through Google, or GitHub with `github`, and use the
/// session as context `name` from now on.
pub fn run(name: &str, host: &str, github: bool) -> Result<()> {
    if name == context::DEFAULT {
        bail!("{:?} is this machine's bermd", context::DEFAULT);
    }
    let host = host.trim_end_matches('/');
    let listener = TcpListener::bind("127.0.0.1:0").context("listening for the sign-in")?;
    let port = listener.local_addr()?.port();
    let provider = if github { "github" } else { "google" };
    utils::open(&format!(
        "{host}/auth/{provider}?client=terminal&port={port}"
    ));
    eprintln!("waiting for the browser to sign in");
    let (token, browser) = receive(&listener)?;

    let me = match utils::wait(Client::new(host).token(&token).me()) {
        Ok(me) => me,
        Err(e) => {
            answer(browser, "400 Bad Request", "Sign-in failed. Return to your terminal.")?;
            bail!("sign-in failed: {e}");
        }
    };
    answer(browser, "200 OK", "Signed in. Return to your terminal.")?;
    let who = me.user.handle.unwrap_or(me.user.id);
    let mut contexts = Contexts::load()?;
    contexts.contexts.insert(
        name.to_owned(),
        Context {
            host: host.to_owned(),
            token: Some(token),
            owner: Some(Owner::User(who.clone())),
        },
    );
    contexts.current = Some(name.to_owned());
    contexts.save()?;
    eprintln!("signed in as {who} on {host}, as context {name}");
    Ok(())
}

/// The token Cloud hands back at `/callback?token=`, with the browser's request
/// left to answer, or the error it names at `/callback?error=`. Any other
/// request is answered `404` and waited past.
fn receive(listener: &TcpListener) -> Result<(String, TcpStream)> {
    loop {
        let (stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        let target = line.split_whitespace().nth(1).unwrap_or_default();
        let Some(query) = target.strip_prefix("/callback?") else {
            answer(stream, "404 Not Found", "")?;
            continue;
        };
        let field = |key: &str| {
            query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(k, _)| *k == key)
                .map(|(_, v)| decode(v))
        };
        if let Some(token) = field("token") {
            return Ok((token, stream));
        }
        let error = field("error").unwrap_or_else(|| "no token".to_owned());
        answer(
            stream,
            "400 Bad Request",
            "Sign-in failed. Return to your terminal.",
        )?;
        bail!("sign-in failed: {error}");
    }
}

fn answer(mut stream: TcpStream, status: &str, body: &str) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )?;
    Ok(())
}

/// A query value with its `%XX` escapes and `+` undone.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&value[i + 1..i + 3], 16) {
                Ok(byte) => {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
