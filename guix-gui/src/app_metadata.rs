//! Best-effort icon + screenshot fetcher for the search detail pane.
//!
//! Pulls AppStream-style metadata from third-party catalogs and maps Guix
//! package names → upstream component IDs via lightweight heuristics:
//!
//! * **Flathub** — `GET https://flathub.org/api/v2/appstream` once per
//!   session for the full ID list, then build a reverse index from the
//!   last segment of each reverse-DNS ID. Per-app details come from
//!   `GET /api/v2/appstream/{id}`.
//! * **screenshots.debian.net** — keyed by binary package name, which
//!   often matches the Guix name directly. No mapping table needed.
//!
//! Strictly opt-in via `Settings::app_metadata`. All fetches are
//! best-effort; failures degrade silently to "no metadata" rather than
//! surfacing as errors in the UI.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::RwLock;

use crate::settings::AppMetadataSettings;

const FLATHUB_API: &str = "https://flathub.org/api/v2";
const DEBIAN_SCREENSHOTS_API: &str = "https://screenshots.debian.net/json/package";
const USER_AGENT: &str = concat!("guix-gui/", env!("CARGO_PKG_VERSION"));
const HTTP_TIMEOUT: Duration = Duration::from_secs(8);
const SCREENSHOT_PREFETCH: usize = 3;

/// Disk cache TTL for icons and screenshot bytes. Long because icons
/// rarely change and re-downloading on every cold start defeats the
/// purpose. The Settings tab exposes a manual clear for the rare case
/// where an icon does change upstream.
const DISK_CACHE_TTL: Duration = Duration::from_secs(365 * 24 * 60 * 60);
const ICONS_BY_NAME_SUBDIR: &str = "icons-by-name";
const BYTES_SUBDIR: &str = "bytes";

#[derive(Clone)]
pub struct MetadataClient {
    http: reqwest::Client,
    flathub_index: Arc<RwLock<Option<FlathubIndex>>>,
    /// Root for persistent icon + screenshot bytes. `None` disables the
    /// on-disk layer (falls back to per-session in-memory only).
    cache_root: Option<PathBuf>,
}

/// Reverse map: lowercased last-segment of a Flathub component ID →
/// full ID. Populated on first use, kept for the session.
#[derive(Default)]
struct FlathubIndex {
    by_lower_tail: std::collections::HashMap<String, Vec<String>>,
}

impl FlathubIndex {
    fn build(ids: Vec<String>) -> Self {
        let mut by_lower_tail: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::with_capacity(ids.len());
        for id in ids {
            let tail = id.rsplit('.').next().unwrap_or(&id).to_lowercase();
            by_lower_tail.entry(tail).or_default().push(id);
        }
        Self { by_lower_tail }
    }

    /// Heuristic match for a Guix package name. Returns the best
    /// candidate ID (preferring well-known org prefixes when the tail
    /// alone is ambiguous).
    fn lookup(&self, guix_name: &str) -> Option<String> {
        let key = guix_name.to_lowercase();
        let candidates = self.by_lower_tail.get(&key)?;
        if candidates.is_empty() {
            return None;
        }
        // Single match is the easy case.
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }
        // Tie-break: prefer canonical desktop-project prefixes over
        // user-namespaced forks (com.github.*, io.github.*).
        const PREFERRED: &[&str] = &["org.gnome.", "org.kde.", "org.gnu.", "org.freedesktop."];
        for prefix in PREFERRED {
            if let Some(hit) = candidates.iter().find(|id| id.starts_with(prefix)) {
                return Some(hit.clone());
            }
        }
        // Fall back to the first match — caller can override if wrong.
        Some(candidates[0].clone())
    }
}

#[derive(Debug, Clone)]
pub struct AppMetadata {
    pub flathub: Option<FlathubMetadata>,
    pub debian_screenshots: Vec<Screenshot>,
}

