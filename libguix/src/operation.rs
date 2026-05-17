//! Streamed write operations.
//!
//! Coalescer states: **idle** flushes the first event of a quiescent
//! period immediately so the UI feels responsive; **windowing**
//! accumulates for [`COALESCE_WINDOW`] or [`COALESCE_MAX_EVENTS`].
//! Final event of every stream is [`ProgressEvent::ExitSummary`].

use std::collections::HashSet;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use futures_core::Stream;
use futures_util::StreamExt;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::{GuixError, PolkitFailure};
use crate::parsers::lines::Splitter;
use crate::parsers::progress::parse_line;
use crate::process::graceful_kill;
use crate::types::{KnownBug, ProgressEvent, ProgressStream};

pub(crate) const COALESCE_WINDOW: Duration = Duration::from_millis(50);
pub(crate) const COALESCE_MAX_EVENTS: usize = 32;
const STDERR_RING_BYTES: usize = 64 * 1024;

pub type EventStream = Pin<Box<dyn Stream<Item = Vec<ProgressEvent>> + Send + 'static>>;

/// `Pkexec` upgrades 126/127/128+N to [`GuixError::Polkit`]; codes 1-125
/// still surface as [`GuixError::OperationFailed`] so genuine guix
/// failures under pkexec aren't masked.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ExitClassifier {
    Standard,
    Pkexec,
}

/// Hold this to read events; drop to cancel (SIGTERM → 5s → SIGKILL).
pub struct Operation {
    pub(crate) events: EventStream,
    pub(crate) cancel: Option<CancelHandle>,
    pub(crate) guard: DropGuard,
    classifier: ExitClassifier,
    known_bugs: Arc<StdMutex<HashSet<KnownBug>>>,
}

impl Operation {
    pub fn events_mut(&mut self) -> &mut EventStream {
        &mut self.events
    }

    /// Dropping the `Operation` after taking still fires cancel via [`DropGuard`].
    pub fn take_cancel(&mut self) -> Option<CancelHandle> {
        self.cancel.take()
    }

    /// Error precedence on non-zero exit: `KnownBug` > `Polkit` (pkexec
    /// reserved codes only) > `OperationFailed`. Stream ending without
    /// an `ExitSummary` returns `Cancelled`.
    pub async fn await_completion(mut self) -> Result<(), GuixError> {
        let mut last_exit: Option<i32> = None;
        while let Some(batch) = self.events.next().await {
            for evt in batch {
                if let ProgressEvent::ExitSummary { code, .. } = evt {
                    last_exit = Some(code);
                }
            }
        }
        match last_exit {
            Some(0) => Ok(()),
            Some(code) => {
                let stderr_tail = self.guard.stderr_snapshot();
                if let Some(bug) = self.first_known_bug() {
                    return Err(GuixError::KnownBug(bug));
                }
                if let ExitClassifier::Pkexec = self.classifier {
                    if let Some(kind) = classify_pkexec_code(code) {
                        return Err(GuixError::Polkit {
                            kind,
                            code,
                            stderr_tail,
                        });
                    }
                }
                Err(GuixError::OperationFailed { code, stderr_tail })
            }
            None => Err(GuixError::Cancelled),
        }
    }

    fn first_known_bug(&self) -> Option<KnownBug> {
        let set = self.known_bugs.lock().ok()?;
        set.iter().copied().next()
    }
}

/// Per `man pkexec`: 126 = auth failed, 127 = not authorised, 128+N = signalled.
/// 128 itself stays unclassified; 1-125 are guix's own codes.
fn classify_pkexec_code(code: i32) -> Option<PolkitFailure> {
    match code {
        126 => Some(PolkitFailure::AuthFailed),
        127 => Some(PolkitFailure::NotAuthorized),
        c if (129..=255).contains(&c) => Some(PolkitFailure::KilledBySignal(c - 128)),
        _ => None,
    }
}

/// `cancel()` cannot stop pkexec-launched children — they run as root and
/// signals from a non-root caller `EPERM`. See NOTES.md.
pub struct CancelHandle {
    inner: Arc<StdMutex<Option<CancelInner>>>,
}

struct CancelInner {
    cancel_tx: oneshot::Sender<()>,
    driver: JoinHandle<()>,
}

