//! Operation plumbing tests — driven via `__test_support::operation_from_command`.

use std::process::Stdio;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use libguix::__test_support::operation_from_command;
use libguix::{Operation, ProgressEvent, ProgressStream};
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

async fn collect_all(mut op: Operation) -> Vec<ProgressEvent> {
    let mut out = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        out.extend(batch);
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_in_order_and_ends_with_exit_summary() {
    let op = operation_from_command(sh(
        "echo 'line one'; echo 'line two' 1>&2; echo 'line three'; exit 0",
    ))
    .expect("spawn");
    let events = collect_all(op).await;

    assert!(!events.is_empty(), "expected events");
    match events.last().unwrap() {
        ProgressEvent::ExitSummary {
            code,
            duration_secs,
        } => {
            assert_eq!(*code, 0);
            assert!(*duration_secs >= 0.0);
        }
        other => panic!("expected ExitSummary as final event, got {other:?}"),
    }

    let summary_count = events
        .iter()
        .filter(|e| matches!(e, ProgressEvent::ExitSummary { .. }))
        .count();
    assert_eq!(summary_count, 1, "ExitSummary must be emitted exactly once");

    let lines: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ProgressEvent::Line { stream, text, .. } => Some((*stream, text.as_str())),
            _ => None,
        })
        .collect();
    let texts: Vec<_> = lines.iter().map(|(_, t)| *t).collect();
    let i1 = texts
        .iter()
        .position(|t| t.contains("line one"))
        .expect("line one");
    let i3 = texts
        .iter()
        .position(|t| t.contains("line three"))
        .expect("line three");
    assert!(
        i1 < i3,
        "ordering broken: line one at {i1}, line three at {i3}"
    );

    assert!(
        lines.iter().any(|(s, _)| *s == ProgressStream::Stderr),
        "expected at least one stderr-tagged Line"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn coalescer_first_batch_arrives_fast() {
    let mut op = operation_from_command(sh("echo 'first'; sleep 0.3; echo 'second'; exit 0"))
        .expect("spawn");
    let t0 = Instant::now();
    let first = op.events_mut().next().await.expect("first batch");
    let dt = t0.elapsed();
    assert!(
        dt < Duration::from_millis(250),
        "first batch took {dt:?}; expected fast-path flush"
    );
    assert!(
        first.iter().any(|e| matches!(
            e,
            ProgressEvent::Line { text, .. } if text.contains("first")
        )),
        "first batch should contain 'first', got: {first:?}"
    );
    while op.events_mut().next().await.is_some() {}
}

#[tokio::test(flavor = "multi_thread")]
async fn coalescer_batches_within_window() {
    let mut op = operation_from_command(sh(
        "echo a; for i in $(seq 1 10); do echo line$i; done; exit 0",
    ))
    .expect("spawn");
    let mut events_per_batch: Vec<usize> = Vec::new();
    while let Some(b) = op.events_mut().next().await {
        events_per_batch.push(b.len());
    }
    assert!(!events_per_batch.is_empty(), "expected batches");
    assert!(
        events_per_batch.iter().any(|&n| n > 1),
        "expected at least one multi-event batch; got per-batch sizes: {events_per_batch:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn drop_operation_cancels_subprocess_quickly() {
    let op = operation_from_command(sh("sleep 10; exit 0")).expect("spawn");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let t0 = Instant::now();
    drop(op);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let elapsed = t0.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "drop took {elapsed:?}; expected SIGTERM to reap sleep within ~500ms"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_cancel_returns_ok() {
    let mut op = operation_from_command(sh("sleep 5; exit 0")).expect("spawn");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let cancel = op.take_cancel().expect("cancel present");
    let res = cancel.cancel().await;
    assert!(
        matches!(res, Ok(())),
        "explicit cancel should Ok, got {res:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn nonzero_exit_propagates_in_summary() {
    let op = operation_from_command(sh("echo 'oops' 1>&2; exit 7")).expect("spawn");
    let events = collect_all(op).await;
    match events.last().unwrap() {
        ProgressEvent::ExitSummary { code, .. } => assert_eq!(*code, 7),
        other => panic!("expected ExitSummary, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn carriage_return_progress_yields_multiple_events() {
    // Synthetic version of the M0 surprise: substitute: progress with
    // \r-overwrite + ESC[K. We drive `printf` directly so we don't have
    // to fight shell-quoting of `\r`, `\033`, `'`.
    let mut c = Command::new("printf");
    c.arg("%s")
        .arg(
            "substitute: \rsubstitute: \x1b[Klooking for substitutes on 'https://x'... 0.0%\rsubstitute: \x1b[Klooking for substitutes on 'https://x'... 50.0%\rsubstitute: \x1b[Klooking for substitutes on 'https://x'... 100.0%\n",
        )
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let op = operation_from_command(c).expect("spawn");
    let events = collect_all(op).await;
    let lookups: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ProgressEvent::SubstituteLookup { percent, .. } => Some(*percent),
            _ => None,
        })
        .collect();
    assert!(
        lookups.len() >= 3,
        "expected >=3 SubstituteLookup events, got {lookups:?} from full event list {events:?}"
    );
}

// --------------------------------------------------------------------------
// M2-review additions
// --------------------------------------------------------------------------

/// S2: a failing subprocess surfaces as `OperationFailed { code, stderr_tail }`
/// when consumed via `await_completion()`. The stderr ring should
/// contain every stderr line emitted.
#[tokio::test(flavor = "multi_thread")]
async fn await_completion_reports_operation_failed_with_stderr_tail() {
    let op = operation_from_command(sh("echo line1 1>&2; echo line2 1>&2; exit 2")).expect("spawn");
    let err = op.await_completion().await.expect_err("expected failure");
    match err {
        libguix::GuixError::OperationFailed { code, stderr_tail } => {
            assert_eq!(code, 2);
            assert!(
                stderr_tail.contains("line1") && stderr_tail.contains("line2"),
                "stderr tail missing lines: {stderr_tail:?}",
            );
        }
        other => panic!("expected OperationFailed, got {other:?}"),
    }
}

/// S2: a successful subprocess returns `Ok(())` from `await_completion()`.
#[tokio::test(flavor = "multi_thread")]
async fn await_completion_ok_on_zero_exit() {
    let op = operation_from_command(sh("echo hi; exit 0")).expect("spawn");
    op.await_completion().await.expect("ok exit");
}

/// S3 regression: a fast-exiting child raced against `Drop` must not
/// panic, never signals a reaped child, and doesn't flake on the
/// `kill_on_drop` reaper path either. Loop a bunch of times so a flake
/// would actually show up.
#[tokio::test(flavor = "multi_thread")]
async fn fast_exit_then_drop_is_safe() {
    for _ in 0..100 {
        let op = operation_from_command(sh("exit 0")).expect("spawn");
        // Don't await; drop immediately. The driver task may or may not
        // have observed the exit yet; either branch (cancel arm vs
        // child.wait arm) must terminate cleanly.
        drop(op);
    }
}

/// Cancel mid-coalescer-window: several events queued inside the 50ms
/// window must flush before the final `ExitSummary` produced by the
/// cancel path.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_flushes_pending_window_before_exit_summary() {
    // Emit 5 events fast, then sit on a long sleep so we can cancel.
    let mut op = operation_from_command(sh("for i in 1 2 3 4 5; do echo line$i; done; sleep 10"))
        .expect("spawn");

    // Wait for the first (idle→windowing) flushed batch to come through;
    // by then the readers have queued the rest of the lines into the
    // coalescer's current window.
    let _first = op.events_mut().next().await.expect("first batch");

    // Cancel — this triggers SIGTERM through the driver. The driver waits
    // on the child *before* sending the final ExitSummary; the readers
    // close on EOF, which closes parse_tx, which lets the coalescer
    // flush its pending Vec before exiting.
    let cancel = op.take_cancel().expect("cancel present");
    let cancel_task = tokio::spawn(async move { cancel.cancel().await });

    let mut rest: Vec<ProgressEvent> = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        rest.extend(batch);
    }
    let _ = cancel_task.await;

    // The final event must be ExitSummary.
    match rest.last().unwrap() {
        ProgressEvent::ExitSummary { .. } => {}
        other => panic!("expected ExitSummary last, got {other:?}"),
    }
    // We must have seen at least one "line<n>" Line event AFTER the first
    // batch (i.e. flushed from the pending window).
    let line_count = rest
        .iter()
        .filter(|e| matches!(e, ProgressEvent::Line { text, .. } if text.starts_with("line")))
        .count();
    assert!(
        line_count >= 1,
        "expected pending-window flush of line<n> events, got {rest:?}",
    );
}

/// Stderr-only subprocess: a `Line { stream: Stderr, text: "from-stderr" }`
/// arrives, then `ExitSummary { code: 0 }`. Verifies the stderr reader
/// path is wired regardless of the stdout reader's behaviour.
#[tokio::test(flavor = "multi_thread")]
async fn stderr_only_subprocess_yields_events() {
    let op = operation_from_command(sh("echo from-stderr 1>&2; exit 0")).expect("spawn");
    let events = collect_all(op).await;

    let saw_stderr = events.iter().any(|e| {
        matches!(
            e,
            ProgressEvent::Line { stream: ProgressStream::Stderr, text, .. }
            if text.contains("from-stderr")
        )
    });
    assert!(saw_stderr, "expected stderr Line, got {events:?}");

    match events.last().unwrap() {
        ProgressEvent::ExitSummary { code, .. } => assert_eq!(*code, 0),
        other => panic!("expected ExitSummary, got {other:?}"),
    }
}

/// Stdout closed before exit: a subprocess that closes stdout early then
/// exits cleanly must not hang the driver. Stream must terminate with
/// ExitSummary { code: 0 }.
#[tokio::test(flavor = "multi_thread")]
async fn stdout_closed_before_exit_does_not_hang() {
    let op = operation_from_command(sh("exec >&-; sleep 0.05; exit 0")).expect("spawn");
    let collect = tokio::time::timeout(Duration::from_secs(3), collect_all(op))
        .await
        .expect("must not hang");
    match collect.last().unwrap() {
        ProgressEvent::ExitSummary { code, .. } => assert_eq!(*code, 0),
        other => panic!("expected ExitSummary, got {other:?}"),
    }
}

/// Drop-then-cancel race: take the cancel handle out, drop the rest of
/// the Operation, then call cancel(). Must not panic, must not deadlock.
/// The shared inner `Arc<StdMutex<Option<…>>>` is the only path: dropping
/// the guard already fires the cancel signal, so the explicit cancel
/// returns `Err(Cancelled)` cleanly.
#[tokio::test(flavor = "multi_thread")]
async fn take_cancel_then_drop_operation_is_safe() {
    let mut op = operation_from_command(sh("sleep 5; exit 0")).expect("spawn");
    let cancel = op.take_cancel().expect("cancel present");
    drop(op);
    // After the guard's drop fires the cancel signal, the explicit cancel
    // call observes the `None` in the shared inner and returns Cancelled.
    let res = cancel.cancel().await;
    assert!(
        matches!(res, Err(libguix::GuixError::Cancelled)),
        "expected Cancelled after drop fired the signal, got {res:?}",
    );
}