impl AppMetadata {
    pub fn is_empty(&self) -> bool {
        self.flathub.is_none() && self.debian_screenshots.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Screenshot {
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct FlathubMetadata {
    pub component_id: String,
    pub icon_bytes: Option<Vec<u8>>,
    pub screenshots: Vec<Screenshot>,
}

// --- API response shapes -------------------------------------------------

#[derive(Deserialize)]
struct FlathubAppstream {
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    screenshots: Vec<FlathubScreenshot>,
}

#[derive(Deserialize)]
struct FlathubScreenshot {
    #[serde(default)]
    sizes: Vec<FlathubScreenshotSize>,
    #[serde(default)]
    src: Option<String>,
}

#[derive(Deserialize)]
struct FlathubScreenshotSize {
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    width: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct DebianScreenshotsResp {
    #[serde(default)]
    screenshots: Vec<DebianScreenshot>,
}

#[derive(Deserialize)]
struct DebianScreenshot {
    #[serde(default)]
    large_image_url: Option<String>,
    #[serde(default)]
    screenshot_url: Option<String>,
}

// --- Client --------------------------------------------------------------

impl MetadataClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(HTTP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let cache_root =
            directories::ProjectDirs::from("", "", "guix-gui").map(|d| d.cache_dir().to_path_buf());
        Self {
            http,
            flathub_index: Arc::new(RwLock::new(None)),
            cache_root,
        }
    }

    pub fn cache_root(&self) -> Option<&Path> {
        self.cache_root.as_deref()
    }

    /// Wipe both cache subdirectories. Doesn't touch the in-memory
    /// Flathub index — that's the caller's responsibility, since it
    /// can't be replenished from disk anyway.
    pub async fn clear_disk_cache(&self) -> Result<(), String> {
        let Some(root) = self.cache_root.as_ref() else {
            return Ok(());
        };
        for sub in [ICONS_BY_NAME_SUBDIR, BYTES_SUBDIR] {
            let p = root.join(sub);
            match tokio::fs::remove_dir_all(&p).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", p.display())),
            }
        }
        Ok(())
    }

    /// Return cached bytes if the file exists and hasn't aged past the
    /// TTL. Anything older is treated as a miss so the next fetch
    /// overwrites it.
    async fn read_cache(&self, sub: &str, key: &str) -> Option<Vec<u8>> {
        let path = self.cache_root.as_ref()?.join(sub).join(key);
        let meta = tokio::fs::metadata(&path).await.ok()?;
        let modified = meta.modified().ok()?;
        let age = std::time::SystemTime::now().duration_since(modified).ok()?;
        if age > DISK_CACHE_TTL {
            return None;
        }
        tokio::fs::read(&path).await.ok()
    }

    /// Best-effort write — failures (no XDG cache dir, permission
    /// denied, disk full) degrade silently to "no disk cache" rather
    /// than surfacing as a fetch error.
    async fn write_cache(&self, sub: &str, key: &str, bytes: &[u8]) {
        let Some(root) = self.cache_root.as_ref() else {
            return;
        };
        let dir = root.join(sub);
        if tokio::fs::create_dir_all(&dir).await.is_err() {
            return;
        }
        // tmp + rename for atomicity — partial writes from a crash mid-
        // download shouldn't poison the cache with a truncated image.
        let tmp_path = dir.join(format!("{key}.tmp"));
        let final_path = dir.join(key);
        if tokio::fs::write(&tmp_path, bytes).await.is_ok() {
            let _ = tokio::fs::rename(&tmp_path, &final_path).await;
        }
    }

    /// Guix package names are mostly `[a-z0-9-+.]`, but a few use `@`,
    /// `+`, or `_`. Map anything outside the safe filename alphabet to
    /// `_` so the on-disk key stays valid on any sane filesystem.
    fn safe_name(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '.' => c,
                _ => '_',
            })
            .collect()
    }

