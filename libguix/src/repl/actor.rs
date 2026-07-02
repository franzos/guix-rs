//! Repl actor: long-lived `guix repl -t machine` subprocess.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time;

use crate::error::GuixError;
use crate::repl::framer::Framer;
#[allow(unused_imports)]
use crate::{trace_debug, trace_warn};

const STDERR_RING_BYTES: usize = 64 * 1024;

/// Bounded wait for the SIGINT-induced reply so the in-flight slot
/// drains before we return.
const SIGINT_DRAIN_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct Repl {
    inner: Arc<Inner>,
}

struct Inner {
    tx: mpsc::Sender<Request>,
    stderr_ring: Arc<Mutex<StderrRing>>,
    timeout: Duration,
    child_pid: u32,
    /// Paired with the Scheme-side `%guix-rs-in-eval?` flag — both layers
    /// are required for SIGINT cancellation. See NOTES.md.
    in_flight: Arc<AtomicBool>,
    _writer_task: JoinHandle<()>,
    _reader_task: JoinHandle<()>,
    _stderr_task: JoinHandle<()>,
}

#[derive(Default)]
struct StderrRing {
    buf: VecDeque<u8>,
}

impl StderrRing {
    fn push_line(&mut self, line: &str) {
        for b in line.as_bytes() {
            self.buf.push_back(*b);
        }
        while self.buf.len() > STDERR_RING_BYTES {
            self.buf.pop_front();
        }
    }

