# Changelog

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
