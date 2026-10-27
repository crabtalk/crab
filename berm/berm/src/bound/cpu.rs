//! A bound on the CPU time one invocation may spend.
//!
//! Measured on the thread's own CPU clock across each poll of the invocation,
//! less what nested invocations polled inside it spent, so a call is charged
//! for its own work and not for a callee's. Enforced at each engine tick, so a
//! guest can run up to one tick past its budget before it is stopped.

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
};

thread_local! {
    /// CPU nanoseconds the invocations polled inside the current poll spent,
    /// theirs and their own callees' together.
    static NESTED: Cell<u64> = const { Cell::new(0) };
}

/// Nanoseconds of CPU this thread has used.
pub(crate) fn now() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is a valid timespec to write to, and the thread CPU clock
    // exists on every target berm runs on.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}

/// The CPU time one invocation has spent on its own.
#[derive(Debug, Default)]
pub(crate) struct Clock {
    /// Nanoseconds spent in the polls that have returned.
    spent: AtomicU64,
    /// The thread's CPU clock when the poll running now began.
    since: AtomicU64,
}

impl Clock {
    /// Nanoseconds spent so far, the poll running now included. Read on the
    /// thread polling the invocation, between its polls or inside one.
    pub(crate) fn spent(&self) -> u64 {
        let spent = self.spent.load(Ordering::Relaxed);
        match self.since.load(Ordering::Relaxed) {
            0 => spent,
            since => spent + now().saturating_sub(since).saturating_sub(NESTED.get()),
        }
    }

    /// `future`, its polls timed on this clock.
    pub(crate) fn time<F: Future>(self: &Arc<Self>, future: F) -> Timed<F> {
        Timed {
            clock: self.clone(),
            future: Box::pin(future),
        }
    }
}

/// A future whose polls are timed on a [`Clock`].
pub(crate) struct Timed<F> {
    clock: Arc<Clock>,
    future: Pin<Box<F>>,
}

impl<F: Future> Future for Timed<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let outer = NESTED.replace(0);
        let start = now();
        self.clock.since.store(start, Ordering::Relaxed);
        let poll = self.future.as_mut().poll(cx);
        let elapsed = now().saturating_sub(start);
        let own = elapsed.saturating_sub(NESTED.get());
        self.clock.since.store(0, Ordering::Relaxed);
        self.clock.spent.fetch_add(own, Ordering::Relaxed);
        NESTED.set(outer + elapsed);
        poll
    }
}
