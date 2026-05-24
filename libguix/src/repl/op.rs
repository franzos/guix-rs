//! REPL-native progress driver. Child execs `guix repl -t machine`,
//! parent dup2's a pipe onto fd 3, Scheme payload writes structured
//! event s-expressions there. See NOTES.md.

use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::error::GuixError;
use crate::operation::{
    assemble_operation_from_event_rx, new_stderr_ring, ExitClassifier, Operation, StderrRing,
};
use crate::types::{KnownBug, ProgressEvent, ProgressStream};

pub(crate) fn spawn_repl_op(binary: &Path, scheme_payload: &str) -> Result<Operation, GuixError> {
    let (read_owned, write_owned) = make_pipe()?;

    let mut cmd = Command::new(binary);
    cmd.arg("repl")
        .arg("-t")
        .arg("machine")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let write_fd_raw = write_owned.as_raw_fd();
    unsafe {
        cmd.as_std_mut().pre_exec(move || {
            if libc::dup2(write_fd_raw, 3) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let started = Instant::now();
    let mut child: Child = cmd.spawn().map_err(GuixError::Spawn)?;

    // Parent drops its copy — without this, the fd-3 reader never sees
    // EOF on child exit. RAII: dropping `write_owned` closes it.
    drop(write_owned);

    let events_read =
        tokio::net::unix::pipe::Receiver::from_owned_fd(read_owned).map_err(GuixError::Spawn)?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| GuixError::Spawn(io::Error::other("no repl stdin")))?;
    let payload = scheme_payload.to_owned();
    tokio::spawn(async move {
        let _ = stdin.write_all(payload.as_bytes()).await;
        let _ = stdin.write_all(b"\n").await;
        let _ = stdin.flush().await;
    });

    let (event_tx, event_rx) = mpsc::channel::<ProgressEvent>(1024);

    let stderr_ring = new_stderr_ring();
    let known_bugs = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));

    spawn_event_reader(events_read, event_tx.clone());

    if let Some(stderr) = child.stderr.take() {
        spawn_stderr_reader(stderr, event_tx.clone(), stderr_ring.clone());
    }

    // Drain stdout so the pipe doesn't fill — we get events from fd 3 only.
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let mut r = BufReader::new(stdout);
            let mut buf = String::new();
            while let Ok(n) = r.read_line(&mut buf).await {
                if n == 0 {
                    break;
                }
                buf.clear();
            }
        });
    }

    drop(event_tx);

    Ok(assemble_operation_from_event_rx(
        child,
        event_rx,
        stderr_ring,
        known_bugs,
        ExitClassifier::Standard,
        started,
    ))
}