    fn snapshot(&self) -> String {
        let bytes: Vec<u8> = self.buf.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[derive(Clone, Copy)]
enum EvalMode {
    /// Fresh `(make-module)` per request — no state leakage across evals.
    /// Used by search, package metadata, and every other read path.
    Fresh,
    /// Evaluates inside the persistent `libguix-rs` namespace — used by
    /// channels and (Phase 5) config ops, whose Scheme helpers are primed
    /// once at actor bootstrap and called into many times. See NOTES.md
    /// "Fresh-module isolation" for why this is opt-in.
    Persistent,
}

struct Request {
    mode: EvalMode,
    modules: Vec<String>,
    form: String,
    reply: oneshot::Sender<Result<lexpr::Value, GuixError>>,
}

async fn err_with_tail(msg: String, ring: &Arc<Mutex<StderrRing>>) -> GuixError {
    let tail = ring.lock().await.snapshot();
    GuixError::ReplProtocol {
        message: msg,
        stderr_tail: tail,
    }
}

impl Repl {
    /// `timeout` is per-eval; a timeout doesn't kill the repl.
    pub(crate) async fn spawn(binary: PathBuf, timeout: Duration) -> Result<Self, GuixError> {
        let mut child: Child = Command::new(&binary)
            .arg("repl")
            .arg("-t")
            .arg("machine")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(GuixError::Spawn)?;

        // Grab PID before `child` moves into the writer task.
        let child_pid = child
            .id()
            .ok_or_else(|| GuixError::repl("repl child has no pid"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| GuixError::repl("failed to capture repl stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GuixError::repl("failed to capture repl stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| GuixError::repl("failed to capture repl stderr"))?;

        let (frame_tx, mut frame_rx) = mpsc::channel::<String>(64);
        let stderr_ring = Arc::new(Mutex::new(StderrRing::default()));

        let reader_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut framer = Framer::new();
            let mut buf = String::new();
            loop {
                buf.clear();
                let n = match reader.read_line(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => break,
                };
                if n == 0 {
                    break;
                }
                let mut frames = Vec::new();
                framer.feed(&buf, &mut frames);
                for f in frames {
                    if frame_tx.send(f).await.is_err() {
                        return;
                    }
                }
            }
        });

        let stderr_task = {
            let ring = stderr_ring.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            trace_debug!(target: "libguix::repl", "stderr: {}", line.trim_end());
                            ring.lock().await.push_line(&line);
                        }
                    }
                }
            })
        };

        let (req_tx, mut req_rx) = mpsc::channel::<Request>(16);
        let in_flight = Arc::new(AtomicBool::new(false));
        let writer_task = {
            let ring = stderr_ring.clone();
            let in_flight = in_flight.clone();
            tokio::spawn(async move {
                let mut stdin = stdin;
                let _child = child;

                // Eat the `(repl-version …)` banner so it isn't mis-delivered.
                match frame_rx.recv().await {
                    Some(banner) => {
                        trace_debug!(target: "libguix::repl", "banner: {}", banner);
                        let _ = banner;
                    }
                    None => return,
                }

                // Guarded SIGINT handler — only raises while %guix-rs-in-eval?
                // is #t. See NOTES.md "SIGINT cancellation".
                //
                // We also pre-allocate the `libguix-rs` persistent
                // namespace here so persistent-mode evals can target it
                // immediately. The helper scripts are primed into it right
                // after the init reply (see below).
                let init = "(begin \
                            (define %guix-rs-in-eval? #f) \
                            (define %libguix-rs-module \
                              (let ((m (make-module))) \
                                (beautify-user-module! m) m)) \
                            (sigaction SIGINT \
                              (lambda (sig) \
                                (when %guix-rs-in-eval? \
                                  (scm-error 'signal #f \"Interrupted\" '() #f)))))\n";
                if stdin.write_all(init.as_bytes()).await.is_err() {
                    return;
                }
                if stdin.flush().await.is_err() {
                    return;
                }
                // Eat the init form's `(values …)` reply.
                let _ = frame_rx.recv().await;

                // Prime the persistent helpers directly against the TOP
                // module (raw write, not wrap_expr) so `%libguix-rs-module`
                // is bound; `catch #t` keeps a broken guix from bricking the
                // REPL — Fresh-mode search stays independent of this module.
                let channel_helper = include_str!("channel_ops.scm");
                let installed_helper = include_str!("installed_ops.scm");
                let prime = format!(
                    "(catch #t \
                       (lambda () \
                         (for-each \
                           (lambda (iface) \
                             (module-use! %libguix-rs-module (resolve-interface iface))) \
                           '((guix read-print) (guix channels) (guix profiles) \
                             (guix packages) (guix utils) (guix describe) (gnu packages))) \
                         (eval '(begin {channel_helper} {installed_helper} #t) %libguix-rs-module) \
                         #t) \
                       (lambda (key . args) #f))\n"
                );
                if stdin.write_all(prime.as_bytes()).await.is_err() {
                    return;
                }
                if stdin.flush().await.is_err() {
                    return;
                }
                // Eat the prime form's single (values …) reply.
                let _ = frame_rx.recv().await;

                while let Some(Request {
                    mode,
                    modules,
                    form,
                    reply,
                }) = req_rx.recv().await
                {
                    // RAII guard pairs with Scheme-side `%guix-rs-in-eval?`.
                    struct InFlightGuard<'a>(&'a AtomicBool);
                    impl Drop for InFlightGuard<'_> {
                        fn drop(&mut self) {
                            self.0.store(false, Ordering::Release);
                        }
                    }
                    in_flight.store(true, Ordering::Release);
                    let guard = InFlightGuard(&in_flight);
                    let res =
                        handle_one(&mut stdin, &mut frame_rx, &ring, mode, &modules, &form).await;
                    drop(guard);
                    let _ = reply.send(res);
                }
            })
        };

        Ok(Repl {
            inner: Arc::new(Inner {
                tx: req_tx,
                stderr_ring,
                timeout,
                child_pid,
                in_flight,
                _writer_task: writer_task,
                _reader_task: reader_task,
                _stderr_task: stderr_task,
            }),
        })
    }

    /// SIGINT the in-flight eval. No-op if idle — SIGINT against an idle
    /// REPL escapes the per-eval handler and kills the subprocess.
    /// See NOTES.md "SIGINT cancellation".
    pub fn interrupt(&self) -> Result<(), GuixError> {
        if !self.inner.in_flight.load(Ordering::Acquire) {
            return Ok(());
        }
        let pid = i32::try_from(self.inner.child_pid)
            .map_err(|_| GuixError::repl(format!("invalid child pid {}", self.inner.child_pid)))?;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGINT,
        )
        .map_err(|e| GuixError::repl(format!("kill: {e}")))?;
        Ok(())
    }

    pub async fn eval(&self, form: &str) -> Result<lexpr::Value, GuixError> {
        self.eval_with_modules(&[], form).await
    }

    /// Forces every `(gnu packages …)` submodule to load. ~10-15 s cold.
    /// See NOTES.md "Warmup must fully prime submodules".
    pub async fn warmup(&self) -> Result<(), GuixError> {
        let _ = self
            .eval_with_modules(
                &["(gnu packages)", "(guix packages)"],
                "(fold-packages (lambda (_ acc) acc) #t)",
            )
            .await?;
        Ok(())
    }

    /// Persistent-namespace eval — runs against the `libguix-rs` module
    /// initialised at spawn. State (definitions, imports) persists across
    /// calls. Used by channels.rs and (Phase 5) config.rs.
    pub async fn eval_persistent(&self, form: &str) -> Result<lexpr::Value, GuixError> {
        self.eval_dispatch(EvalMode::Persistent, &[], form).await
    }

    /// Evaluates in a fresh module — see NOTES.md "Fresh-module isolation".
    pub async fn eval_with_modules(
        &self,
        modules: &[&str],
        form: &str,
    ) -> Result<lexpr::Value, GuixError> {
        self.eval_dispatch(EvalMode::Fresh, modules, form).await
    }

    async fn eval_dispatch(
        &self,
        mode: EvalMode,
        modules: &[&str],
        form: &str,
    ) -> Result<lexpr::Value, GuixError> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .tx
            .send(Request {
                mode,
                modules: modules.iter().map(|s| (*s).to_owned()).collect(),
                form: form.to_owned(),
                reply: tx,
            })
            .await
            .map_err(|_| -> GuixError {
                GuixError::ReplProtocol {
                    message: "repl actor channel closed".into(),
                    stderr_tail: String::new(),
                }
            })?;

        // Pinned so we can await it again after a timeout-induced SIGINT.
        let mut rx = Box::pin(rx);
        let res = time::timeout(self.inner.timeout, &mut rx).await;
        match res {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => Err(self.repl_err("repl actor dropped reply").await),
            Err(_) => {
                // SIGINT the eval and wait briefly for the unwound
                // reply — without this the next request queues behind a
                // never-clearing in-flight slot and looks like a deadlock.
                let _ = self.interrupt();
                let _ = time::timeout(SIGINT_DRAIN_GRACE, &mut rx).await;
                Err(self
                    .repl_err(format!(
                        "repl did not respond within {}s",
                        self.inner.timeout.as_secs_f64()
                    ))
                    .await)
            }
        }
    }

    async fn repl_err<S: Into<String>>(&self, message: S) -> GuixError {
        let tail = self.inner.stderr_ring.lock().await.snapshot();
        GuixError::ReplProtocol {
            message: message.into(),
            stderr_tail: tail,
        }
    }
}