/// `std::sync::Mutex` is fine — only held across `Option::take` in `Drop`,
/// no `.await`, so we don't need to spawn from `Drop` to acquire.
pub(crate) struct DropGuard {
    inner: Arc<StdMutex<Option<CancelInner>>>,
    stderr_ring: Arc<StdMutex<StderrRing>>,
}

impl DropGuard {
    fn stderr_snapshot(&self) -> String {
        let ring = self.stderr_ring.lock().expect("stderr ring poisoned");
        ring.snapshot()
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(c) = guard.take() {
                let _ = c.cancel_tx.send(());
                drop(c.driver);
            }
        }
    }
}

pub(crate) struct StderrRing {
    buf: Vec<u8>,
}

impl StderrRing {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() > STDERR_RING_BYTES {
            let drop_n = self.buf.len() - STDERR_RING_BYTES;
            self.buf.drain(..drop_n);
        }
    }

    fn snapshot(&self) -> String {
        String::from_utf8_lossy(&self.buf).into_owned()
    }
}

impl CancelHandle {
    /// Send SIGTERM (then SIGKILL after 5s if needed). Returns `Cancelled`
    /// if already gone. pkexec caveat: see type-level docs.
    pub async fn cancel(self) -> Result<(), GuixError> {
        let inner = {
            let mut guard = self.inner.lock().expect("cancel inner poisoned");
            guard.take()
        };
        let Some(c) = inner else {
            return Err(GuixError::Cancelled);
        };
        let _ = c.cancel_tx.send(());
        let _ = c.driver.await;
        Ok(())
    }
}

pub(crate) fn spawn_operation(cmd: Command) -> Result<Operation, GuixError> {
    spawn_operation_with(cmd, ExitClassifier::Standard)
}

/// REPL-native ops feed structured events through `event_rx` instead of
/// parsing stderr; the rest of the pipeline matches `spawn_operation_with`.
pub(crate) fn assemble_operation_from_event_rx(
    child: Child,
    event_rx: mpsc::Receiver<ProgressEvent>,
    stderr_ring: Arc<StdMutex<StderrRing>>,
    known_bugs: Arc<StdMutex<HashSet<KnownBug>>>,
    classifier: ExitClassifier,
    started: Instant,
) -> Operation {
    let (batch_tx, batch_rx) = mpsc::channel::<Vec<ProgressEvent>>(32);
    let coalescer = spawn_coalescer(event_rx, batch_tx.clone());

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    let driver = tokio::spawn(async move {
        drive(child, started, cancel_rx, coalescer, batch_tx).await;
    });

    let inner = Arc::new(StdMutex::new(Some(CancelInner { cancel_tx, driver })));

    let cancel = CancelHandle {
        inner: inner.clone(),
    };
    let guard = DropGuard {
        inner: inner.clone(),
        stderr_ring,
    };

    let events: EventStream = Box::pin(tokio_stream::wrappers::ReceiverStream::new(batch_rx));

    Operation {
        events,
        cancel: Some(cancel),
        guard,
        classifier,
        known_bugs,
    }
}

pub(crate) fn new_stderr_ring() -> Arc<StdMutex<StderrRing>> {
    Arc::new(StdMutex::new(StderrRing::new()))
}

pub(crate) fn spawn_operation_with(
    mut cmd: Command,
    classifier: ExitClassifier,
) -> Result<Operation, GuixError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let started = Instant::now();
    let mut child: Child = cmd.spawn().map_err(GuixError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| GuixError::Spawn(std::io::Error::other("no stdout pipe")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| GuixError::Spawn(std::io::Error::other("no stderr pipe")))?;

    let (parse_tx, parse_rx) = mpsc::channel::<ProgressEvent>(1024);
    let (batch_tx, batch_rx) = mpsc::channel::<Vec<ProgressEvent>>(32);

    let stderr_ring = Arc::new(StdMutex::new(StderrRing::new()));
    let known_bugs: Arc<StdMutex<HashSet<KnownBug>>> = Arc::new(StdMutex::new(HashSet::new()));

    spawn_reader(
        stdout,
        ProgressStream::Stdout,
        parse_tx.clone(),
        None,
        known_bugs.clone(),
    );
    spawn_reader(
        stderr,
        ProgressStream::Stderr,
        parse_tx.clone(),
        Some(stderr_ring.clone()),
        known_bugs.clone(),
    );
    drop(parse_tx);

    let coalescer = spawn_coalescer(parse_rx, batch_tx.clone());

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    let driver = tokio::spawn(async move {
        drive(child, started, cancel_rx, coalescer, batch_tx).await;
    });

    let inner = Arc::new(StdMutex::new(Some(CancelInner { cancel_tx, driver })));

    let cancel = CancelHandle {
        inner: inner.clone(),
    };
    let guard = DropGuard {
        inner: inner.clone(),
        stderr_ring,
    };

    let events: EventStream = Box::pin(tokio_stream::wrappers::ReceiverStream::new(batch_rx));

    Ok(Operation {
        events,
        cancel: Some(cancel),
        guard,
        classifier,
        known_bugs,
    })
}

