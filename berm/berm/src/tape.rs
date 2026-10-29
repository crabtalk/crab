//! A tape: one invocation's answers from outside the guest, in the order it got
//! them, kept so the invocation can be run again against them.
//!
//! Every hosted syscall is recorded with a sha256 of its request as the guest
//! made it, before a host substitutes anything into it, and the answer it got;
//! the clock and the random source with what they returned. A replay answers
//! each from the tape and refuses a request whose digest differs.

use crate::{Callsite, Refused, abi};
use anyhow::{Context, Result, bail};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::Arc};

/// The most bytes of answers one tape holds. Past it the tape stops recording
/// and is marked incomplete.
pub const MAX_TAPE: usize = 4 << 20;

/// One invocation's answers, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tape {
    pub entries: Vec<Entry>,
    /// Every answer is recorded; `false` once [`MAX_TAPE`] cut it.
    pub complete: bool,
}

/// One answer from outside the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The syscall's number, as [`abi::hash`] gives it.
    pub syscall: u64,
    /// sha256 of the request; zero for the clock and the random source.
    pub asked: [u8; 32],
    pub answer: Answer,
}

/// What a syscall answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The bytes staged for the guest, or the random source's.
    Done(Vec<u8>),
    /// It ran and failed.
    Failed(String),
    /// It refused, and nothing ran.
    Refused(String),
    /// The clock's reading.
    Number(u64),
}

impl Answer {
    /// What a syscall's reply is recorded as.
    pub(crate) fn of(reply: &Result<Vec<u8>>) -> Self {
        match reply {
            Ok(bytes) => Answer::Done(bytes.clone()),
            Err(error)
                if error
                    .chain()
                    .any(|cause| cause.downcast_ref::<Refused>().is_some()) =>
            {
                Answer::Refused(error.to_string())
            }
            Err(error) => Answer::Failed(error.to_string()),
        }
    }

    /// The reply a recorded answer stands for.
    pub(crate) fn reply(self) -> Result<Vec<u8>> {
        match self {
            Answer::Done(bytes) => Ok(bytes),
            Answer::Failed(message) => Err(anyhow::anyhow!(message)),
            Answer::Refused(message) => Err(Refused(message).into()),
            Answer::Number(number) => Ok(number.to_le_bytes().to_vec()),
        }
    }
}

/// What an invocation does with a tape.
pub enum Taping {
    Off,
    /// Record one into the recorder, however the invocation ends.
    Record(Recorder),
    /// Answer from `tape`. The doors of the syscalls in `through` run too,
    /// their answers discarded: a replay's logs are emitted where it runs.
    Replay {
        tape: Tape,
        through: Vec<String>,
    },
}

/// A tape being recorded or played, shared by an invocation's syscalls.
pub(crate) type Deck = Arc<Mutex<Playing>>;

pub(crate) enum Playing {
    Recording {
        tape: Tape,
        bytes: usize,
    },
    Replaying {
        tape: Tape,
        at: usize,
        through: HashSet<u64>,
    },
}

/// A tape being recorded, read by whoever started the invocation once it
/// has ended, trapped or not.
#[derive(Clone)]
pub struct Recorder(Deck);

impl Default for Recorder {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Playing::Recording {
            tape: Tape {
                entries: Vec::new(),
                complete: true,
            },
            bytes: 0,
        })))
    }
}

impl Recorder {
    /// What was recorded, taken out.
    pub fn tape(&self) -> Tape {
        self.0.lock().recorded().unwrap_or_default()
    }
}

impl Taping {
    pub(crate) fn deck(self) -> Option<Deck> {
        let playing = match self {
            Taping::Off => return None,
            Taping::Record(recorder) => return Some(recorder.0),
            Taping::Replay { tape, through } => Playing::Replaying {
                tape,
                at: 0,
                through: through.iter().map(|name| abi::hash(name)).collect(),
            },
        };
        Some(Arc::new(Mutex::new(playing)))
    }
}

impl Playing {
    /// Add `answer` to `syscall`, asked as `asked`, on a tape being recorded.
    pub(crate) fn record(&mut self, syscall: u64, asked: [u8; 32], answer: Answer) {
        let Playing::Recording { tape, bytes } = self else {
            return;
        };
        if !tape.complete {
            return;
        }
        *bytes += match &answer {
            Answer::Done(bytes) => bytes.len(),
            Answer::Failed(text) | Answer::Refused(text) => text.len(),
            Answer::Number(_) => 8,
        } + 41;
        if *bytes > MAX_TAPE {
            tape.complete = false;
            return;
        }
        tape.entries.push(Entry {
            syscall,
            asked,
            answer,
        });
    }

