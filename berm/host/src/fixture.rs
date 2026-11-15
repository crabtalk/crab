//! HTTP responses supplied to an isolated host.

use anyhow::{Result, anyhow};
use berm::syscall::http::{Request, Response};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{self, Poll},
    time::Duration,
};
use tokio::sync::Notify;
use tower::Service;

#[derive(Clone, Default)]
pub struct Http(Arc<Mutex<Fetches>>);

#[derive(Default)]
struct Fetches {
    answers: HashMap<(String, String), Answer>,
    sent: Vec<Request>,
}

struct Answer {
    response: Response,
    delay: Duration,
    ready: Option<Arc<Notify>>,
}

impl Http {
    pub fn respond(&self, method: &str, url: &str, response: Response, delay: Duration) {
        self.0.lock().answers.insert(
            (method.to_owned(), url.to_owned()),
            Answer {
                response,
                delay,
                ready: None,
            },
        );
    }

    pub fn respond_blocked(&self, method: &str, url: &str, response: Response) -> Arc<Notify> {
        let ready = Arc::new(Notify::new());
        self.0.lock().answers.insert(
            (method.to_owned(), url.to_owned()),
            Answer {
                response,
                delay: Duration::ZERO,
                ready: Some(ready.clone()),
            },
        );
        ready
    }

    pub fn fetched(&self) -> Vec<Request> {
        self.0.lock().sent.clone()
    }

    pub fn unanswered(&self) -> Vec<Request> {
        let fetches = self.0.lock();
        fetches
            .sent
            .iter()
            .filter(|request| {
                !fetches
                    .answers
                    .contains_key(&(request.method.clone(), request.url.clone()))
            })
            .cloned()
            .collect()
    }
}

impl Service<Request> for Http {
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
            Some(Answer {
                response,
                delay,
                ready,
            }) => Ok((
                Response {
                    status: response.status,
                    headers: response.headers.clone(),
                    body: response.body[..request
                        .limit
                        .map_or(response.body.len(), |limit| limit.min(response.body.len()))]
                        .to_vec(),
                },
                *delay,
                ready.clone(),
            )),
            None => Err(anyhow!(
                "no response arranged for {} {}",
                request.method,
                request.url
            )),
        };
        let timeout = request.timeout;
        fetches.sent.push(request);
        Box::pin(async move {
            let (response, delay, ready) = answer?;
            let waiting = async move {
                if let Some(ready) = ready {
                    ready.notified().await;
                }
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                response
            };
            match timeout {
                Some(timeout) => Ok(tokio::time::timeout(timeout, waiting).await?),
                None => Ok(waiting.await),
            }
        })
    }
}
