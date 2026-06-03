//! Opt-in retry for transient substitute/network failures.
//!
//! Streaming retry (re-running while forwarding live `ProgressEvent`s to a
//! UI) stays caller-side: an `Operation`'s event stream is consumed once, so
//! a generic helper can't replay it. `run_with_retry` covers the headless
//! case — await each attempt to completion, retry transient failures.

use std::time::Duration;

use crate::error::GuixError;
use crate::operation::Operation;

/// `Default` is `none()` (empty backoffs = no retries).
#[derive(Debug, Clone, Default)]
pub struct RetryPolicy {
    /// `backoffs[i]` is the delay before retry `i+1`. Empty = no retries.
    pub backoffs: Vec<Duration>,
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self {
            backoffs: Vec::new(),
        }
    }

    /// Matches the installer's 0s/60s/300s pattern (3 retries, 4 attempts).
    pub fn installer_default() -> Self {
        Self {
            backoffs: vec![
                Duration::ZERO,
                Duration::from_secs(60),
                Duration::from_secs(300),
            ],
        }
    }

    pub fn from_secs(secs: &[u64]) -> Self {
        Self {
            backoffs: secs.iter().copied().map(Duration::from_secs).collect(),
        }
    }
}

/// Await each attempt to completion, retrying transient failures per `policy`.
/// `factory` is called once per attempt, so it must spawn a fresh `Operation`.
pub async fn run_with_retry<F, Fut>(policy: &RetryPolicy, mut factory: F) -> Result<(), GuixError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Operation, GuixError>>,
{
    let mut attempt = 0usize;
    loop {
        let result = match factory().await {
            Ok(op) => op.await_completion().await,
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt < policy.backoffs.len() && e.is_transient() {
                    let delay = policy.backoffs[attempt];
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    attempt += 1;
                    continue;
                }
                return Err(e);
            }
        }
    }
}
