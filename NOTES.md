# Developer notes

## What this is

A Rust workspace with two crates:

- `libguix` — library that drives `guix` from Rust. Subprocess-based; we do **not** link libguile.
- `guix-gui` — Iced 0.13 front-end. Search, install, remove, list, update. Designed for users on Guix System and foreign distros alike.

There is no daemon, no setuid helper, no embedded Guile. The library talks to `guix` via three subprocess strategies (long-lived REPL, one-shot CLI, streamed operation) and the GUI consumes typed events.

## Building / running

```sh
# Dev shell (rust, cargo, gcc, pkg-config, wayland libs)
direnv allow             # auto-loaded via .envrc + manifest.scm

# Cargo
cargo check --workspace
cargo test --workspace --features live-tests   # live tests hit real `guix`
cargo run -p guix-gui
```

Direnv sets `LD_LIBRARY_PATH=$LIBRARY_PATH` because winit `dlopen`s wayland at runtime — running `cargo` outside the shell crashes the GUI at startup.

Two `#[ignore]`d tests trigger interactive polkit prompts; opt in with `cargo test --features live-tests -- --ignored` when you're at the keyboard.

## Why subprocess + REPL, not libguile

Linking libguile would mean shipping a runtime, mixing GCs, and undefined behaviour on cross-FFI panics. `guix repl -t machine` over stdin/stdout gives us structured s-exp values for the hot read paths (search, package metadata) at 10% of the cost. Steel (a standalone Rust Scheme) won't load Guile modules so it can't read `(guix …)`.

Revisit only if performance forces it. It won't.

## The Guix REPL — oddities you must know

The long-lived REPL actor lives in `libguix/src/repl/actor.rs`.

### Machine protocol framing

`guix repl -t machine` writes s-expressions to stdout. Responses can be multi-line (pretty-printed values). **Paren-depth framing is mandatory** — splitting on `\n` will fragment values. `repl/framer.rs` tracks paren depth.

The protocol emits more than one `(values …)` form per request: top-level `use-modules` returns `(values (non-self-quoting <handle> "#<unspecified>"))` before the actual `(values (value …))`. The actor classifies frames and keeps reading until it sees the real payload.

### Fresh-module isolation

Without isolation, `(define + -)` in one eval poisons every subsequent eval until the REPL restarts. Each request is wrapped via `make-module` + `beautify-user-module!` + `eval form-in-fresh-module`. Live test `repl_fresh_module_isolation` pins this — keep it green.

### `lexpr::Value::Cons` has a recursive `Drop`

`lexpr::Value::Cons` is `Box<(Value, Value)>` with the derived `Drop`. An N-cell spine takes N stack frames to drop. At 5000+ packages matching a short query, this **overflowed the 2 MB tokio worker stack** and crashed the GUI.

Consume cons cells via `Cons::into_iter()` / `into_pair()` so the `cdr` moves into the cursor and each cell drops without descending. See `parsers/sexp.rs` and `package.rs::parse_records`. **If you build a new sexp consumer, replicate this.**

Also: `DEFAULT_SEARCH_LIMIT = 200` is enforced server-side via `call-with-current-continuation` inside the Guile snippet. Don't bump it without thinking about the drop story.

### SIGINT cancellation — three layers of guards

`Repl::interrupt()` sends SIGINT to the REPL child to cancel an in-flight eval (e.g. when the user types a new search query). Doing this naively kills the REPL. We have **three layers** that all need to stay in sync:

1. **GUI** — only call `interrupt()` when a search is actually in flight (`searching == true`) AND warmup has completed (`warmup_done == true`). Stale `SearchCompleted` replies must still clear `searching`, otherwise a subsequent keystroke fires SIGINT against an idle REPL.
2. **Rust (`Inner.in_flight: AtomicBool`)** — `interrupt()` early-returns when `in_flight == false`, sidestepping the syscall entirely for the common case. An RAII guard around `handle_one` keeps the flag honest.
3. **Scheme (`%guix-rs-in-eval?`)** — the SIGINT handler installed in the REPL subprocess only raises an exception when the flag is `#t`. The flag is toggled via `dynamic-wind` **inside** the per-eval `with-exception-handler #:unwind? #t`, so even the kernel-delivery race window between Rust's check and signal arrival is safe: the exception either lands inside our handler scope, or the Scheme handler refuses to raise.

All three are needed. Removing any one re-opens the "broken pipe + Guile backtrace" failure mode.

