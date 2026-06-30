# Changelog

## [0.1.11] - 2026-06-30

### Fixed
- System pull/reconfigure silently did nothing without a detected polkit agent

### Changed
- Polkit agent check is now advisory; privileged ops no longer blocked by it

### Added
- Updates tab shows a manual `sudo` fallback command when a privileged op fails

## [0.1.10] - 2026-06-30

### Added
- Colored letter-tile placeholder for apps without an icon (and when metadata is off)

### Changed
- App metadata (icons, screenshots) now fetched by default; disable in Settings

### Fixed
- Sidebar icons now render on systems without an emoji font (bundled Noto Emoji subset)

## [0.1.9] - 2026-06-30

### Added
- Auto-refresh the desktop application menu after install (KDE, XFCE, MATE, LXQt)

## [0.1.8] - 2026-06-30

### Fixed
- Blank UI on machines without system fonts: DejaVu fonts now bundled

## [0.1.7] - 2026-06-03

### Added
- `libguix`: `guix system init` operation for installers
- `libguix`: already-root execution mode — runs `guix` directly without `pkexec`
- `libguix`: build-flag pass-through on pull/reconfigure/init (`--substitute-urls`, `--no-substitutes`, `--cores`, `--max-jobs`, `--system`)
- `libguix`: `--channels=<file>` on `guix pull`
- `libguix`: `guix archive --authorize` wrapper
- `libguix`: opt-in retry for transient substitute/network failures
- `libguix`: progress state machine reusable as `libguix::progress::Summary`

### Changed
- Progress percentage uses the up-front build/download totals when available

## [0.1.6] - 2026-06-02

### Added
- Translatable UI — follows the system locale, with an in-app language picker and live switching
- Translations: German, Spanish, French, Italian, Brazilian Portuguese, Simplified Chinese

## [0.1.5] - 2026-05-24

### Added
- Confirmation step before `pkexec guix system reconfigure` — shows the exact config path and every `-L` load path being authorised
- Channel field validation (URL scheme/length, control & deceptive Unicode, branch chars, intro commit/fingerprint shape)
- Provenance label and trust warning in the channel-add confirm card
- Corrupt-settings JSON is copied to a `.bak` sibling on load (existing `.bak` preserved)
- Image magic-byte sniff (PNG/JPEG/WebP) — lightbox and disk cache reject unsupported bytes

### Changed
- `channels.scm` atomic write now refuses symlinks and uses a random-named tempfile in the same dir
- `discovery` HTTP client is `https_only` with a 2-hop redirect cap
- REPL timeout sends SIGINT and waits briefly for the reply to drain so the next request doesn't queue behind a dead slot
- REPL fd-3 pipe handling switched to RAII (`OwnedFd`), removing manual `close` and a fork-race window
- `xdg-open` invocation parses + canonicalises URLs via `url::Url` and rejects control bytes

### Fixed
- `MetadataClient` builder failure no longer silently drops user-agent and timeout — surfaces error and falls back to a labelled degraded client
- `safe_name` for on-disk cache keys can no longer produce `.`, `..`, or dotfile names; length capped at 128
- `resolve_profile_path` returns an error instead of defaulting to `/` when `HOME` is unset

## [0.1.4] - 2026-05-19

### Added
- Channels tab — view, add, and remove entries in `~/.config/guix/channels.scm` straight from the GUI, with restore from `.bak`. User-level only; system channels are deferred
- Optional Discover sub-mode (off by default) — browse channels and packages indexed by `toys.whereis.social` and add them with one click, including an "Add channel & install" shortcut for packages
- Automatic rollback offer when a `guix pull` fails after a channel edit, with known-bug hints surfaced from the pull's progress stream
- Per-channel Remove warning listing the installed packages attributed to that channel via `(guix describe) package-channels`

### Changed
- Atomic write for `channels.scm` now `fsync`s the tmp file and the parent directory, and copies (rather than renames) the previous file to `.bak` so the canonical path is never absent mid-write
- Settings and Channels surfaces distinguish user-level paths from system-level paths more clearly

## [0.1.3] - 2026-05-18

### Added
- About tab in the sidebar with project info, contributors, source link, and license notices
- Development category on the Home tab, with `zed` as the first entry
- Channel-gated Home tiles — apps that require a specific Guix channel (e.g. `pantherx`) only appear when that channel is configured. Initial set: `rnote`, `appflowy`, `qalculate-gtk`, `gitbutler`, `tidal-hifi`, `halloy`, `rustdesk`, `discord`, `syncthingtray`

### Changed
- Relicensed `libguix` as MIT OR Apache-2.0 so other tools can embed it freely. `guix-gui` remains GPL-3.0-only
- Channel list is now loaded at startup (no longer only when opening the Updates tab) so Home filtering works from the first frame

## [0.1.2] - 2026-05-18

### Added
- Home tab — curated grid of well-known GUI applications grouped by category, shown as the default landing view
- Disk cache for icons and screenshots under `$XDG_CACHE_HOME/guix-gui/` with a 365-day TTL and a "Clear cache" button in Settings
- Opt-in icons + screenshots in the Search detail pane, fetched from Flathub and screenshots.debian.net (toggle per source in Settings)
- Lightbox preview when clicking a screenshot (Esc or click-outside to close)
- Clickable homepage link in the Search detail pane (opens via `xdg-open`)

### Changed
- Selected row in Search results is now highlighted (same treatment as hover)
- The active tab is no longer persisted across launches — the app always opens on Home
