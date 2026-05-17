//! Live tests — gated on `live-tests`. These shell out to a real `guix`
//! binary on the host.

#![cfg(feature = "live-tests")]

use futures_util::StreamExt;
use libguix::{Guix, ProgressEvent, DEFAULT_SEARCH_LIMIT};

#[tokio::test]
async fn discover_and_search_hello() {
    let g = Guix::discover().await.expect("discover");
    let results = g.package().search("^hello$").await.expect("search");
    assert!(
        results.iter().any(|p| p.name == "hello"),
        "expected `hello` in search results, got: {:?}",
        results.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn show_hello() {
    let g = Guix::discover().await.expect("discover");
    let d = g.package().show("hello").await.expect("show");
    assert_eq!(d.name, "hello");
    assert!(!d.version.is_empty());
}

#[tokio::test]
async fn list_installed_runs() {
    let g = Guix::discover().await.expect("discover");
    let _ = g.package().list_installed().await.expect("list_installed");
}

#[tokio::test]
async fn list_generations_runs() {
    let g = Guix::discover().await.expect("discover");
    let gens = g
        .package()
        .list_generations()
        .await
        .expect("list_generations");
    assert!(!gens.is_empty(), "expected at least one generation");
}

#[tokio::test]
async fn describe_channels_runs() {
    let g = Guix::discover().await.expect("discover");
    let chans = g.describe().channels().await.expect("channels");
    assert!(!chans.is_empty(), "expected at least one channel");
    assert!(chans.iter().any(|c| c.name == "guix"));
}

#[tokio::test]
async fn repl_eval_simple() {
    let g = Guix::discover().await.expect("discover");
    let repl = g.repl().await.expect("repl spawn");
    let v = repl.eval("(+ 1 2)").await.expect("eval");
    assert_eq!(v.as_i64(), Some(3));
}

#[tokio::test]
async fn repl_search_fast_hello() {
    let g = Guix::discover().await.expect("discover");
    let results = g.package().search_fast("hello").await.expect("search_fast");
    let hello = results
        .iter()
        .find(|p| p.name == "hello")
        .unwrap_or_else(|| {
            panic!(
                "expected hello in fast search, got {} results",
                results.len()
            )
        });

    // M5 sprint 2: search_fast returns the full detail set in a single
    // pass over fold-packages. The detail pane in the GUI now renders
    // directly from these fields — no follow-up `show()` call — so we
    // assert all newly-populated fields are present for a known
    // canonical package.
    assert!(!hello.version.is_empty(), "hello.version is empty");
    assert!(!hello.synopsis.is_empty(), "hello.synopsis is empty");
    assert!(!hello.description.is_empty(), "hello.description is empty");
    assert!(
        hello.homepage.starts_with("http"),
        "hello.homepage looks unset: {:?}",
        hello.homepage
    );
    assert!(!hello.license.is_empty(), "hello.license is empty");
    assert!(
        hello.outputs.iter().any(|o| o == "out"),
        "hello.outputs missing `out`: {:?}",
        hello.outputs
    );
}

/// S1 verification: each eval runs in a fresh `(guile-user)`-style module,
/// so a `(define + -)` in eval N cannot rebind `+` for eval N+1.
#[tokio::test]
async fn repl_fresh_module_isolation() {
    let g = Guix::discover().await.expect("discover");
    let repl = g.repl().await.expect("repl spawn");

    // Eval 1: rebind `+` to `-` inside its fresh module.
    let _ = repl.eval("(define + -)").await.expect("eval define");

    // Eval 2: the rebinding must NOT have leaked. `(+ 1 2)` must be 3.
    let v = repl.eval("(+ 1 2)").await.expect("eval add");
    assert_eq!(
        v.as_i64(),
        Some(3),
        "fresh module isolation failed — `+` leaked from earlier eval"
    );
}

/// S1: zero-hit search_fast still parses cleanly.
#[tokio::test]
async fn repl_search_fast_no_hits() {
    let g = Guix::discover().await.expect("discover");
    let results = g
        .package()
        .search_fast("zzzz-no-such-package-zzzz")
        .await
        .expect("search_fast no hits");
    assert_eq!(results.len(), 0);
}

/// M5 regression: a short query like `f` matches thousands of packages.
/// Before the cap, the resulting cons-list overflowed the tokio worker
/// stack when dropped (`lexpr::Value::Cons`'s derived `Drop` recurses
/// through the spine). After the fix, `search_fast` caps at
/// [`DEFAULT_SEARCH_LIMIT`] (200) via `call/cc` inside the Guile fold,
/// and the result spine is drained iteratively via `Cons::into_iter`.
///
/// On a typical Guix host this matches ~5000+ packages; with the cap we
/// expect exactly `DEFAULT_SEARCH_LIMIT` results, and the call must not
/// crash.
#[tokio::test]
async fn repl_search_fast_huge_result_set_does_not_overflow() {
    let g = Guix::discover().await.expect("discover");
    let results = g
        .package()
        .search_fast("f")
        .await
        .expect("search_fast(`f`)");
    assert!(
        results.len() <= DEFAULT_SEARCH_LIMIT,
        "expected <= {} results (the cap), got {}",
        DEFAULT_SEARCH_LIMIT,
        results.len()
    );
    // We expect *many* — definitely more than a handful — otherwise the
    // cap isn't exercising the regression path.
    assert!(
        results.len() >= 50,
        "expected at least 50 matches for `f`, got {} — host package set unusually small?",
        results.len()
    );
}

/// M5: explicit `search_fast_limited` round-trip reports truncation.
#[tokio::test]
async fn repl_search_fast_limited_reports_truncation() {
    let g = Guix::discover().await.expect("discover");
    let res = g
        .package()
        .search_fast_limited("f", 10)
        .await
        .expect("search_fast_limited");
    assert_eq!(res.limit, 10);
    assert!(res.results.len() <= 10);
    assert!(
        res.truncated,
        "expected truncation flag for query `f` capped at 10"
    );
}

/// M5: `warmup` is a fast idempotent no-op once the modules are loaded.
#[tokio::test]
async fn repl_warmup_is_idempotent() {
    let g = Guix::discover().await.expect("discover");
    let repl = g.repl().await.expect("repl spawn");
    repl.warmup().await.expect("warmup 1");
    // Second call must not fail — module load is sticky across evals
    // in the same subprocess.
    repl.warmup().await.expect("warmup 2");
}

/// Exception path: `(error "boom")` should surface as a `ReplProtocol`
/// error rather than killing the repl.
#[tokio::test]
async fn repl_exception_surfaces_as_error() {
    let g = Guix::discover().await.expect("discover");
    let repl = g.repl().await.expect("repl spawn");

    let err = repl.eval("(error \"boom\")").await;
    assert!(err.is_err(), "expected error from `(error \"boom\")`");

    // Subsequent eval still works — the repl survived the exception.
    let v = repl.eval("(+ 1 2)").await.expect("post-exception eval");
    assert_eq!(v.as_i64(), Some(3));
}

// --------------------------------------------------------------------------
// M2 write-op smokes against a temp profile.
//
// These touch the store (`guix package -p <tmp> -i hello` builds against
// the store) but do **not** mutate the user's main profile or
// `/run/current-system`. The temp profile is wiped via `TempDir::drop`.
// --------------------------------------------------------------------------

async fn drain_events(mut op: libguix::Operation) -> Vec<ProgressEvent> {
    let mut out = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        out.extend(batch);
    }
    out
}

fn last_exit(events: &[ProgressEvent]) -> i32 {
    match events.last() {
        Some(ProgressEvent::ExitSummary { code, .. }) => *code,
        other => panic!("expected ExitSummary as final event, got {other:?}"),
    }
}

/// Install `hello` into a temp profile and confirm exit 0 + at least one
/// generation. Slow — actually shells out to guix, but `hello` is tiny
/// and almost always in the substitute cache.
#[tokio::test(flavor = "multi_thread")]
async fn install_hello_into_temp_profile() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let profile = tmp.path().join("profile");

    let g = Guix::discover()
        .await
        .expect("discover")
        .with_profile(&profile);

    let op = g.package().install(&["hello"]).expect("spawn install");
    let events = drain_events(op).await;
    let code = last_exit(&events);
    assert_eq!(code, 0, "install hello failed; events: {events:?}");

    // After a successful install, the profile dir contains a `<profile>-1-link`
    // symlink pointing at the first generation.
    let gen1 = tmp.path().join("profile-1-link");
    assert!(
        gen1.exists(),
        "expected profile-1-link to exist after install, dir listing: {:?}",
        std::fs::read_dir(tmp.path())
            .map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    // `hello` should be in `list_installed` against this profile.
    let installed = g.package().list_installed().await.expect("list installed");
    assert!(
        installed.iter().any(|p| p.name == "hello"),
        "hello not in temp-profile install list: {installed:?}"
    );
}

/// Install then remove `hello`. After both ops, `list_generations` should
/// show at least two generations, and the profile directory should
/// contain both `<profile>-1-link` and `<profile>-2-link` symlinks.
#[tokio::test(flavor = "multi_thread")]
async fn install_then_remove_hello() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let profile = tmp.path().join("profile");

    let g = Guix::discover()
        .await
        .expect("discover")
        .with_profile(&profile);

    let events = drain_events(g.package().install(&["hello"]).expect("install spawn")).await;
    assert_eq!(last_exit(&events), 0, "install failed: {events:?}");

    let gen1 = tmp.path().join("profile-1-link");
    assert!(gen1.exists(), "profile-1-link missing after install");

    let events = drain_events(g.package().remove(&["hello"]).expect("remove spawn")).await;
    assert_eq!(last_exit(&events), 0, "remove failed: {events:?}");

    let gen2 = tmp.path().join("profile-2-link");
    assert!(gen2.exists(), "profile-2-link missing after remove");

    let gens = g.package().list_generations().await.expect("generations");
    assert!(
        gens.len() >= 2,
        "expected >=2 generations after install+remove, got {gens:?}"
    );
}