    /// 64-bit non-crypto hash of the URL — collisions among the few
    /// hundred URLs we'll ever cache are vanishingly unlikely.
    fn url_key(url: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    /// Disk-cached counterpart to `fetch_bytes` — read-through with a
    /// write-back on miss. Used for everything image-shaped (icons in
    /// the full Search detail fetch, all screenshots).
    async fn fetch_bytes_cached(&self, url: &str) -> Option<Vec<u8>> {
        let key = Self::url_key(url);
        if let Some(bytes) = self.read_cache(BYTES_SUBDIR, &key).await {
            return Some(bytes);
        }
        let bytes = self.fetch_bytes(url).await?;
        self.write_cache(BYTES_SUBDIR, &key, &bytes).await;
        Some(bytes)
    }

    pub async fn fetch(&self, guix_name: &str, cfg: AppMetadataSettings) -> AppMetadata {
        let mut out = AppMetadata {
            flathub: None,
            debian_screenshots: Vec::new(),
        };
        if !cfg.enabled {
            return out;
        }

        let flathub_fut = async {
            if cfg.use_flathub {
                self.fetch_flathub(guix_name).await
            } else {
                None
            }
        };
        let debian_fut = async {
            if cfg.use_debian_screenshots {
                self.fetch_debian(guix_name).await
            } else {
                Vec::new()
            }
        };
        let (fh, dbn) = tokio::join!(flathub_fut, debian_fut);
        out.flathub = fh;
        out.debian_screenshots = dbn;
        out
    }

    /// Lightweight path for the Home tab: resolve the Flathub component
    /// for `guix_name` and fetch just its icon bytes. Skips screenshot
    /// metadata so opening Home doesn't fan out into ~7 requests per
    /// tile. On a disk cache hit, the network is skipped entirely —
    /// including the appstream/{id} JSON lookup, which is the whole
    /// point of caching by Guix name rather than by URL.
    pub async fn fetch_icon(&self, guix_name: &str) -> Option<Vec<u8>> {
        let key = Self::safe_name(guix_name);
        if let Some(bytes) = self.read_cache(ICONS_BY_NAME_SUBDIR, &key).await {
            return Some(bytes);
        }
        let id = self.flathub_lookup(guix_name).await?;
        let url = format!("{FLATHUB_API}/appstream/{id}");
        let resp: FlathubAppstream = self
            .http
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        let icon_url = resp.icon?;
        let bytes = self.fetch_bytes(&icon_url).await?;
        self.write_cache(ICONS_BY_NAME_SUBDIR, &key, &bytes).await;
        Some(bytes)
    }

    async fn fetch_flathub(&self, guix_name: &str) -> Option<FlathubMetadata> {
        let id = self.flathub_lookup(guix_name).await?;
        let url = format!("{FLATHUB_API}/appstream/{id}");
        let resp: FlathubAppstream = self
            .http
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;

        let mut screenshot_urls: Vec<String> = Vec::new();
        for s in resp.screenshots.into_iter().take(SCREENSHOT_PREFETCH) {
            // Prefer the first sized variant with a numeric width, fall
            // back to top-level `src`. Flathub sizes range from thumb
            // to original; the first sized entry is usually a
            // reasonable mid-resolution image.
            let pick = s
                .sizes
                .iter()
                .find_map(|sz| sz.src.clone().filter(|_| sz.width.is_some()))
                .or_else(|| s.sizes.into_iter().find_map(|sz| sz.src))
                .or(s.src);
            if let Some(u) = pick {
                screenshot_urls.push(u);
            }
        }

        let icon_bytes = if let Some(url) = resp.icon.as_deref() {
            self.fetch_bytes_cached(url).await
        } else {
            None
        };
        let screenshots = self.fetch_screenshot_bytes(screenshot_urls).await;

        Some(FlathubMetadata {
            component_id: id,
            icon_bytes,
            screenshots,
        })
    }

    async fn fetch_debian(&self, guix_name: &str) -> Vec<Screenshot> {
        let url = format!("{DEBIAN_SCREENSHOTS_API}/{guix_name}");
        let Ok(resp) = self.http.get(&url).send().await else {
            return Vec::new();
        };
        let Ok(resp) = resp.error_for_status() else {
            return Vec::new();
        };
        let Ok(body): Result<DebianScreenshotsResp, _> = resp.json().await else {
            return Vec::new();
        };
        let urls: Vec<String> = body
            .screenshots
            .into_iter()
            .filter_map(|s| s.large_image_url.or(s.screenshot_url))
            .take(SCREENSHOT_PREFETCH)
            .collect();
        self.fetch_screenshot_bytes(urls).await
    }

    /// Fetch screenshot bytes concurrently so the detail pane can
    /// display them without a second message round-trip. Read-through
    /// the disk cache so opening the same package twice doesn't
    /// re-download the same screenshots.
    async fn fetch_screenshot_bytes(&self, urls: Vec<String>) -> Vec<Screenshot> {
        let futs = urls.into_iter().map(|url| {
            let this = self.clone();
            async move {
                let bytes = this.fetch_bytes_cached(&url).await;
                Screenshot { bytes }
            }
        });
        futures_util::future::join_all(futs).await
    }

    /// Fetch raw bytes for an image URL. Bounded so a runaway response
    /// can't balloon memory — Flathub icons are tiny (a few KB).
    pub async fn fetch_bytes(&self, url: &str) -> Option<Vec<u8>> {
        const MAX_BYTES: usize = 4 * 1024 * 1024;
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        let bytes = resp.bytes().await.ok()?;
        if bytes.len() > MAX_BYTES {
            return None;
        }
        Some(bytes.to_vec())
    }

    async fn flathub_lookup(&self, guix_name: &str) -> Option<String> {
        if let Some(idx) = self.flathub_index.read().await.as_ref() {
            return idx.lookup(guix_name);
        }
        // Slow path: fetch the full ID list once.
        let url = format!("{FLATHUB_API}/appstream");
        let ids: Vec<String> = self
            .http
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        let idx = FlathubIndex::build(ids);
        let hit = idx.lookup(guix_name);
        *self.flathub_index.write().await = Some(idx);
        hit
    }
}

impl Default for MetadataClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_finds_single_match() {
        let idx = FlathubIndex::build(vec!["org.inkscape.Inkscape".into()]);
        assert_eq!(
            idx.lookup("inkscape"),
            Some("org.inkscape.Inkscape".to_string())
        );
    }