async fn handle_one(
    stdin: &mut ChildStdin,
    frame_rx: &mut mpsc::Receiver<String>,
    stderr_ring: &Arc<Mutex<StderrRing>>,
    mode: EvalMode,
    modules: &[String],
    form: &str,
) -> Result<lexpr::Value, GuixError> {
    let payload = wrap_expr(mode, modules, form);
    let mut bytes = payload.into_bytes();
    bytes.push(b'\n');

    if let Err(e) = stdin.write_all(&bytes).await {
        return Err(err_with_tail(format!("write: {e}"), stderr_ring).await);
    }
    if let Err(e) = stdin.flush().await {
        return Err(err_with_tail(format!("flush: {e}"), stderr_ring).await);
    }

    // One `(values …)` per request; defensively skip stray banner frames.
    let mut waited = 0;
    while let Some(frame) = frame_rx.recv().await {
        waited += 1;
        if waited > 128 {
            return Err(
                err_with_tail("too many frames waiting for reply".into(), stderr_ring).await,
            );
        }
        let parsed = match lexpr::from_str(&frame) {
            Ok(v) => v,
            Err(e) => {
                return Err(err_with_tail(
                    format!("frame parse: {e}; raw: {frame:?}"),
                    stderr_ring,
                )
                .await);
            }
        };
        match classify(&parsed) {
            FrameKind::Banner => continue,
            FrameKind::NonSelfQuoting | FrameKind::Empty => return Ok(lexpr::Value::Null),
            FrameKind::Value(v) => return Ok(v),
            FrameKind::Exception(s) => {
                return Err(err_with_tail(format!("guile exception: {s}"), stderr_ring).await);
            }
            FrameKind::Unknown => {
                trace_warn!(target: "libguix::repl", "unknown frame: {}", frame);
                continue;
            }
        }
    }
    Err(err_with_tail("repl stdout closed mid-reply".into(), stderr_ring).await)
}

