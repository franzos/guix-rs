//! Integration tests for `SystemOps::reconfigure` — pkexec exit codes
//! and channel-shadow #74396 known-bug escalation.

use std::process::Stdio;

use futures_util::StreamExt;
use libguix::__test_support::{operation_from_command, pkexec_operation_from_command};
use libguix::{GuixError, KnownBug, PolkitFailure, ProgressEvent};
use tokio::process::Command;

/// Plain `sh -c` builder with locale forced, mirroring `operation.rs` tests.
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

/// `pkexec` exit 0 → `Ok(())` even under the Pkexec classifier.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_zero_exit_is_ok() {
    let op = pkexec_operation_from_command(sh("echo hi; exit 0")).expect("spawn");
    op.await_completion().await.expect("ok exit");
}

/// `pkexec` exit 126 → `PolkitFailure::AuthFailed`, stderr_tail populated.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_126_maps_to_auth_failed() {
    let op = pkexec_operation_from_command(sh("echo 'Error: dismissed by user' 1>&2; exit 126"))
        .expect("spawn");
    let err = op
        .await_completion()
        .await
        .expect_err("expected polkit err");
    match err {
        GuixError::Polkit {
            kind: PolkitFailure::AuthFailed,
            code,
            stderr_tail,
        } => {
            assert_eq!(code, 126);
            assert!(
                stderr_tail.contains("dismissed"),
                "stderr_tail should include the failure line: {stderr_tail:?}"
            );
        }
        other => panic!("expected Polkit AuthFailed, got {other:?}"),
    }
}

/// `pkexec` exit 127 → `PolkitFailure::NotAuthorized`.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_127_maps_to_not_authorized() {
    let op = pkexec_operation_from_command(sh("echo 'Error: not authorized' 1>&2; exit 127"))
        .expect("spawn");
    let err = op
        .await_completion()
        .await
        .expect_err("expected polkit err");
    assert!(
        matches!(
            err,
            GuixError::Polkit {
                kind: PolkitFailure::NotAuthorized,
                code: 127,
                ..
            }
        ),
        "got {err:?}"
    );
}

/// `pkexec` exit 130 (128 + SIGINT=2) → `PolkitFailure::KilledBySignal(2)`.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_130_maps_to_killed_by_signal() {
    let op = pkexec_operation_from_command(sh("exit 130")).expect("spawn");
    let err = op
        .await_completion()
        .await
        .expect_err("expected polkit err");
    assert!(
        matches!(
            err,
            GuixError::Polkit {
                kind: PolkitFailure::KilledBySignal(2),
                code: 130,
                ..
            }
        ),
        "got {err:?}"
    );
}

/// TG2: `pkexec` exit 139 (128 + SIGSEGV=11) → `KilledBySignal(11)`.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_139_maps_to_segv() {
    let op = pkexec_operation_from_command(sh("exit 139")).expect("spawn");
    let err = op
        .await_completion()
        .await
        .expect_err("expected polkit err");
    assert!(
        matches!(
            err,
            GuixError::Polkit {
                kind: PolkitFailure::KilledBySignal(11),
                code: 139,
                ..
            }
        ),
        "got {err:?}"
    );
}

/// Inner-command pass-through (codes 1..=125) under pkexec stays
/// `OperationFailed`, **not** `Polkit`. Otherwise a genuine guix failure
/// under `pkexec` would be misclassified.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_classifier_pass_through_stays_operation_failed() {
    let op = pkexec_operation_from_command(sh("echo guix-fail 1>&2; exit 7")).expect("spawn");
    let err = op.await_completion().await.expect_err("expected op-failed");
    match err {
        GuixError::OperationFailed { code, stderr_tail } => {
            assert_eq!(code, 7);
            assert!(stderr_tail.contains("guix-fail"));
        }
        other => panic!("expected OperationFailed, got {other:?}"),
    }
}