/// EOF chain: `child.wait()` → reader EOF → drops last `parse_tx` →
/// coalescer's `recv()` returns `None` and drains its in-progress `Vec`
/// before exiting. Pending events reach the consumer before `ExitSummary`.
async fn drive(
    mut child: Child,
    started: Instant,
    mut cancel_rx: oneshot::Receiver<()>,
    coalescer: JoinHandle<()>,
    batch_tx: mpsc::Sender<Vec<ProgressEvent>>,
) {
    let exit_code: i32 = tokio::select! {
        wait = child.wait() => {
            match wait {
                Ok(status) => status_to_code(status),
                Err(_) => -1,
            }
        }
        _ = &mut cancel_rx => {
            graceful_kill(&mut child).await.unwrap_or(-1)
        }
    };

    let _ = coalescer.await;

    let elapsed = started.elapsed().as_secs_f64();
    let summary = ProgressEvent::ExitSummary {
        code: exit_code,
        duration_secs: elapsed,
    };
    let _ = batch_tx.send(vec![summary]).await;
}

fn status_to_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    -1
}

fn spawn_reader<R>(
    reader: R,
    stream: ProgressStream,
    tx: mpsc::Sender<ProgressEvent>,
    stderr_ring: Option<Arc<StdMutex<StderrRing>>>,
    known_bugs: Arc<StdMutex<HashSet<KnownBug>>>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut splitter = Splitter::new();
        let mut buf = [0u8; 4096];
        let mut reader = reader;
        loop {
            let n = match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if let Some(ring) = &stderr_ring {
                if let Ok(mut r) = ring.lock() {
                    r.push(&buf[..n]);
                }
            }
            let mut frames = Vec::new();
            splitter.feed(&buf[..n], &mut frames);
            for frame in frames {
                let evt = parse_line(stream, &frame.text, frame.redraw);
                if let ProgressEvent::KnownBug(bug) = evt {
                    if let Ok(mut s) = known_bugs.lock() {
                        s.insert(bug);
                    }
                }
                if tx.send(evt).await.is_err() {
                    return;
                }
            }
        }
        let mut tail = Vec::new();
        splitter.flush_eof(&mut tail);
        for frame in tail {
            let evt = parse_line(stream, &frame.text, frame.redraw);
            if let ProgressEvent::KnownBug(bug) = evt {
                if let Ok(mut s) = known_bugs.lock() {
                    s.insert(bug);
                }
            }
            if tx.send(evt).await.is_err() {
                return;
            }
        }
    });
}