/// Linear scan that counts `(` and `)` outside string literals — a coarse
/// sanity check for `wrap_expr` input. Tracks `\` escapes inside strings.
fn scheme_parens_balanced(s: &str) -> bool {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for c in s.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    !in_string && depth == 0
}

/// Fresh-module isolation + per-eval exception handler + dynamic-wind
/// flag toggle for SIGINT. See NOTES.md.
///
/// Persistent mode targets `%libguix-rs-module` (allocated once at spawn)
/// and skips the per-eval `make-module`, so definitions and `module-use!`
/// imports survive across requests.
///
/// `form` is spliced in verbatim — callers must escape interpolated
/// strings via `scheme_string`. Passing untrusted input here is a
/// code-injection bug at the call site.
fn wrap_expr(mode: EvalMode, modules: &[String], form: &str) -> String {
    debug_assert!(
        scheme_parens_balanced(form),
        "wrap_expr: unbalanced parens in form: {form}"
    );
    if !scheme_parens_balanced(form) {
        #[cfg(feature = "tracing")]
        tracing::warn!(target: "libguix::repl", "wrap_expr: unbalanced parens in form (proceeding): {form}");
    }
    let module_expr = match mode {
        EvalMode::Fresh => "(let ((mod (make-module))) (beautify-user-module! mod) mod)",
        EvalMode::Persistent => "%libguix-rs-module",
    };

    let mut imports = String::new();
    if !modules.is_empty() {
        imports.push_str("(for-each (lambda (iface) (module-use! m (resolve-interface iface))) '(");
        for (i, m) in modules.iter().enumerate() {
            if i > 0 {
                imports.push(' ');
            }
            imports.push_str(m);
        }
        imports.push_str("))");
    }

    // dynamic-wind must be INSIDE with-exception-handler so SIGINT lands
    // in scope. The after-thunk clears the flag on normal/unwind both.
    format!(
        "(let ((m {module_expr})) \
            {imports} \
            (with-exception-handler \
                (lambda (e) (list 'exception (object->string e))) \
                (lambda () \
                  (dynamic-wind \
                    (lambda () (set! %guix-rs-in-eval? #t)) \
                    (lambda () (eval '{form} m)) \
                    (lambda () (set! %guix-rs-in-eval? #f)))) \
                #:unwind? #t))",
        module_expr = module_expr,
        imports = imports,
        form = form,
    )
}

enum FrameKind {
    Banner,
    NonSelfQuoting,
    Empty,
    Value(lexpr::Value),
    Exception(String),
    Unknown,
}