    #[test]
    fn index_prefers_gnome_over_github() {
        let idx = FlathubIndex::build(vec![
            "com.github.someone.calculator".into(),
            "org.gnome.Calculator".into(),
        ]);
        assert_eq!(
            idx.lookup("calculator"),
            Some("org.gnome.Calculator".to_string())
        );
    }

    #[test]
    fn index_misses_unknown() {
        let idx = FlathubIndex::build(vec!["org.gnome.Inkscape".into()]);
        assert!(idx.lookup("definitely-not-here").is_none());
    }

    #[test]
    fn metadata_is_empty_when_both_sources_empty() {
        let m = AppMetadata {
            flathub: None,
            debian_screenshots: Vec::new(),
        };
        assert!(m.is_empty());
    }

    #[test]
    fn safe_name_strips_unsafe_chars() {
        // Guix names with `@`, `+`, `_`, `/` etc. shouldn't be allowed
        // to escape the icons-by-name dir or break on weird filesystems.
        assert_eq!(MetadataClient::safe_name("gimp"), "gimp");
        assert_eq!(MetadataClient::safe_name("0ad"), "0ad");
        assert_eq!(MetadataClient::safe_name("python@3.9"), "python_3.9");
        assert_eq!(MetadataClient::safe_name("clang++"), "clang__");
        assert_eq!(MetadataClient::safe_name("../etc/passwd"), ".._etc_passwd");
    }

    #[test]
    fn url_key_is_deterministic_and_collision_free_for_distinct_urls() {
        let a = MetadataClient::url_key("https://example.org/a.png");
        let a2 = MetadataClient::url_key("https://example.org/a.png");
        let b = MetadataClient::url_key("https://example.org/b.png");
        assert_eq!(a, a2, "same URL must hash to the same key");
        assert_ne!(a, b);
        assert_eq!(a.len(), 16, "16 hex chars = 64 bits");
    }

    /// Round-trip read → miss → write → read → hit, plus expiry by
    /// backdating the file mtime past the TTL. Uses an isolated cache
    /// root so no XDG paths are touched.
    #[tokio::test]
    async fn cache_roundtrip_and_expiry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut client = MetadataClient::new();
        client.cache_root = Some(tmp.path().to_path_buf());

        assert!(client.read_cache("icons-by-name", "gimp").await.is_none());

        client
            .write_cache("icons-by-name", "gimp", b"PNG-bytes")
            .await;
        assert_eq!(
            client.read_cache("icons-by-name", "gimp").await.as_deref(),
            Some(&b"PNG-bytes"[..])
        );

        // Backdate past the TTL via std's File::set_times (stable since
        // 1.75) — avoids pulling in a separate filetime crate just for
        // this one assertion.
        let path = tmp.path().join("icons-by-name").join("gimp");
        let stale = std::time::SystemTime::now() - DISK_CACHE_TTL - Duration::from_secs(60);
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open cached file for mtime backdate");
        let times = std::fs::FileTimes::new().set_modified(stale);
        f.set_times(times).expect("set_times");
        assert!(client.read_cache("icons-by-name", "gimp").await.is_none());
    }

    /// Clearing a non-existent cache dir must succeed silently so users
    /// who've never enabled metadata don't see a phantom error.
    #[tokio::test]
    async fn clear_disk_cache_on_empty_root_is_ok() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut client = MetadataClient::new();
        client.cache_root = Some(tmp.path().to_path_buf());
        client.clear_disk_cache().await.expect("ok on empty root");
    }

    /// After clearing, a previously-cached entry should miss.
    #[tokio::test]
    async fn clear_disk_cache_drops_entries() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut client = MetadataClient::new();
        client.cache_root = Some(tmp.path().to_path_buf());
        client.write_cache("bytes", "abc", b"x").await;
        assert!(client.read_cache("bytes", "abc").await.is_some());
        client.clear_disk_cache().await.unwrap();
        assert!(client.read_cache("bytes", "abc").await.is_none());
    }
}
