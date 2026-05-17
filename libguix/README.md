# libguix

[![Crates.io](https://img.shields.io/crates/v/libguix.svg)](https://crates.io/crates/libguix)
[![Docs.rs](https://docs.rs/libguix/badge.svg)](https://docs.rs/libguix)

Unofficial Rust client library for [GNU Guix](https://guix.gnu.org/).

Wraps the `guix` CLI and its machine-readable REPL (`guix repl -t machine`) so you can drive package management, system administration, and builds from Rust without parsing human-readable output. Designed for long-running operations: every write returns an `Operation` with a coalesced event stream and a `CancelHandle`.

## Module map

The module tree mirrors `guix --help` so navigating the library reads like navigating the CLI:

| Rust | Guix CLI |
|---|---|
| `Guix::package() → PackageOps` | `guix package` (install, remove, upgrade, search, show, generations) |
| `Guix::system() → SystemOps` | `guix system reconfigure` |
| `Guix::pull() → PullOps` | `guix pull` (user catalog + pkexec'd root catalog) |
| `Guix::gc() → GcOps` | `guix gc` |
| `Guix::shell() → ShellOps` | `guix shell` |
| `Guix::build() → BuildOps` | `guix build` |
| `Guix::describe() → DescribeOps` | `guix describe` |

## Examples

Search and list:

```rust
use libguix::Guix;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guix = Guix::discover().await?;
    for hit in guix.package().search_fast("ripgrep").await? {
        println!("{} — {}", hit.name, hit.synopsis);
    }
    Ok(())
}
```

Streaming install (REPL-backed, with structured fd-3 events):

```rust
use futures_util::StreamExt;
use libguix::ProgressEvent;

let mut op = guix.package().install(&["ripgrep", "fd"])?;
while let Some(batch) = op.events_mut().next().await {
    for evt in batch {
        if let ProgressEvent::Line { text, .. } = evt {
            println!("{}", text);
        }
    }
}
op.await_completion().await?;
```

Run a command in an ad-hoc environment (`guix shell`):

```rust
let op = guix.shell().run(&["rust", "rust:cargo"], "cargo", &["check"])?;
op.await_completion().await?;
```

Build a package and collect its store paths (`guix build`):

```rust
let paths = guix.build().run_to_paths(&["hello"]).await?;
for p in paths {
    println!("{}", p.display());
}
```

Pull the user catalog, or the root catalog under pkexec:

```rust
use libguix::SystemPullOptions;

let op = guix.pull().user()?;                                  // ~/.config/guix/current
let op = guix.pull().as_root(SystemPullOptions::default())?;   // /var/guix/profiles/per-user/root
```

## Requirements

A working `guix` binary on `PATH` (or at `/run/current-system/profile/bin/guix`). Tested against modern Guix releases — see `MIN_GUIX_VERSION_DATE`.

The privileged paths (`SystemOps::reconfigure`, `PullOps::as_root`) additionally need polkit actions installed and an authentication agent running in your session. The library returns a structured `PolkitFailure` error if either is missing.

## Features

| Feature | Default | What it does |
|---------|---------|--------------|
| `tracing` | yes | Emit `tracing` events from the REPL actor and command helpers. |
| `live-tests` | no | Enables tests that shell out to a real `guix` on the host. |
| `blocking` | no | Reserved for a future blocking wrapper API. |

## Status

Pre-1.0. The API will shift as the GUI consuming it (in the same repo) grows. Pin a specific version if you depend on it.

## License

GPL-3.0-only.
