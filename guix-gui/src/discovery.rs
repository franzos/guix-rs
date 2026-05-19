//! Opt-in discovery client for `toys.whereis.social`.
//!
//! The catalog exposes two endpoints we care about:
//!
//! * `GET /api/channels` — every channel the indexer has crawled, with
//!   subscription snippets (Scheme `(channel …)` forms as strings).
//! * `GET /api/packages?search=&page=&limit=` — package search across
//!   every indexed channel, keyed by `channel` (name).
//!
//! **Critical filter.** Only channels that carry an `introduction`
//! (commit + fingerprint) reach the UI — unintroduced channels can't
//! be authenticated by `guix pull` and would be a footgun to surface.
//! The filter applies at this layer, never at the view layer. Package
//! hits whose `channel` field doesn't match an introduced channel are
//! dropped the same way.
//!
//! Everything here is gated by `Settings::discovery_enabled`. When that
//! toggle is off, no `Discovery` is constructed and no network call is
//! made — see `app.rs`.
//!
//! Lives in `guix-gui` deliberately: `libguix` stays pure (wraps `guix`,
//! no HTTP, no third-party catalogs).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use libguix::{parse_channels_list, Channel};
use serde::Deserialize;
use tokio::sync::RwLock;

pub const TOYS_API: &str = "https://toys.whereis.social/api";
const USER_AGENT: &str = concat!("guix-gui/", env!("CARGO_PKG_VERSION"));
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Channels turn over slowly on the catalog side — an hour is plenty
/// to keep a long-lived session honest without hammering the API.
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// Response shape for `/api/channels`. Fields mirror the API verbatim
/// (camelCase via serde rename). `subscriptionSnippet` is a Scheme
/// `(channel …)` form serialized as a string.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredChannel {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub packages_count: u32,
    #[serde(default)]
    pub services_count: u32,
    pub subscription_snippet: String,
}

impl DiscoveredChannel {
    /// Parses `subscriptionSnippet` via libguix's `parse_channels_list`.
    /// The snippet is a bare `(channel …)` form; wrap it as
    /// `(list <snippet>)` first so the parser's top-level-form locator
    /// has something it recognises.
    ///
    /// Returns `None` if parsing fails or the resulting channel lacks an
    /// introduction (commit + fingerprint). The caller treats this as
    /// "drop the entry"; we never let unintroduced channels reach the UI.
    pub fn to_channel(&self) -> Option<Channel> {
        let wrapped = format!("(list {})", self.subscription_snippet);
        let parsed = parse_channels_list(&wrapped).ok()?;
        let channels = parsed.into_channels();
        let ch = channels.into_iter().next()?;
        if ch.introduction_commit.is_none() || ch.introduction_fingerprint.is_none() {
            return None;
        }
        Some(ch)
    }
}

/// Response shape for `/api/packages`. The API returns plenty more
/// fields (origin, hash, build system, …) — we deserialize only the
/// ones the UI surfaces, leaning on `#[serde(default)]` so adjacent
/// schema drift doesn't break the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredPackage {
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub channel: String,
    #[serde(default)]
    pub synopsis: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub licenses: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub file: String,
}