pub(crate) const PULL_SCHEME: &str = r#"
(begin
  (use-modules (guix status)
               (guix scripts pull)
               (ice-9 ports))
  (let ((event-port (fdopen 3 "w")))
    (setvbuf event-port 'line)
    (module-set! (resolve-module '(guix status))
                 'call-with-status-verbosity
                 (lambda (level thunk) (thunk)))
    (with-exception-handler
      (lambda (exn)
        (write (list 'error (object->string exn)) event-port)
        (newline event-port)
        (force-output event-port))
      (lambda ()
        (with-status-report
          (lambda (event _old _new)
            (write event event-port)
            (newline event-port)
            (force-output event-port))
          (guix-pull))
        (write '(done 0) event-port)
        (newline event-port)
        (force-output event-port))
      #:unwind? #t)))
"#;

/// Reject newlines/null bytes — would smuggle a second form past the REPL parser.
pub(crate) fn validate_arg(arg: &str) -> Result<(), GuixError> {
    if arg.is_empty() {
        return Err(GuixError::repl("invalid package name: empty"));
    }
    if let Some(bad) = arg.chars().find(|c| matches!(c, '\n' | '\r' | '\0')) {
        return Err(GuixError::repl(format!(
            "invalid package name: contains control char {:?}",
            bad
        )));
    }
    Ok(())
}

pub(crate) fn build_package_payload(
    profile: Option<&Path>,
    argv: &[&str],
) -> Result<String, GuixError> {
    let profile_str = match profile {
        Some(p) => {
            let s = p
                .to_str()
                .ok_or_else(|| GuixError::repl("profile path is not valid UTF-8"))?;
            validate_arg(s)?;
            Some(s)
        }
        None => None,
    };
    for a in argv {
        validate_arg(a)?;
    }
    let argv_scheme = profile_str
        .into_iter()
        .flat_map(|p| ["-p", p])
        .chain(argv.iter().copied())
        .map(scheme_string_literal)
        .collect::<Vec<_>>()
        .join(" ");
    Ok(format!(
        r#"
(begin
  (use-modules (guix status)
               (guix scripts package)
               (ice-9 ports))
  (let ((event-port (fdopen 3 "w")))
    (setvbuf event-port 'line)
    (module-set! (resolve-module '(guix status))
                 'call-with-status-verbosity
                 (lambda (level thunk) (thunk)))
    (with-exception-handler
      (lambda (exn)
        (write (list 'error (object->string exn)) event-port)
        (newline event-port)
        (force-output event-port))
      (lambda ()
        (with-status-report
          (lambda (event _old _new)
            (write event event-port)
            (newline event-port)
            (force-output event-port))
          ((@ (guix scripts package) guix-package) {argv}))
        (write '(done 0) event-port)
        (newline event-port)
        (force-output event-port))
      #:unwind? #t)))
"#,
        argv = argv_scheme,
    ))
}

fn scheme_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Read end keeps `FD_CLOEXEC`; write end clears it so the dup2-target survives exec.
fn make_pipe() -> Result<(OwnedFd, OwnedFd), GuixError> {
    use nix::fcntl::{fcntl, FcntlArg, FdFlag, OFlag};
    let (read_owned, write_owned) = nix::unistd::pipe2(OFlag::O_CLOEXEC)
        .map_err(|e| GuixError::Spawn(io::Error::from_raw_os_error(e as i32)))?;
    let cur = fcntl(&write_owned, FcntlArg::F_GETFD)
        .map_err(|e| GuixError::Spawn(io::Error::from_raw_os_error(e as i32)))?;
    let mut flags = FdFlag::from_bits_truncate(cur);
    flags.remove(FdFlag::FD_CLOEXEC);
    fcntl(&write_owned, FcntlArg::F_SETFD(flags))
        .map_err(|e| GuixError::Spawn(io::Error::from_raw_os_error(e as i32)))?;
    Ok((read_owned, write_owned))
}

fn spawn_event_reader(read: tokio::net::unix::pipe::Receiver, tx: mpsc::Sender<ProgressEvent>) {
    tokio::spawn(async move {
        let mut r = BufReader::new(read);
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            let Some(evt) = parse_event_sexp(trimmed) else {
                continue;
            };
            if tx.send(evt).await.is_err() {
                return;
            }
        }
    });
}

fn spawn_stderr_reader(
    stderr: tokio::process::ChildStderr,
    tx: mpsc::Sender<ProgressEvent>,
    ring: Arc<std::sync::Mutex<StderrRing>>,
) {
    tokio::spawn(async move {
        let mut r = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match r.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Ok(mut g) = ring.lock() {
                g.push(line.as_bytes());
            }
            let evt = ProgressEvent::Line {
                stream: ProgressStream::Stderr,
                text: line.trim_end_matches('\n').to_owned(),
                redraw: false,
            };
            if tx.send(evt).await.is_err() {
                return;
            }
        }
    });
}

/// Tag dispatch — see `guix/status.scm:222 compute-status`. Unknown tags
/// fall through as `[repl-op]`-prefixed `Line`s (gated by GUI debug flag).
pub(crate) fn parse_event_sexp(line: &str) -> Option<ProgressEvent> {
    let parsed = match lexpr::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            return Some(ProgressEvent::Line {
                stream: ProgressStream::Stderr,
                text: format!("[repl-op] unparsed: {line}"),
                redraw: false,
            });
        }
    };

    let Some(mut it) = parsed.list_iter() else {
        return Some(fallthrough(line));
    };
    let Some(head) = it.next().and_then(lexpr::Value::as_symbol) else {
        return Some(fallthrough(line));
    };

    match head {
        "build-started" => {
            if let Some(drv) = it.next().and_then(lexpr::Value::as_str) {
                return Some(ProgressEvent::BuildStart {
                    drv: drv.to_owned(),
                });
            }
        }
        "build-succeeded" => {
            if let Some(drv) = it.next().and_then(lexpr::Value::as_str) {
                return Some(ProgressEvent::BuildDone {
                    drv: drv.to_owned(),
                });
            }
        }
        "build-failed" => {
            if let Some(drv) = it.next().and_then(lexpr::Value::as_str) {
                return Some(ProgressEvent::BuildFailed {
                    drv: drv.to_owned(),
                    log_path: None,
                });
            }
        }
        "build-log" => {
            // Strip trailing `\n` — terminal buffer adds its own `\r\n`.
            let _pid = it.next();
            if let Some(text) = it.next().and_then(lexpr::Value::as_str) {
                let stripped = text.strip_suffix('\n').unwrap_or(text);
                return Some(ProgressEvent::Line {
                    stream: ProgressStream::Stderr,
                    text: stripped.to_owned(),
                    redraw: false,
                });
            }
        }
        "download-started" => {
            if let Some(item) = it.next().and_then(lexpr::Value::as_str) {
                let total = it
                    .nth(1) // skip uri
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok());
                return Some(ProgressEvent::SubstituteDownload {
                    item: item.to_owned(),
                    bytes_done: 0,
                    bytes_total: total,
                });
            }
        }
        "download-progress" => {
            if let Some(item) = it.next().and_then(lexpr::Value::as_str) {
                let _uri = it.next();
                let total = it
                    .next()
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok());
                let done = it
                    .next()
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                return Some(ProgressEvent::SubstituteDownload {
                    item: item.to_owned(),
                    bytes_done: done,
                    bytes_total: total,
                });
            }
        }
        "download-succeeded" => {
            // <bytes> may be int or string depending on guix version.
            if let Some(item) = it.next().and_then(lexpr::Value::as_str) {
                let _uri = it.next();
                let bytes_total = it.next().and_then(|v| {
                    v.as_u64()
                        .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                });
                return Some(ProgressEvent::SubstituteDownloadDone {
                    item: item.to_owned(),
                    bytes_total,
                });
            }
        }
        "substituter-started" | "substituter-succeeded" | "done" => return None,
        "error" => {
            let msg = it
                .next()
                .and_then(lexpr::Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Some(ProgressEvent::Line {
                stream: ProgressStream::Stderr,
                text: format!("error: {msg}"),
                redraw: false,
            });
        }
        _ => {}
    }
    if crate::parsers::progress::is_channel_shadow_line(line) {
        return Some(ProgressEvent::KnownBug(KnownBug::ChannelShadow74396));
    }
    Some(fallthrough(line))
}

fn fallthrough(line: &str) -> ProgressEvent {
    ProgressEvent::Line {
        stream: ProgressStream::Stderr,
        text: format!("[repl-op] {line}"),
        redraw: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_build_started() {
        let e =
            parse_event_sexp(r#"(build-started "/gnu/store/abc-foo.drv" "-" "x86_64-linux" "")"#);
        match e {
            Some(ProgressEvent::BuildStart { drv }) => {
                assert_eq!(drv, "/gnu/store/abc-foo.drv");
            }
            other => panic!("expected BuildStart, got {other:?}"),
        }
    }

    #[test]
    fn parses_build_succeeded() {
        let e = parse_event_sexp(r#"(build-succeeded "/gnu/store/abc-foo.drv")"#);
        assert!(matches!(e, Some(ProgressEvent::BuildDone { .. })));
    }

    #[test]
    fn parses_build_failed() {
        let e = parse_event_sexp(r#"(build-failed "/gnu/store/abc-foo.drv" "1")"#);
        match e {
            Some(ProgressEvent::BuildFailed { drv, log_path }) => {
                assert!(drv.ends_with("foo.drv"));
                assert!(log_path.is_none());
            }
            other => panic!("expected BuildFailed, got {other:?}"),
        }
    }

    #[test]
    fn parses_download_started() {
        let e = parse_event_sexp(r#"(download-started "/gnu/store/xxx" "https://ci/x" "12345")"#);
        match e {
            Some(ProgressEvent::SubstituteDownload {
                item,
                bytes_done,
                bytes_total,
            }) => {
                assert_eq!(item, "/gnu/store/xxx");
                assert_eq!(bytes_done, 0);
                assert_eq!(bytes_total, Some(12345));
            }
            other => panic!("expected SubstituteDownload, got {other:?}"),
        }
    }

    #[test]
    fn parses_download_progress() {
        let e = parse_event_sexp(
            r#"(download-progress "/gnu/store/xxx" "https://ci/x" "12345" "6789")"#,
        );
        match e {
            Some(ProgressEvent::SubstituteDownload {
                item,
                bytes_done,
                bytes_total,
            }) => {
                assert_eq!(item, "/gnu/store/xxx");
                assert_eq!(bytes_done, 6789);
                assert_eq!(bytes_total, Some(12345));
            }
            other => panic!("expected SubstituteDownload, got {other:?}"),
        }
    }

    /// Strips trailing `\n` — terminal buffer adds `\r\n`.
    #[test]
    fn parses_build_log_unwraps_text() {
        let e = parse_event_sexp(r#"(build-log 4415 "warning: collision\n")"#);
        match e {
            Some(ProgressEvent::Line { text, .. }) => {
                assert_eq!(text, "warning: collision");
            }
            other => panic!("expected Line, got {other:?}"),
        }
    }

    #[test]
    fn parses_download_succeeded() {
        let e = parse_event_sexp(r#"(download-succeeded "/gnu/store/xxx" "https://ci/x" 12345)"#);
        match e {
            Some(ProgressEvent::SubstituteDownloadDone { item, bytes_total }) => {
                assert_eq!(item, "/gnu/store/xxx");
                assert_eq!(bytes_total, Some(12345));
            }
            other => panic!("expected SubstituteDownloadDone, got {other:?}"),
        }
    }

    #[test]
    fn drops_substituter_events() {
        assert!(
            parse_event_sexp(r#"(substituter-started "/gnu/store/xxx" "substitute")"#,).is_none()
        );
        assert!(parse_event_sexp(r#"(substituter-succeeded "/gnu/store/xxx")"#).is_none());
    }

    #[test]
    fn drops_done_event() {
        assert!(parse_event_sexp("(done 0)").is_none());
        assert!(parse_event_sexp("(done 1)").is_none());
    }

    #[test]
    fn parses_error_event() {
        let e = parse_event_sexp(r#"(error "oh no")"#);
        match e {
            Some(ProgressEvent::Line { text, .. }) => {
                assert!(text.contains("oh no"), "got text: {text}");
                assert!(text.starts_with("error:"));
            }
            other => panic!("expected Line, got {other:?}"),
        }
    }

    #[test]
    fn unknown_event_falls_through_to_line() {
        let e = parse_event_sexp(r#"(some-unknown-tag "/gnu/store/xxx")"#);
        match e {
            Some(ProgressEvent::Line { text, .. }) => {
                assert!(text.contains("some-unknown-tag"));
                assert!(text.starts_with("[repl-op]"));
            }
            other => panic!("expected Line passthrough, got {other:?}"),
        }
    }

    #[test]
    fn malformed_line_falls_through() {
        let e = parse_event_sexp("not actually s-exp )))");
        assert!(matches!(e, Some(ProgressEvent::Line { .. })));
    }

    #[test]
    fn validate_arg_rejects_control_chars() {
        assert!(validate_arg("foo\nbar").is_err());
        assert!(validate_arg("foo\rbar").is_err());
        assert!(validate_arg("foo\0bar").is_err());
        assert!(validate_arg("").is_err());
    }

    #[test]
    fn validate_arg_accepts_normal_names() {
        assert!(validate_arg("hello").is_ok());
        assert!(validate_arg("rust-").is_ok());
        assert!(validate_arg("gcc-toolchain@13").is_ok());
        assert!(validate_arg("a\"b").is_ok());
        assert!(validate_arg("a\\b").is_ok());
    }

    #[test]
    fn scheme_string_literal_escapes() {
        assert_eq!(scheme_string_literal("hello"), "\"hello\"");
        assert_eq!(scheme_string_literal("a\"b"), "\"a\\\"b\"");
        assert_eq!(scheme_string_literal("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn build_package_payload_install_shape() {
        let p = build_package_payload(None, &["-i", "hello"]).expect("ok");
        assert!(p.contains("guix-package"));
        assert!(p.contains("\"-i\""));
        assert!(p.contains("\"hello\""));
        assert!(p.contains("call-with-status-verbosity"));
        assert!(!p.contains("\"-p\""));
    }

    #[test]
    fn build_package_payload_rejects_bad_arg() {
        assert!(build_package_payload(None, &["-i", "foo\nbar"]).is_err());
    }

    #[test]
    fn build_package_payload_threads_profile() {
        let p = build_package_payload(Some(Path::new("/tmp/p")), &["-i", "hello"]).expect("ok");
        let p_idx = p.find("\"-p\"").expect("-p in payload");
        let path_idx = p.find("\"/tmp/p\"").expect("profile path in payload");
        let i_idx = p.find("\"-i\"").expect("-i in payload");
        let hello_idx = p.find("\"hello\"").expect("hello in payload");
        assert!(p_idx < path_idx);
        assert!(path_idx < i_idx);
        assert!(i_idx < hello_idx);
    }

    #[test]
    fn build_package_payload_rejects_bad_profile_path() {
        assert!(build_package_payload(Some(Path::new("/tmp/p\nx")), &["-i", "hello"]).is_err());
        assert!(build_package_payload(Some(Path::new("/tmp/p\0x")), &["-i", "hello"]).is_err());
    }
}