### Warmup must fully prime submodules

The ~5000 package definitions live in submodules (`(gnu packages base)`, `(gnu packages abiword)`, …) that Guile loads **lazily** on first reference. `Repl::warmup()` runs `(fold-packages (lambda (_ acc) acc) #t)` to force every submodule to load *before* `warmup_done` flips.

If warmup only did `(use-modules (gnu packages))`, the user's first real `fold-packages` walk would still trigger lazy loading — and a mid-walk SIGINT would corrupt the module cache (Guile's loader marks the failed module as "failed to load: Interrupted", and downstream packages get `unbound variable` errors that look catastrophic in the GUI). Don't downgrade warmup.

Warmup is ~10–15 s on a truly cold host, ~1–2 s when Guile's `.go` cache is warm.

### REPL-native progress events via fd 3

For write ops that are driven through the REPL (`pull`, `install`, `upgrade`, `remove`), we wire **fd 3** in the child via `pre_exec` `dup2`, and the Scheme payload writes structured event s-expressions to that fd from inside `with-status-report`. The Rust side parses each event into a typed `ProgressEvent` — no fragile stderr-regex scraping. See `libguix/src/repl/op.rs` and `(guix status)` upstream.

Two non-obvious bits:

- **`call-with-status-verbosity` monkey-patch.** Both `guix-pull` and `guix-package` internally call `(with-status-verbosity verbosity …)`, which re-parameterises `current-build-output-port` away from our handler. The Scheme payload no-ops `call-with-status-verbosity` so the outer `with-status-report` survives. This is a hack — but it works and is documented inline. A clean fix would be upstreaming a `current-status-handler` parameter in `(guix status)`.
- **Parent must close its copy of the write fd post-spawn.** Otherwise the fd-3 reader never sees EOF when the child exits. Hit this in testing; fix is in `op.rs` after `cmd.spawn()`.

The legacy stderr-parsing path (`parsers/progress.rs`) still drives `system().pull()` and `system().reconfigure()` — those go through `pkexec` and the fd-3 trick doesn't survive the privilege boundary (see next section). Both paths emit the same `ProgressEvent` enum, so the GUI doesn't care which source produced an event.

### `LC_ALL=C` is mandatory on every guix spawn

Without it, recutils field names get translated (`Description:` → `Beschreibung:` etc.) and parsers silently break. Centralised in `cmd.rs::guix_cmd` and the REPL actor's spawn. Always go through these.

## Privileged operations — what works, what doesn't

`guix system reconfigure` and `sudo guix pull` (root catalog) require root. We use **polkit + `pkexec`**:

- `.policy` file shipped via the `libguix-polkit` Guix package (panther → `px/packages/libguix.scm`). Installed via `simple-service polkit-service-type` in the user's system config. On foreign distros: copy to `/etc/polkit-1/actions/` manually — see `polkit/README.md`.
- Until the user reconfigures their system to include the policy, pkexec falls back to the generic `org.freedesktop.policykit.exec` action — still works, just no `auth_admin_keep` so each call re-prompts.
- A polkit auth agent must be running in the user session (`lxqt-policykit-agent`, `polkit-gnome-authentication-agent-1`, etc.). `system.rs::auth_agent_present` does a best-effort `/proc/*/comm` scan against a hard-coded allowlist. False negative? Set `LIBGUIX_SKIP_AGENT_CHECK=1` to bypass.

### `-L` flag positioning matters for polkit

The polkit action enforces `argv1=system argv2=reconfigure`. `-L` must come **after** the subcommand: `guix system reconfigure -L PATH FILE`. `build_reconfigure_args` in `system.rs` already does this; don't reorder.

### Cancel cannot stop pkexec'd operations

POSIX permission boundary: after pkexec does `setresuid(0, 0, 0)` and execs guix, the child is RUID/EUID/SUID root. Our non-root GUI's `kill()` returns `EPERM`. No code path fixes this short of a privileged helper daemon, which PLAN.md explicitly rules out.

GUI mitigation: the Cancel button is disabled with `CANCEL_PKEXEC_TOOLTIP` for `OpKind::SystemPull` and `OpKind::Reconfigure`. **Don't try to "fix" it by adding more force.**

### Polkit invokes the *system* guix, not the user's pulled guix

`/run/current-system/profile/bin/guix` — frozen at last-reconfigure time. Configs that import modules added to a channel **after** the last reconfigure fail with `no code for module (…)`. Users hit this when:

