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

#[derive(Clone)]
pub struct MetadataClient {
    http: reqwest::Client,
    flathub_index: Arc<RwLock<Option<FlathubIndex>>>,
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
        Self {
            http,
            flathub_index: Arc::new(RwLock::new(None)),
        }
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
            self.fetch_bytes(url).await
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
    /// display them without a second message round-trip.
    async fn fetch_screenshot_bytes(&self, urls: Vec<String>) -> Vec<Screenshot> {
        let futs = urls.into_iter().map(|url| {
            let this = self.clone();
            async move {
                let bytes = this.fetch_bytes(&url).await;
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
}