/// Standard classifier never produces `Polkit`, even on 126.
#[tokio::test(flavor = "multi_thread")]
async fn standard_classifier_does_not_produce_polkit() {
    let op = operation_from_command(sh("exit 126")).expect("spawn");
    let err = op.await_completion().await.expect_err("expected op-failed");
    assert!(
        matches!(err, GuixError::OperationFailed { code: 126, .. }),
        "got {err:?}"
    );
}

/// TG1: stderr drained even when the writes come *after* every other
/// event but before the exit summary observable. Fake pkexec emits 10 KB
/// of stderr in tight bursts, then exits 126. The drain-before-classify
/// invariant in `await_completion` means `stderr_tail` must still
/// contain the late-arriving bytes — not the empty string we'd get if
/// the snapshot ran before the readers had finished consuming the pipe.
#[tokio::test(flavor = "multi_thread")]
async fn pkexec_stderr_drained_before_classify() {
    // 10 KB total. Use a fixed marker we can grep for at the tail. The
    // ring buffer holds 64 KB, so the marker should always be reachable.
    let script = r#"
i=0
while [ $i -lt 200 ]; do
  printf 'late-stderr-line-%04d-padding-padding-padding-padding\n' "$i" 1>&2
  i=$((i + 1))
done
printf 'TAIL-MARKER\n' 1>&2
exit 126
"#;
    let op = pkexec_operation_from_command(sh(script)).expect("spawn");
    let err = op
        .await_completion()
        .await
        .expect_err("expected polkit err");
    match err {
        GuixError::Polkit {
            kind: PolkitFailure::AuthFailed,
            code,
            stderr_tail,
        } => {
            assert_eq!(code, 126);
            assert!(
                stderr_tail.contains("TAIL-MARKER"),
                "drain-before-classify failed: stderr_tail missing tail marker"
            );
            // Sanity: we actually accumulated bulk bytes too.
            assert!(
                stderr_tail.len() >= 4 * 1024,
                "expected ~10 KB of stderr, got {} bytes",
                stderr_tail.len()
            );
        }
        other => panic!("expected Polkit AuthFailed, got {other:?}"),
    }
}

/// Under the standard classifier: emitting the trigger phrase on stderr +
/// a non-zero exit surfaces as `GuixError::KnownBug`, *and* a
/// `ProgressEvent::KnownBug` event appears in the live stream.
#[tokio::test(flavor = "multi_thread")]
async fn channel_shadow_streams_and_escalates_on_failure() {
    let mut op = operation_from_command(sh(
        "echo 'no code for module (some-channel mod)' 1>&2; exit 1",
    ))
    .expect("spawn");

    let mut events = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        events.extend(batch);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ProgressEvent::KnownBug(KnownBug::ChannelShadow74396))),
        "expected KnownBug event in stream; got {events:?}"
    );

    // Re-spawn and await: the bug should be observed again and escalate.
    let op2 = operation_from_command(sh(
        "echo 'no code for module (some-channel mod)' 1>&2; exit 1",
    ))
    .expect("spawn");
    let err = op2.await_completion().await.expect_err("expected error");
    assert!(
        matches!(err, GuixError::KnownBug(KnownBug::ChannelShadow74396)),
        "expected KnownBug error, got {err:?}"
    );
}

/// Same trigger line *but the operation succeeds*: the KnownBug event
/// still flows live (so a GUI can show a soft warning), but
/// `await_completion` must return `Ok(())` — we only escalate on
/// failure to avoid crying wolf.
#[tokio::test(flavor = "multi_thread")]
async fn channel_shadow_on_success_does_not_escalate() {
    let mut op = operation_from_command(sh(
        "echo 'no code for module (some-channel mod)' 1>&2; exit 0",
    ))
    .expect("spawn");

    let mut events = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        events.extend(batch);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ProgressEvent::KnownBug(KnownBug::ChannelShadow74396))),
        "expected live KnownBug event; got {events:?}"
    );

    let op2 = operation_from_command(sh(
        "echo 'no code for module (some-channel mod)' 1>&2; exit 0",
    ))
    .expect("spawn");
    op2.await_completion()
        .await
        .expect("zero exit must stay Ok despite known-bug line");
}