- A new package is added to a custom channel but the system hasn't been reconfigured yet to bake in the new commit.
- The new module exists only in a local checkout and isn't published to the channel.

GUI mitigation: stderr is scanned for the symptom (`BOOTSTRAP_HINT_PATTERN`) and the overlay surfaces a manual bootstrap command:

```
sudo guix system reconfigure -L /path/to/load /path/to/config.scm
```

(`sudo` resolves to root's *pulled* guix, which has the new commits if pulled, or sees local paths via `-L`.) For local-only modules: user adds the directory under System → Advanced load paths and retries.

## The two catalogs — user and root

```
/var/guix/profiles/per-user/$USER/current-guix    ← `guix pull` (user)
/var/guix/profiles/per-user/root/current-guix     ← `sudo guix pull` (root)
/var/guix/profiles/system                          ← `guix system reconfigure`
```

The GUI exposes both pulls as separate buttons in the Updates tab. `guix system reconfigure` does **not** auto-pull channels — the visible channel fetch during reconfigure is `guix-for-channels` building the *deployed system's* guix binary, not refreshing the catalog used for package resolution. Source proof: `guix/scripts/system.scm` calls `maybe-suggest-running-guix-pull` which is only a hint.

Implication: to deploy fresh package versions, the user needs both `sudo guix pull` and then `sudo guix system reconfigure`. The Updates tab's two-button layout reflects this honestly.

mtimes of the three profile symlinks drive the "Last pulled / Last reconfigured" hints in the Updates view. They're refreshed via `tokio::fs::symlink_metadata` alongside the channel list — never call sync `std::fs` from inside `view()`.

## Streaming progress events

`ProgressEvent` (`libguix/src/types.rs`) is the union of every signal we extract from a running op. Coalesced in the library into ~50 ms batches (`Vec<ProgressEvent>`) to avoid per-line re-render storms.

Variants you'll see most:

- `BuildStart` / `BuildDone` / `BuildFailed` — daemon build events. `.drv` paths are real and complete.
- `SubstituteDownload { item, bytes_done, bytes_total }` / `SubstituteDownloadDone { item, bytes_total }` — substitute fetches. The two-event split (progress + done) lets the structured view show a real download counter.
- `Line { stream, text, redraw }` — fallback for anything we don't classify. `redraw: true` means the line was `\r`-terminated (in-place update).
- `ExitSummary { code, duration_secs }` — synthesised at end-of-stream by the operation driver, always last.

Unmapped events from the REPL fd-3 stream land as `[repl-op] (raw-sexp)` Line events; the GUI hides those unless `GUIX_GUI_DEBUG_EVENTS=1` is set.

### `Operation` drop = cancel — hold the whole struct, not just its stream

`Operation` contains an `EventStream` and a `DropGuard` that wires drop → SIGTERM to the child. **Pulling `events` out and dropping the rest** kills the subprocess immediately. The subscription wrapper in `operation_subscription.rs` keeps the whole `Operation` alive inside its `unfold` state for exactly this reason.

### Cancellation = graceful kill, not `kill_on_drop`

`kill_on_drop(true)` is `SIGKILL` only. SIGKILL-ing a reconfigure mid-`grub-install` is not acceptable. `process::graceful_kill` sends SIGTERM, waits ≤ 5 s, then escalates to SIGKILL. `kill_on_drop` stays on as a panic-safety backstop, but the explicit path is what we want.

## iced 0.13 gotchas

- **Tasks with cancellation.** Use `Task::abortable()` + `Handle::abort_on_drop()` for "cancel previous on new input" — store the handle in state; reassigning drops the old one and cancels it. Search uses seq numbers + `interrupt()` instead, mostly for historical reasons; both patterns work.
- **Live timers.** `view()` only runs when a `Message` dispatches. For an updating elapsed-time string, subscribe to `iced::time::every(Duration::from_secs(1))` while the op is active and emit a no-op `Message::Tick`. The dispatch itself triggers the redraw.
- **No indeterminate `progress_bar`.** `iced::widget::progress_bar(0.0..=1.0, frac)` exists in 0.13 but has no indeterminate variant. Downloads without a known `bytes_total` render a flat 0.0 bar with a numeric MB label as the activity signal.
- **`view()` is `&self`.** Anything `&mut` (vt100 scrollback offset reads, mtime stats) must happen in `update()` and be cached. Reading the filesystem in `view()` blocks the UI thread; a synchronous syscall per render at 60 fps is a visible stall.
- **iced text widgets aren't selectable.** `text_editor` is, but rebuilds its `Content` on every state change which kills any in-flight selection. We picked the **Copy button** in the progress overlay — captures `terminal.scrollback() + rows()` via `iced::clipboard::write`. If you ever want real selection, the trade-off is selection-loss-during-live-updates or a complex diff-and-append on `Content`.

## Build output, `\r`-redraw, ANSI

Guix assumes its stdout is a terminal. Progress percentages are `\r`-overwrites + ANSI clear-line. The library strips ANSI early in `parsers/lines.rs`; the GUI's vt100 buffer (`guix-gui/src/terminal_buffer.rs`, 40 rows × 120 cols, 20 000-row scrollback) reconstructs synthetic bytes from `ProgressEvent`s:

- `Line { redraw: true }` → `text + \r` (overwrite same row)
- `Line { redraw: false }` → `text + \r\n` (advance row + reset cursor; vt100 doesn't auto-CR on bare `\n`)
- Typed events → rendered via `format_event` then `\r\n`

The path is lossy for colors — ANSI is stripped at the splitter. A full-fidelity path would route raw bytes from the lib's reader tasks directly to vt100. Not yet built.

The structured progress overlay (`views/progress.rs`) is the default view; the terminal buffer is the opt-in log view (toggle via `Message::ToggleLog`; default persisted in Settings as `show_log_by_default`).

## Known upstream issues

- **[#74396](https://issues.guix.gnu.org/74396)** — channel shadowed by new pull. Supposedly fixed; still observed in practice. Detected via stderr regex during pull/reconfigure, surfaced as `KnownBug::ChannelShadow74396` with a link. Don't try to work around — the user needs to know.
- **No upstream polkit action exists.** `guix-daemon` is a different beast (it runs as root via a socket). Our `.policy` is the first polkit action for `guix system reconfigure`. Expect occasional drift if upstream ships one — we'd want to converge.

## `is_pure_chrome` filter on progress lines

Guix renders progress bars with `▕███▏` (Unicode) or `[####]` (ASCII). Iced fonts often lack block-glyph coverage so Unicode renders as missing-glyph rectangles (`[][][]`). The filter drops any frame with no alphanumeric character. `[####] 50%` survives (digits); `▕███▏` doesn't.

## Subprocess hygiene

- **Resolve `guix` binary once**, cache as `PathBuf` in `Guix`. `guix pull` rewrites `~/.config/guix/current` — re-resolving mid-session can rug-pull the running process.
- **Version-check at startup** via `guix --version`; refuse below pinned minimum. The repl machine protocol is undocumented and varies across versions.
- **No stdin tty.** Substitute servers / GPG can prompt; without a tty the prompt goes nowhere and the process hangs. Detect "no output for N seconds" during builds as a soft warning. Pre-authorise channels where possible; `--no-substitutes` is a documented fallback.
- **CPU-bound parsing inside the async task**, never in `update()`. `spawn_blocking` if a parser becomes hot.

## Direnv + manifest.scm

`.envrc` does `watch_file manifest.scm`. Edit `manifest.scm`, direnv auto-reloads. **Don't run `cargo` outside the direnv shell** — winit's wayland `dlopen` will fail because `LD_LIBRARY_PATH` is empty.

## Quick troubleshooting

- Subprocess output looks scrambled / parsers fail → check `LC_ALL=C` (it's in `cmd.rs::guix_cmd`).
- Cancel button does nothing → check the op kind. pkexec'd ops can't be cancelled by design; the button is disabled with a tooltip.
- Stack overflow on big inputs → suspect `lexpr` recursive drop; consume via `into_iter()` / `into_pair()`.
- Reconfigure fails with `unbound variable` / `no code for module` → it's the system-guix vs pulled-guix split. Bootstrap with `sudo guix system reconfigure -L …` once; the GUI surfaces this hint.
- Search shows a wall of `unbound variable` errors → REPL module cache was corrupted by SIGINT mid-`fold-packages`. Warmup should prevent this; if it doesn't, the safe recovery is to clear the query and retype. Real fix is to ensure warmup completes `fold-packages` before `warmup_done` flips.
- GUI panics at startup with wayland `dlopen` error → run inside the direnv shell.

