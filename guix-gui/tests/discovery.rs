//! Discovery filter tests — exercised against committed JSON fixtures so
//! they run offline. The live HTTP path is not tested here (that's a
//! network-dependent integration concern); these tests pin the filter
//! semantics: no-intro channels and packages from them never reach the
//! UI, even if the upstream catalog includes them.

use std::collections::HashSet;

use guix_gui::discovery::{
    filter_introduced_channels, filter_packages_by_introduced, DiscoveredChannel, DiscoveredPackage,
};
use libguix::parse_channels_list;

const CHANNELS_MIXED: &str = include_str!("fixtures/discovery/channels-mixed.json");
const PACKAGES_MIXED: &str = include_str!("fixtures/discovery/packages-mixed.json");

#[test]
fn channels_filter_drops_no_intro_entries() {
    let raw: Vec<DiscoveredChannel> = serde_json::from_str(CHANNELS_MIXED).expect("fixture parses");
    assert_eq!(raw.len(), 2, "fixture has two entries");

    let filtered = filter_introduced_channels(raw);
    assert_eq!(filtered.len(), 1, "only the introduced channel survives");
    assert_eq!(filtered[0].name, "glue");
}

#[test]
fn packages_filter_drops_packages_from_no_intro_channels() {
    let raw_channels: Vec<DiscoveredChannel> =
        serde_json::from_str(CHANNELS_MIXED).expect("channels fixture parses");
    let filtered_channels = filter_introduced_channels(raw_channels);
    let introduced_names: HashSet<String> =
        filtered_channels.iter().map(|c| c.name.clone()).collect();

    let raw_packages: Vec<DiscoveredPackage> =
        serde_json::from_str(PACKAGES_MIXED).expect("packages fixture parses");
    assert_eq!(raw_packages.len(), 2);

    let filtered_packages = filter_packages_by_introduced(raw_packages, &introduced_names);
    assert_eq!(filtered_packages.len(), 1);
    assert_eq!(filtered_packages[0].name, "glue-utility");
    assert_eq!(filtered_packages[0].channel, "glue");
}

/// The Discovery client's `to_channel` wraps the snippet as
/// `(list <snippet>)` before parsing. Asserting the round-trip equals a
/// direct `parse_channels_list` of the same wrapping pins the contract:
/// no per-channel parser drift between Discovery and libguix.
#[test]
fn subscription_snippet_parses_to_same_channel_as_libguix() {
    let raw: Vec<DiscoveredChannel> = serde_json::from_str(CHANNELS_MIXED).expect("fixture parses");
    let glue = raw
        .iter()
        .find(|c| c.name == "glue")
        .expect("glue entry present");

    let via_discovery = glue
        .to_channel()
        .expect("introduced snippet yields Channel");

    let wrapped = format!("(list {})", glue.subscription_snippet);
    let via_libguix = parse_channels_list(&wrapped)
        .expect("snippet parses as channels-list")
        .into_channels()
        .into_iter()
        .next()
        .expect("at least one channel");

    assert_eq!(via_discovery, via_libguix);
}