fn classify(v: &lexpr::Value) -> FrameKind {
    let Some(mut it) = v.list_iter() else {
        return FrameKind::Unknown;
    };
    let Some(head) = it.next().and_then(lexpr::Value::as_symbol) else {
        return FrameKind::Unknown;
    };
    match head {
        "repl-version" => FrameKind::Banner,
        "values" => {
            let Some(inner) = it.next() else {
                return FrameKind::Empty;
            };
            let Some(mut ii) = inner.list_iter() else {
                return FrameKind::Unknown;
            };
            match ii.next().and_then(lexpr::Value::as_symbol) {
                Some("value") => {
                    let Some(payload) = ii.next() else {
                        return FrameKind::Empty;
                    };
                    if let Some(mut pi) = payload.list_iter() {
                        if pi.next().and_then(lexpr::Value::as_symbol) == Some("exception") {
                            let msg = pi
                                .next()
                                .and_then(lexpr::Value::as_str)
                                .unwrap_or("<no message>")
                                .to_owned();
                            return FrameKind::Exception(msg);
                        }
                    }
                    FrameKind::Value(payload.clone())
                }
                Some("non-self-quoting") => FrameKind::NonSelfQuoting,
                _ => FrameKind::Unknown,
            }
        }
        "exception" => {
            let msg = it
                .next()
                .and_then(lexpr::Value::as_str)
                .unwrap_or("<no message>")
                .to_owned();
            FrameKind::Exception(msg)
        }
        _ => FrameKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexpr::from_str;

    #[test]
    fn classify_banner() {
        let v = from_str("(repl-version 0 1 1)").unwrap();
        assert!(matches!(classify(&v), FrameKind::Banner));
    }

    #[test]
    fn classify_non_self_quoting() {
        let v = from_str("(values (non-self-quoting 2052 \"#<unspecified>\"))").unwrap();
        assert!(matches!(classify(&v), FrameKind::NonSelfQuoting));
    }

    #[test]
    fn classify_value() {
        let v = from_str("(values (value ((\"hello\" \"2.12.3\" \"x\"))))").unwrap();
        match classify(&v) {
            FrameKind::Value(payload) => {
                assert!(payload.list_iter().is_some());
            }
            _ => panic!("expected Value"),
        }
    }

    #[test]
    fn classify_exception_wrapper() {
        let v = from_str("(values (value (exception \"oh no\")))").unwrap();
        match classify(&v) {
            FrameKind::Exception(m) => assert_eq!(m, "oh no"),
            _ => panic!("expected Exception"),
        }
    }

    #[test]
    fn classify_bare_exception() {
        let v = from_str("(exception \"boom\")").unwrap();
        match classify(&v) {
            FrameKind::Exception(m) => assert_eq!(m, "boom"),
            _ => panic!("expected Exception"),
        }
    }

    /// Zero-hit fold returns `(values (value ()))` — accept either
    /// `Value(Null)` or `Empty`.
    #[test]
    fn classify_zero_hits_empty_list() {
        let v = from_str("(values (value ()))").unwrap();
        match classify(&v) {
            FrameKind::Value(payload) => {
                assert!(payload.is_null() || payload.list_iter().is_some());
            }
            FrameKind::Empty => {}
            _ => panic!("expected Value(Null) or Empty"),
        }
    }

    #[test]
    fn wrap_expr_includes_fresh_module() {
        let w = wrap_expr(EvalMode::Fresh, &["(gnu packages)".into()], "(+ 1 2)");
        assert!(w.contains("make-module"));
        assert!(w.contains("beautify-user-module!"));
        assert!(w.contains("resolve-interface"));
        assert!(w.contains("(gnu packages)"));
        assert!(w.contains("eval"));
        assert!(w.contains("(+ 1 2)"));
    }

    /// Pins layer 3 of the SIGINT cancellation invariant: dynamic-wind
    /// must be INSIDE with-exception-handler.
    #[test]
    fn wrap_expr_toggles_in_eval_flag_via_dynamic_wind() {
        let w = wrap_expr(EvalMode::Fresh, &[], "(+ 1 2)");
        assert!(w.contains("dynamic-wind"), "missing dynamic-wind: {w}");
        assert!(
            w.contains("(set! %guix-rs-in-eval? #t)"),
            "missing in-eval set: {w}"
        );
        assert!(
            w.contains("(set! %guix-rs-in-eval? #f)"),
            "missing in-eval clear: {w}"
        );
        let weh = w.find("with-exception-handler").expect("weh");
        let dw = w.find("dynamic-wind").expect("dw");
        assert!(
            weh < dw,
            "dynamic-wind must be inside with-exception-handler: {w}"
        );
    }

    /// Regression: `'(FORM)` makes eval apply a literal — must be `'FORM`.
    #[test]
    fn wrap_expr_quotes_form_without_extra_parens() {
        let w = wrap_expr(EvalMode::Fresh, &[], "(+ 1 2)");
        assert!(w.contains("'(+ 1 2)"));
        assert!(!w.contains("'((+ 1 2))"));
    }

    #[test]
    fn wrap_expr_omits_for_each_when_no_modules() {
        let w = wrap_expr(EvalMode::Fresh, &[], "(+ 1 2)");
        assert!(!w.contains("for-each"));
        assert!(w.contains("(+ 1 2)"));
    }

    /// Persistent mode must target the named `%libguix-rs-module`, not
    /// allocate a fresh one — definitions and imports survive across
    /// calls. This is the read side of the channels-ops contract.
    #[test]
    fn wrap_expr_persistent_targets_named_module() {
        let w = wrap_expr(EvalMode::Persistent, &[], "(+ 1 2)");
        assert!(
            w.contains("%libguix-rs-module"),
            "missing persistent namespace: {w}"
        );
        assert!(
            !w.contains("make-module"),
            "persistent mode must not allocate a fresh module: {w}"
        );
    }

    #[test]
    fn stderr_ring_caps_at_limit() {
        let mut r = StderrRing::default();
        for _ in 0..(STDERR_RING_BYTES / 16 + 1024) {
            r.push_line("0123456789ABCDEF");
        }
        let snap = r.snapshot();
        assert!(snap.len() <= STDERR_RING_BYTES);
        assert!(!snap.is_empty());
    }
}
