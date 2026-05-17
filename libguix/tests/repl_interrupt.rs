//! `Repl::interrupt` integration — skips if `guix` isn't on PATH.

use std::time::Duration;

use libguix::Guix;

#[tokio::test]
async fn interrupt_cancels_in_flight_eval_and_repl_survives() {
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("skipping: `guix` not available: {e}");
            return;
        }
    };

    // Generous per-eval timeout so the slow form below has time to be
    // running when we send SIGINT. Without this, the eval would race
    // the default 30s timeout (fine here, but cleaner to be explicit).
    let g = g.with_repl_timeout(Duration::from_secs(60));
    let repl = g.repl().await.expect("repl spawn");

    // Sanity: the repl responds to a trivial form.
    let _ = repl.eval("(+ 1 1)").await.expect("warmup eval");

    // Fire a guaranteed-slow eval in a task. The loop calls a body
    // (here `1`) so Guile's async signal queue runs between
    // primitive-eval steps — that's where the installed SIGINT
    // handler raises `'signal`, the wrapper's
    // `with-exception-handler #:unwind? #t` catches it, and the
    // eval returns `(exception "…")` => `Err`.
    let repl_for_slow = repl.clone();
    let slow = tokio::spawn(async move {
        repl_for_slow
            .eval("(let loop ((i 0)) (loop (+ i 1)))")
            .await
    });

    // Give the writer task a moment to actually send the form to the
    // subprocess; otherwise SIGINT can arrive while the repl is still
    // sitting in `read` and the kernel may coalesce it with the
    // pending input — benign, but it leaves the test racy.
    tokio::time::sleep(Duration::from_millis(200)).await;

    repl.interrupt().expect("kill should succeed");

    let res = tokio::time::timeout(Duration::from_secs(5), slow)
        .await
        .expect("slow eval did not return within 5s of SIGINT")
        .expect("slow eval task panicked");
    assert!(
        res.is_err(),
        "slow eval should have returned Err after interrupt, got Ok: {res:?}"
    );

    // The repl process should have survived — a follow-up eval works.
    let v = repl.eval("(+ 1 2)").await.expect("follow-up eval");
    assert_eq!(v.to_string(), "3");
}

/// Regression test for the idle-SIGINT-kills-repl bug. The GUI used to
/// call `interrupt()` against an idle repl in a stale-search edge case;
/// the installed SIGINT handler would raise a `'signal` exception that
/// escaped out of the per-eval `with-exception-handler` (which is only
/// in dynamic scope during an eval) and tear down the `guix repl`
/// subprocess. After the fix, idle interrupts are no-ops and the repl
/// keeps working.
#[tokio::test]
async fn interrupt_during_idle_is_safe() {
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("skipping: `guix` not available: {e}");
            return;
        }
    };
    let g = g.with_repl_timeout(Duration::from_secs(30));
    let repl = g.repl().await.expect("repl spawn");

    // Warm up so the SIGINT handler is installed and the repl is
    // demonstrably idle.
    let _ = repl.eval("(+ 1 1)").await.expect("warmup eval");

    // Interrupt with nothing in flight. Must be a no-op.
    repl.interrupt().expect("idle interrupt should succeed");

    // Give any (incorrectly-delivered) signal a moment to land before
    // the next eval starts setting the in-eval flag.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let v = repl
        .eval("(+ 1 2)")
        .await
        .expect("post-idle-interrupt eval");
    assert_eq!(v.to_string(), "3");
}

/// Defends against rapid-fire typing during the
/// `SearchDebounceTick` / `SearchCompleted` gap, where the GUI could
/// fire many `interrupt()` calls back-to-back. None of them should
/// disturb the idle repl.
#[tokio::test]
async fn interrupt_repeatedly_during_idle_is_safe() {
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("skipping: `guix` not available: {e}");
            return;
        }
    };
    let g = g.with_repl_timeout(Duration::from_secs(30));
    let repl = g.repl().await.expect("repl spawn");

    let _ = repl.eval("(+ 1 1)").await.expect("warmup eval");

    for _ in 0..10 {
        repl.interrupt().expect("idle interrupt should succeed");
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    let v = repl.eval("(+ 5 6)").await.expect("follow-up eval");
    assert_eq!(v.to_string(), "11");
}

/// Regression test for the search-corruption bug. Before the fix,
/// `Repl::warmup()` only loaded the two top-level Guix modules — the
/// ~5000 package submodules under `(gnu packages …)` were still
/// lazy. A SIGINT mid-way through the first `fold-packages` walk
/// corrupted the module cache and the user saw a wall of
/// `unbound variable` errors. The fix moves the full submodule walk
/// into warmup, so by the time the GUI's SIGINT gate opens every
/// submodule is loaded and interruptible walks are safe.
///
/// The test:
/// 1. Warm up the repl (now actually walks fold-packages).
/// 2. Fire a slow fold-packages walk and interrupt it mid-flight.
/// 3. Confirm the slow eval came back Err with Guile's signal exception.
/// 4. Confirm the repl still works (trivial eval).
/// 5. Run a follow-up fold-packages count and assert it's in the
///    expected ballpark — i.e. modules are still healthy, no
///    `unbound variable` corruption.
#[tokio::test]
async fn warmup_loads_submodules_so_fold_packages_is_safe_to_interrupt() {
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("skipping: `guix` not available: {e}");
            return;
        }
    };

    // Warmup can take 10–15 s on a cold host; give it 90 s of headroom
    // so a slow disk doesn't flake the test.
    let g = g.with_repl_timeout(Duration::from_secs(90));
    let repl = g.repl().await.expect("repl spawn");

    repl.warmup().await.expect("warmup should succeed");

    // Slow fold-packages walk: nested loop in the accumulator so each
    // visited package burns enough cycles for SIGINT to land mid-walk.
    // The body matters less than its duration — Guile's async signal
    // queue runs between primitive-eval steps regardless.
    let repl_for_slow = repl.clone();
    let slow = tokio::spawn(async move {
        repl_for_slow
            .eval_with_modules(
                &["(gnu packages)", "(guix packages)"],
                "(fold-packages
                   (lambda (_ acc)
                     (let loop ((i 0))
                       (if (< i 100000) (loop (+ i 1)) acc)))
                   0)",
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    repl.interrupt().expect("kill should succeed");

    let res = tokio::time::timeout(Duration::from_secs(10), slow)
        .await
        .expect("slow fold-packages did not return within 10s of SIGINT")
        .expect("slow eval task panicked");
    assert!(
        res.is_err(),
        "slow fold-packages should have returned Err after interrupt, got Ok: {res:?}"
    );

    // Repl survived.
    let v = repl.eval("(+ 1 2)").await.expect("post-interrupt eval");
    assert_eq!(v.to_string(), "3");

    // Crucial regression check: a follow-up fold-packages walk works
    // and returns a sane count. If the module cache were corrupted by
    // the interrupt we'd either see `unbound variable` errors from
    // half-loaded submodules or a count near zero.
    let count = repl
        .eval_with_modules(
            &["(gnu packages)", "(guix packages)"],
            "(fold-packages (lambda (_ acc) (+ acc 1)) 0)",
        )
        .await
        .expect("post-interrupt fold-packages should succeed");

    let n: i64 = count
        .to_string()
        .parse()
        .unwrap_or_else(|_| panic!("expected integer package count, got {count:?}"));
    assert!(
        n > 1000,
        "expected a healthy package count (>1000), got {n} — module cache likely corrupted"
    );
}