#[derive(Debug)]
pub enum DiscoveryError {
    Http(reqwest::Error),
    Parse(String),
    /// A package hit's `channel` field references a channel that either
    /// isn't indexed or lacks an introduction. We never let such hits
    /// through, but the variant exists so callers can surface a
    /// meaningful message in the rare case it bubbles up directly.
    NoIntroductionForChannel(String),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::Http(e) => write!(f, "HTTP error: {e}"),
            DiscoveryError::Parse(s) => write!(f, "parse error: {s}"),
            DiscoveryError::NoIntroductionForChannel(name) => {
                write!(f, "no introduction for channel: {name}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DiscoveryError::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for DiscoveryError {
    fn from(e: reqwest::Error) -> Self {
        DiscoveryError::Http(e)
    }
}

/// Lazily-constructed catalog client. Holds a `reqwest::Client` and a
/// cached set of introduced-channel names so per-package cross-reference
/// stays cheap.
#[derive(Clone)]
pub struct Discovery {
    http: reqwest::Client,
    /// Filled by the first `channels()` call; reused by subsequent
    /// `search_packages()` calls so the same channel set drives both
    /// listing and cross-reference.
    cache: Arc<RwLock<Option<ChannelCache>>>,
}

#[derive(Clone)]
struct ChannelCache {
    channels: Vec<DiscoveredChannel>,
    introduced_names: HashSet<String>,
    fetched_at: Instant,
}

impl ChannelCache {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

impl Discovery {
    /// Returns `Err` if the HTTP client builder fails — caller decides
    /// how to surface that. A silent fallback to `Client::new()` would
    /// drop the user-agent and timeout, which is the worst combination
    /// for a constrained sandbox.
    pub fn new() -> Result<Self, DiscoveryError> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(HTTP_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Drops the cached channel set so the next `channels()` call hits
    /// the network. The TTL handles long-running sessions on its own.
    pub async fn invalidate(&self) {
        *self.cache.write().await = None;
    }

    /// Returns every channel from the API that carries a valid
    /// introduction (commit + fingerprint) in its `subscriptionSnippet`.
    /// Unintroduced entries are dropped at the client — the UI must
    /// never see them. Cached for [`CACHE_TTL`]; call [`invalidate`] to
    /// force a refetch sooner.
    pub async fn channels(&self) -> Result<Vec<DiscoveredChannel>, DiscoveryError> {
        if let Some(cache) = self.cache.read().await.as_ref() {
            if cache.is_fresh() {
                return Ok(cache.channels.clone());
            }
        }
        let url = format!("{TOYS_API}/channels");
        let raw: Vec<DiscoveredChannel> = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let filtered = filter_introduced_channels(raw);
        let introduced_names: HashSet<String> = filtered.iter().map(|c| c.name.clone()).collect();
        let cache = ChannelCache {
            channels: filtered.clone(),
            introduced_names,
            fetched_at: Instant::now(),
        };
        *self.cache.write().await = Some(cache);
        Ok(filtered)
    }

    /// Searches packages and cross-references each hit's `channel` field
    /// against the cached introduced-channel set, dropping hits whose
    /// providing channel isn't introduced (or isn't indexed at all).
    pub async fn search_packages(
        &self,
        q: &str,
        page: u32,
        limit: u32,
    ) -> Result<Vec<DiscoveredPackage>, DiscoveryError> {
        // Snapshot the introduced-channel set up front — a concurrent
        // TTL expiry or `invalidate()` between populate and read could
        // otherwise leave us holding `None`.
        let _ = self.channels().await?;
        let introduced_names: HashSet<String> = self
            .cache
            .read()
            .await
            .as_ref()
            .map(|c| c.introduced_names.clone())
            .unwrap_or_default();
        let url = format!("{TOYS_API}/packages");
        let raw: Vec<DiscoveredPackage> = self
            .http
            .get(&url)
            .query(&[
                ("search", q),
                ("page", &page.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(filter_packages_by_introduced(raw, &introduced_names))
    }
}

/// Filter helper exposed for tests — applies the introduction-required
/// rule against an already-deserialized channel list. The production
/// path uses this after `serde_json::from_*`; tests use it on fixtures.
pub fn filter_introduced_channels(channels: Vec<DiscoveredChannel>) -> Vec<DiscoveredChannel> {
    channels
        .into_iter()
        .filter(|c| c.to_channel().is_some())
        .collect()
}

/// Filter helper exposed for tests — drops packages whose `channel`
/// field doesn't appear in the introduced-channel name set.
pub fn filter_packages_by_introduced(
    packages: Vec<DiscoveredPackage>,
    introduced_names: &HashSet<String>,
) -> Vec<DiscoveredPackage> {
    packages
        .into_iter()
        .filter(|p| introduced_names.contains(&p.channel))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of this module: a snippet without an
    /// `(introduction …)` form must yield `None`, even when it parses
    /// successfully as a `Channel`.
    #[test]
    fn to_channel_returns_none_when_introduction_absent() {
        let dc = DiscoveredChannel {
            name: "wigust".into(),
            url: "https://notabug.org/wigust/guix-wigust".into(),
            branch: Some("master".into()),
            commit: Some("83e86a2891dd57f54fc3568d6a56581fabbb02d2".into()),
            packages_count: 157,
            services_count: 0,
            subscription_snippet:
                "(channel\n  (name 'wigust)\n  (url \"https://notabug.org/wigust/guix-wigust\")\n  (branch \"master\"))\n"
                    .into(),
        };
        assert!(dc.to_channel().is_none());
    }

    #[test]
    fn to_channel_yields_channel_when_introduction_present() {
        let dc = DiscoveredChannel {
            name: "glue".into(),
            url: "https://git.sr.ht/~puercopop/glue".into(),
            branch: Some("default".into()),
            commit: None,
            packages_count: 35,
            services_count: 3,
            subscription_snippet:
                "(channel\n  (name 'glue)\n  (url \"https://git.sr.ht/~puercopop/glue\")\n  (branch \"default\")\n  (introduction\n    (make-channel-introduction\n      \"ea330f23fbebdb623892c1345d9bf6a0c4861276\"\n      (openpgp-fingerprint\n        \"D5A3 4BC7 B37F 4017 D091  5CF5 EEF6 BD0D 5626 DB0F\"))))\n"
                    .into(),
        };
        let ch = dc.to_channel().expect("introduced snippet parses");
        assert_eq!(ch.name, "glue");
        assert_eq!(
            ch.introduction_commit.as_deref(),
            Some("ea330f23fbebdb623892c1345d9bf6a0c4861276")
        );
        assert!(ch.introduction_fingerprint.is_some());
    }
}