fn spawn_coalescer(
    mut parse_rx: mpsc::Receiver<ProgressEvent>,
    batch_tx: mpsc::Sender<Vec<ProgressEvent>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // idle: first event flushes immediately as a single-element Vec.
            let Some(evt) = parse_rx.recv().await else {
                return;
            };
            if batch_tx.send(vec![evt]).await.is_err() {
                return;
            }

            // windowing: accumulate until COALESCE_WINDOW or COALESCE_MAX_EVENTS.
            let mut batch: Vec<ProgressEvent> = Vec::with_capacity(8);
            let deadline = tokio::time::sleep(COALESCE_WINDOW);
            tokio::pin!(deadline);
            loop {
                if batch.len() >= COALESCE_MAX_EVENTS {
                    break;
                }
                tokio::select! {
                    biased;
                    () = &mut deadline => break,
                    next = parse_rx.recv() => match next {
                        Some(e) => batch.push(e),
                        None => {
                            if !batch.is_empty() {
                                let _ = batch_tx.send(batch).await;
                            }
                            return;
                        }
                    }
                }
            }
            if !batch.is_empty() && batch_tx.send(batch).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{advance, pause};

    /// Pins idle→windowing fast-path: every event after a quiet period
    /// flushes immediately as a single-element Vec.
    #[tokio::test(start_paused = true)]
    async fn coalescer_flushes_first_event_per_idle_period() {
        let (parse_tx, parse_rx) = mpsc::channel::<ProgressEvent>(16);
        let (batch_tx, mut batch_rx) = mpsc::channel::<Vec<ProgressEvent>>(16);
        let task = spawn_coalescer(parse_rx, batch_tx);

        parse_tx
            .send(ProgressEvent::Line {
                stream: ProgressStream::Stdout,
                text: "A".into(),
                redraw: false,
            })
            .await
            .unwrap();
        let first = batch_rx.recv().await.expect("first batch");
        assert_eq!(first.len(), 1);
        match &first[0] {
            ProgressEvent::Line { text, .. } => assert_eq!(text, "A"),
            other => panic!("unexpected event: {other:?}"),
        }

        advance(Duration::from_millis(200)).await;

        parse_tx
            .send(ProgressEvent::Line {
                stream: ProgressStream::Stdout,
                text: "B".into(),
                redraw: false,
            })
            .await
            .unwrap();
        let second = batch_rx.recv().await.expect("second batch");
        assert_eq!(second.len(), 1);
        match &second[0] {
            ProgressEvent::Line { text, .. } => assert_eq!(text, "B"),
            other => panic!("unexpected event: {other:?}"),
        }

        drop(parse_tx);
        let _ = task.await;
    }

    /// Pins burst coalescing after the initial fast-path event.
    #[tokio::test(start_paused = true)]
    async fn coalescer_bursts_coalesce_after_first() {
        let (parse_tx, parse_rx) = mpsc::channel::<ProgressEvent>(16);
        let (batch_tx, mut batch_rx) = mpsc::channel::<Vec<ProgressEvent>>(16);
        let task = spawn_coalescer(parse_rx, batch_tx);

        let make = |s: &str| ProgressEvent::Line {
            stream: ProgressStream::Stdout,
            text: s.into(),
            redraw: false,
        };

        parse_tx.send(make("A")).await.unwrap();
        let a_batch = batch_rx.recv().await.expect("A batch");
        assert_eq!(a_batch.len(), 1);

        parse_tx.send(make("B")).await.unwrap();
        parse_tx.send(make("C")).await.unwrap();
        parse_tx.send(make("D")).await.unwrap();

        advance(Duration::from_millis(60)).await;
        let burst = batch_rx.recv().await.expect("burst batch");
        let texts: Vec<_> = burst
            .iter()
            .filter_map(|e| match e {
                ProgressEvent::Line { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["B", "C", "D"]);

        drop(parse_tx);
        let _ = task.await;
    }

    #[test]
    fn stderr_ring_rolls_at_cap() {
        let mut r = StderrRing::new();
        for _ in 0..2000 {
            r.push(b"0123456789ABCDEF0123456789ABCDEF\n");
        }
        let snap = r.snapshot();
        assert!(snap.len() <= STDERR_RING_BYTES);
        assert!(snap.ends_with("0123456789ABCDEF\n"));
    }

    #[test]
    fn pkexec_classifier_maps_reserved_codes() {
        assert_eq!(classify_pkexec_code(126), Some(PolkitFailure::AuthFailed));
        assert_eq!(
            classify_pkexec_code(127),
            Some(PolkitFailure::NotAuthorized)
        );
        assert_eq!(
            classify_pkexec_code(130),
            Some(PolkitFailure::KilledBySignal(2))
        );
        assert_eq!(
            classify_pkexec_code(137),
            Some(PolkitFailure::KilledBySignal(9))
        );
        assert_eq!(
            classify_pkexec_code(143),
            Some(PolkitFailure::KilledBySignal(15))
        );
        assert_eq!(classify_pkexec_code(0), None);
        assert_eq!(classify_pkexec_code(1), None);
        assert_eq!(classify_pkexec_code(125), None);
        assert_eq!(classify_pkexec_code(128), None);
    }

    #[allow(dead_code)]
    fn _ensure_pause_imported() {
        let _ = pause;
    }
}