/// KnownBug escalation takes precedence over Polkit classification: even
/// if the operation exited 126 *and* the bug line was observed, we want
/// the bug error (it's more actionable for the user).
#[tokio::test(flavor = "multi_thread")]
async fn channel_shadow_outranks_polkit_classification() {
    let op =
        pkexec_operation_from_command(sh("echo 'no code for module (foo bar)' 1>&2; exit 126"))
            .expect("spawn");
    let err = op.await_completion().await.expect_err("expected error");
    assert!(
        matches!(err, GuixError::KnownBug(KnownBug::ChannelShadow74396)),
        "expected KnownBug to outrank Polkit, got {err:?}"
    );
}

/// Unrelated stderr lines must not trip the bug detector.
#[tokio::test(flavor = "multi_thread")]
async fn unrelated_stderr_does_not_trigger_known_bug() {
    let op = operation_from_command(sh(
        "echo 'guix: error: failed to build derivation' 1>&2; exit 1",
    ))
    .expect("spawn");
    let err = op.await_completion().await.expect_err("expected error");
    assert!(
        matches!(err, GuixError::OperationFailed { code: 1, .. }),
        "got {err:?}"
    );
}

/// TG3: `LIBGUIX_FORCE_NO_AGENT=1` exercises the auth-agent-absent
/// pre-flight branch without depending on whatever's running on the host.
/// We also clear `LIBGUIX_SKIP_AGENT_CHECK` for the test process so
/// the skip path doesn't shadow the force path.
///
/// Synchronous (not `#[tokio::test]`) because `reconfigure` only spawns
/// when the pre-flight passes — we're asserting the pre-check fires
/// before any subprocess work.
///
/// The env vars are process-global; we use a mutex to serialise this
/// test against any other env-var-touching test in the same binary.
#[test]
fn reconfigure_force_no_agent_returns_no_auth_agent() {
    use libguix::ReconfigureOptions;
    use std::path::PathBuf;
    use std::sync::Mutex;

    // Serialise env-var mutation so parallel test runners don't race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let prev_skip = std::env::var_os("LIBGUIX_SKIP_AGENT_CHECK");
    let prev_force = std::env::var_os("LIBGUIX_FORCE_NO_AGENT");
    std::env::remove_var("LIBGUIX_SKIP_AGENT_CHECK");
    std::env::set_var("LIBGUIX_FORCE_NO_AGENT", "1");

    let sys = libguix::__test_support::system_ops();
    let cfg = PathBuf::from("/tmp/libguix-fake-cfg.scm");
    let result = sys.reconfigure(&cfg, ReconfigureOptions::default());

    // Restore env before asserting so a panic doesn't leak state into
    // other tests in this binary.
    if let Some(v) = prev_skip {
        std::env::set_var("LIBGUIX_SKIP_AGENT_CHECK", v);
    }
    if let Some(v) = prev_force {
        std::env::set_var("LIBGUIX_FORCE_NO_AGENT", v);
    } else {
        std::env::remove_var("LIBGUIX_FORCE_NO_AGENT");
    }

    // `Operation` doesn't impl `Debug`, so use a manual match rather
    // than `expect_err`.
    match result {
        Ok(_) => panic!("expected NoAuthAgent pre-flight failure, got Ok"),
        Err(GuixError::Polkit {
            kind: PolkitFailure::NoAuthAgent,
            ..
        }) => {}
        Err(other) => panic!("expected NoAuthAgent, got {other:?}"),
    }
}

