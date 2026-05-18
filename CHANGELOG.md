# Changelog

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
