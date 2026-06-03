//! `run_with_retry` — transient retries, non-transient pass-through, and
//! single-attempt `none()` policy. Sleeps are virtual via paused time.

use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use libguix::__test_support::operation_from_command;
use libguix::{run_with_retry, RetryPolicy};
use tokio::process::Command;

fn sh(script: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(script)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

const TRANSIENT: &str = "echo 'error: connection timed out' 1>&2; exit 1";
const NON_TRANSIENT: &str = "echo 'error: build failed' 1>&2; exit 1";
const OK: &str = "exit 0";

#[tokio::test(start_paused = true)]
async fn retries_transient_then_succeeds() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_f = Arc::clone(&calls);
    let policy = RetryPolicy::from_secs(&[0, 60, 300]);

    let result = run_with_retry(&policy, move || {
        let n = calls_f.fetch_add(1, Ordering::SeqCst);
        let script = if n < 2 { TRANSIENT } else { OK };
        async move { operation_from_command(sh(script)) }
    })
    .await;

    result.expect("should eventually succeed");
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn non_transient_is_called_once_and_propagates() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_f = Arc::clone(&calls);
    let policy = RetryPolicy::installer_default();

    let result = run_with_retry(&policy, move || {
        calls_f.fetch_add(1, Ordering::SeqCst);
        async move { operation_from_command(sh(NON_TRANSIENT)) }
    })
    .await;

    assert!(result.is_err(), "non-transient error must propagate");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn none_policy_makes_one_attempt() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_f = Arc::clone(&calls);
    let policy = RetryPolicy::none();

    let result = run_with_retry(&policy, move || {
        calls_f.fetch_add(1, Ordering::SeqCst);
        async move { operation_from_command(sh(TRANSIENT)) }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