/// `#[ignore]`-gated: runs `SystemOps::reconfigure(..., dry_run=true)`
/// against a temporary minimal config. This triggers a real polkit
/// prompt; opt in with `cargo test --features live-tests -- --ignored`.
///
/// Doesn't mutate system state (`--dry-run`), but does evaluate guix's
/// own system config DSL, which builds derivations. Tolerates both
/// outcomes: prompt approved → success or non-polkit guix failure
/// (the minimal config we generate may not be a valid bootable system);
/// prompt denied → `PolkitFailure::AuthFailed`.
#[cfg(feature = "live-tests")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "interactive polkit prompt; opt in via --ignored"]
async fn live_reconfigure_dry_run_triggers_polkit() {
    use libguix::{Guix, ReconfigureOptions};
    use std::io::Write;

    let g = Guix::discover().await.expect("discover");
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path().join("test.scm");
    let body = r#"(use-modules (gnu))
(operating-system
  (host-name "test")
  (timezone "UTC")
  (locale "en_US.UTF-8")
  (bootloader (bootloader-configuration
                (bootloader grub-bootloader)
                (targets '("/dev/null"))))
  (file-systems %base-file-systems))
"#;
    std::fs::File::create(&cfg)
        .and_then(|mut f| f.write_all(body.as_bytes()))
        .expect("write config");

    let mut op = g
        .system()
        .reconfigure(
            &cfg,
            ReconfigureOptions {
                dry_run: true,
                ..Default::default()
            },
        )
        .expect("spawn reconfigure");

    // Drain to completion regardless of outcome. We assert that at least
    // one batch arrived, and that the final event is ExitSummary.
    let mut events = Vec::new();
    while let Some(b) = op.events_mut().next().await {
        events.extend(b);
    }
    assert!(!events.is_empty(), "expected at least one event");
    assert!(
        matches!(events.last(), Some(ProgressEvent::ExitSummary { .. })),
        "expected ExitSummary as final event, got {events:?}"
    );
}

/// `#[ignore]`-gated: runs `SystemOps::pull(dry_run=true)` against the
/// real pkexec / polkit stack. Mirrors the reconfigure live test —
/// tolerates the prompt being approved (run completes) or dismissed
/// (`PolkitFailure::AuthFailed`).
///
/// `guix pull --dry-run` doesn't mutate state and skips the
/// derivation build, so this is the cheapest end-to-end live
/// exercise of the new `system().pull()` plumbing.
#[cfg(feature = "live-tests")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "interactive polkit prompt; opt in via --ignored"]
async fn live_system_pull_dry_run_triggers_polkit() {
    use libguix::{Guix, SystemPullOptions};

    let g = Guix::discover().await.expect("discover");
    let mut op = g
        .system()
        .pull(SystemPullOptions { dry_run: true })
        .expect("spawn system pull");

    let mut events = Vec::new();
    while let Some(b) = op.events_mut().next().await {
        events.extend(b);
    }
    assert!(!events.is_empty(), "expected at least one event");
    assert!(
        matches!(events.last(), Some(ProgressEvent::ExitSummary { .. })),
        "expected ExitSummary as final event, got {events:?}"
    );
}

/// `LIBGUIX_FORCE_NO_AGENT=1` exercises the auth-agent-absent
/// pre-flight branch for `system().pull()` too — the agent check is
/// shared with reconfigure, but we assert it fires before any
/// subprocess work for pull as well.
#[test]
fn system_pull_force_no_agent_returns_no_auth_agent() {
    use libguix::SystemPullOptions;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let prev_skip = std::env::var_os("LIBGUIX_SKIP_AGENT_CHECK");
    let prev_force = std::env::var_os("LIBGUIX_FORCE_NO_AGENT");
    std::env::remove_var("LIBGUIX_SKIP_AGENT_CHECK");
    std::env::set_var("LIBGUIX_FORCE_NO_AGENT", "1");

    let _sys = libguix::__test_support::system_ops();
    let pull = libguix::__test_support::pull_ops_with_fake_binary();
    let result = pull.as_root(SystemPullOptions::default());

    if let Some(v) = prev_skip {
        std::env::set_var("LIBGUIX_SKIP_AGENT_CHECK", v);
    }
    if let Some(v) = prev_force {
        std::env::set_var("LIBGUIX_FORCE_NO_AGENT", v);
    } else {
        std::env::remove_var("LIBGUIX_FORCE_NO_AGENT");
    }

    match result {
        Ok(_) => panic!("expected NoAuthAgent pre-flight failure, got Ok"),
        Err(GuixError::Polkit {
            kind: PolkitFailure::NoAuthAgent,
            ..
        }) => {}
        Err(other) => panic!("expected NoAuthAgent, got {other:?}"),
    }
}