    /// The recorded answer to the next syscall, refused when it is not
    /// `syscall` asked as `digest`. `None` while recording.
    pub(crate) fn replay(
        &mut self,
        at_site: &Callsite,
        syscall: u64,
        digest: [u8; 32],
    ) -> Option<Result<Answer>> {
        let Playing::Replaying { tape, at, .. } = self else {
            return None;
        };
        let position = *at + 1;
        let Some(entry) = tape.entries.get(*at) else {
            return Some(Err(match tape.complete {
                true => anyhow::anyhow!(
                    "diverged at syscall {position}: asked {}, and the call made no more",
                    name(at_site, syscall)
                ),
                false => anyhow::anyhow!(
                    "the tape was cut at syscall {position}, where the recording stopped"
                ),
            }));
        };
        if entry.syscall != syscall || entry.asked != digest {
            return Some(Err(anyhow::anyhow!(
                "diverged at syscall {position}: asked {} {}, recorded {} {}",
                name(at_site, syscall),
                hex::encode(&digest[..6]),
                name(at_site, entry.syscall),
                hex::encode(&entry.asked[..6]),
            )));
        }
        *at += 1;
        Some(Ok(entry.answer.clone()))
    }

    /// Whether a replay runs `syscall`'s door too.
    pub(crate) fn through(&self, syscall: u64) -> bool {
        matches!(self, Playing::Replaying { through, .. } if through.contains(&syscall))
    }

    /// The tape recorded, taken out.
    pub(crate) fn recorded(&mut self) -> Option<Tape> {
        match self {
            Playing::Recording { tape, .. } => Some(std::mem::take(tape)),
            Playing::Replaying { .. } => None,
        }
    }
}

/// sha256 of `request`, or zero for none.
pub(crate) fn asked(request: &[u8]) -> [u8; 32] {
    match request.is_empty() {
        true => [0; 32],
        false => Sha256::digest(request).into(),
    }
}

/// `syscall`'s name, among berm's own and those `at`'s program declares.
fn name(at: &Callsite, syscall: u64) -> String {
    [
        abi::CALL,
        abi::CALL_AFTER,
        abi::HTTP_FETCH,
        abi::HTTP_PROXY,
        abi::HTTP_FETCH_ALL,
        abi::HTTP_PROXY_ALL,
        abi::EMIT,
        abi::SQL,
        abi::GET,
        abi::SET,
        "berm.now",
        "berm.random",
    ]
    .into_iter()
    .chain(at.deps.syscalls.keys().map(String::as_str))
    .find(|name| abi::hash(name) == syscall)
    .map_or_else(|| format!("syscall {syscall:#x}"), str::to_owned)
}

/// The tape as bytes: `1` or `0` for complete, then each entry as its
/// syscall's number, its digest, a kind byte, and a little-endian number or a
/// `u32`-prefixed body.
impl From<&Tape> for Vec<u8> {
    fn from(tape: &Tape) -> Self {
        let mut bytes = vec![u8::from(tape.complete)];
        for entry in &tape.entries {
            bytes.extend(entry.syscall.to_le_bytes());
            bytes.extend(entry.asked);
            let (kind, body): (u8, &[u8]) = match &entry.answer {
                Answer::Done(body) => (0, body),
                Answer::Failed(text) => (1, text.as_bytes()),
                Answer::Refused(text) => (2, text.as_bytes()),
                Answer::Number(number) => {
                    bytes.push(3);
                    bytes.extend(number.to_le_bytes());
                    continue;
                }
            };
            bytes.push(kind);
            bytes.extend((body.len() as u32).to_le_bytes());
            bytes.extend(body);
        }
        bytes
    }
}

impl TryFrom<&[u8]> for Tape {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let mut rest = bytes;
        let mut take = |n: usize| -> Result<&[u8]> {
            let (head, tail) = rest.split_at_checked(n).context("the tape is cut short")?;
            rest = tail;
            Ok(head)
        };
        let complete = match take(1)?[0] {
            0 => false,
            1 => true,
            other => bail!("a tape does not open with {other}"),
        };
        let mut entries = Vec::new();
        let mut left = bytes.len() - 1;
        while left > 0 {
            let syscall = u64::from_le_bytes(take(8)?.try_into()?);
            let asked: [u8; 32] = take(32)?.try_into()?;
            let kind = take(1)?[0];
            let (answer, read) = match kind {
                3 => (Answer::Number(u64::from_le_bytes(take(8)?.try_into()?)), 8),
                0..=2 => {
                    let length = u32::from_le_bytes(take(4)?.try_into()?) as usize;
                    let body = take(length)?.to_vec();
                    let answer = match kind {
                        0 => Answer::Done(body),
                        1 => Answer::Failed(String::from_utf8(body)?),
                        _ => Answer::Refused(String::from_utf8(body)?),
                    };
                    (answer, 4 + length)
                }
                other => bail!("a tape's answer is not of kind {other}"),
            };
            left -= 41 + read;
            entries.push(Entry {
                syscall,
                asked,
                answer,
            });
        }
        Ok(Tape { entries, complete })
    }
}
