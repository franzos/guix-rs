//! Line-level parsers for streamed `guix` write-op output. Best-effort —
//! unrecognised input falls through as [`ProgressEvent::Line`].

use crate::types::{KnownBug, ProgressEvent, ProgressStream};

pub(crate) fn parse_line(stream: ProgressStream, text: &str, redraw: bool) -> ProgressEvent {
    let trimmed = text.trim_end();

    if let Some(evt) = parse_known_bug(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_substitute(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_would(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_store_path_indent(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_build_lines(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_pull_lines(trimmed) {
        return evt;
    }
    if let Some(evt) = parse_dryrun_install_header(trimmed) {
        return evt;
    }

    ProgressEvent::Line {
        stream,
        text: text.to_owned(),
        redraw,
    }
}

fn parse_substitute(s: &str) -> Option<ProgressEvent> {
    // Splitter can leave doubled `substitute: substitute: ` prefixes.
    let rest = strip_repeated_prefix(s, "substitute:")?;
    let rest = rest.trim_start();

    if let Some(after) = rest.strip_prefix("looking for substitutes on ") {
        let after = after.trim_start_matches('\'');
        if let Some((url, tail)) = after.split_once("'...") {
            let tail = tail.trim();
            if let Some(pct_str) = tail.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    return Some(ProgressEvent::SubstituteLookup {
                        url: url.to_owned(),
                        percent: pct,
                    });
                }
            }
        }
    }

    if let Some(rest) = rest.strip_prefix("downloading ") {
        return Some(ProgressEvent::SubstituteDownload {
            item: rest.to_owned(),
            bytes_done: 0,
            bytes_total: None,
        });
    }

    if let Some(after) = rest.strip_prefix("updating substitutes from ") {
        let after = after.trim_start_matches('\'');
        if let Some((url, tail)) = after.split_once("'...") {
            let tail = tail.trim();
            if let Some(pct_str) = tail.strip_suffix('%') {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    return Some(ProgressEvent::SubstituteLookup {
                        url: url.to_owned(),
                        percent: pct,
                    });
                }
            }
        }
    }

    None
}

fn strip_repeated_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if !s.starts_with(prefix) {
        return None;
    }
    let mut cur = s;
    loop {
        match cur.strip_prefix(prefix) {
            Some(rest) => cur = rest.trim_start_matches(' '),
            None => return Some(cur),
        }
    }
}

fn parse_would(s: &str) -> Option<ProgressEvent> {
    let (size_str, kind_str) = s.split_once(" would be ")?;
    let bytes = parse_size_to_bytes(size_str)?;
    let kind = kind_str.trim_end_matches(':').trim();
    match kind {
        "downloaded" => Some(ProgressEvent::WouldDownload {
            bytes,
            items: Vec::new(),
        }),
        "built" => Some(ProgressEvent::WouldBuild {
            bytes,
            items: Vec::new(),
        }),
        _ => None,
    }
}

fn parse_size_to_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num_str, unit) = s.rsplit_once(' ')?;
    let num: f64 = num_str.trim().parse().ok()?;
    let mult: f64 = match unit.trim() {
        "B" | "bytes" => 1.0,
        "KB" | "kB" | "K" => 1_000.0,
        "MB" | "M" => 1_000_000.0,
        "GB" | "G" => 1_000_000_000.0,
        "TB" => 1_000_000_000_000.0,
        _ => return None,
    };
    Some((num * mult) as u64)
}

fn parse_store_path_indent(s: &str) -> Option<ProgressEvent> {
    let rest = s.strip_prefix("  ")?;
    if rest.starts_with("/gnu/store/") {
        return Some(ProgressEvent::StorePathListed {
            path: rest.to_owned(),
        });
    }
    None
}

/// Returns `None` if anything trails the `.drv` — caller falls through
/// to `Line` so trailing-status text isn't misclassified.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn extract_drv_exact(rest: &str) -> Option<&str> {
    let rest = rest.trim_end();
    if !rest.starts_with("/gnu/store/") {
        return None;
    }
    if !rest.ends_with(".drv") {
        return None;
    }
    if rest.split_whitespace().count() != 1 {
        return None;
    }
    Some(rest)
}

fn parse_build_lines(s: &str) -> Option<ProgressEvent> {
    if let Some(rest) = s.strip_prefix("building ") {
        if let Some(drv) = extract_drv_exact(rest) {
            return Some(ProgressEvent::BuildStart {
                drv: drv.to_owned(),
            });
        }
    }
    if let Some(rest) = s.strip_prefix("successfully built ") {
        if let Some(drv) = extract_drv_exact(rest) {
            return Some(ProgressEvent::BuildDone {
                drv: drv.to_owned(),
            });
        }
    }
    if let Some(rest) = s.strip_prefix("build of ") {
        if let Some(drv_part) = rest.strip_suffix(" failed") {
            if let Some(drv) = extract_drv_exact(drv_part) {
                // TODO: pair with the "View build log at …" follow-up line.
                return Some(ProgressEvent::BuildFailed {
                    drv: drv.to_owned(),
                    log_path: None,
                });
            }
        }
    }
    if let Some(rest) = s.strip_prefix("phase '") {
        if let Some((phase, _)) = rest.split_once('\'') {
            return Some(ProgressEvent::BuildPhase {
                drv: None,
                phase: phase.to_owned(),
            });
        }
    }
    None
}

fn parse_pull_lines(s: &str) -> Option<ProgressEvent> {
    if let Some(rest) = s.strip_prefix("Computing Guix derivation for '") {
        if let Some((system, _)) = rest.split_once('\'') {
            return Some(ProgressEvent::PullComputingDerivation {
                system: system.to_owned(),
            });
        }
    }
    None
}

fn parse_dryrun_install_header(s: &str) -> Option<ProgressEvent> {
    if s == "The following package would be installed:"
        || s == "The following packages would be installed:"
    {
        return Some(ProgressEvent::DryRunHeader { text: s.to_owned() });
    }
    None
}

/// Channel-shadow #74396. `await_completion` only escalates to
/// `KnownBug` on non-zero exit, so a healthy-pull false positive is benign.
fn parse_known_bug(s: &str) -> Option<ProgressEvent> {
    if is_channel_shadow_line(s) {
        return Some(ProgressEvent::KnownBug(KnownBug::ChannelShadow74396));
    }
    None
}

pub(crate) fn is_channel_shadow_line(s: &str) -> bool {
    let Some(idx) = s.find("no code for module ") else {
        return false;
    };
    let after = &s[idx + "no code for module ".len()..];
    let after = after.trim_end();
    if !after.starts_with('(') || !after.ends_with(')') {
        return false;
    }
    if after.len() <= 2 {
        return false;
    }
    let inner = &after[1..after.len() - 1];
    inner.chars().any(|c| !c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(stream: ProgressStream, text: &str) -> ProgressEvent {
        parse_line(stream, text, false)
    }

    #[test]
    fn substitute_lookup_basic() {
        let evt = parse(
            ProgressStream::Stderr,
            "substitute: looking for substitutes on 'https://ci.example'... 50.0%",
        );
        match evt {
            ProgressEvent::SubstituteLookup { url, percent } => {
                assert_eq!(url, "https://ci.example");
                assert!((percent - 50.0).abs() < f32::EPSILON);
            }
            other => panic!("expected SubstituteLookup, got {other:?}"),
        }
    }

    #[test]
    fn substitute_lookup_with_repeated_prefix() {
        let evt = parse(
            ProgressStream::Stderr,
            "substitute: substitute: looking for substitutes on 'https://x.example'... 100.0%",
        );
        assert!(matches!(evt, ProgressEvent::SubstituteLookup { .. }));
    }

    #[test]
    fn would_download_parses_bytes() {
        let evt = parse(ProgressStream::Stdout, "0.1 MB would be downloaded:");
        match evt {
            ProgressEvent::WouldDownload { bytes, .. } => {
                assert_eq!(bytes, 100_000);
            }
            other => panic!("expected WouldDownload, got {other:?}"),
        }
    }

    #[test]
    fn would_built_parses_bytes() {
        let evt = parse(ProgressStream::Stdout, "12.0 MB would be built:");
        match evt {
            ProgressEvent::WouldBuild { bytes, .. } => {
                assert_eq!(bytes, 12_000_000);
            }
            other => panic!("expected WouldBuild, got {other:?}"),
        }
    }

    #[test]
    fn store_path_indented_recognised() {
        let evt = parse(
            ProgressStream::Stdout,
            "  /gnu/store/ab584kfyc7pymc1cmdrkwzz3lwv86yf6-hello-2.12.3",
        );
        match evt {
            ProgressEvent::StorePathListed { path } => {
                assert!(path.starts_with("/gnu/store/"));
            }
            other => panic!("expected StorePathListed, got {other:?}"),
        }
    }

    #[test]
    fn build_start_done_failed() {
        let s = parse(ProgressStream::Stderr, "building /gnu/store/abc-foo.drv");
        assert!(matches!(s, ProgressEvent::BuildStart { .. }));

        let d = parse(
            ProgressStream::Stderr,
            "successfully built /gnu/store/abc-foo.drv",
        );
        assert!(matches!(d, ProgressEvent::BuildDone { .. }));

        let f = parse(
            ProgressStream::Stderr,
            "build of /gnu/store/abc-foo.drv failed",
        );
        match f {
            ProgressEvent::BuildFailed { drv, log_path } => {
                assert!(drv.ends_with("foo.drv"));
                assert!(log_path.is_none());
            }
            other => panic!("expected BuildFailed, got {other:?}"),
        }
    }

    #[test]
    fn build_with_trailing_garbage_falls_through() {
        let e = parse(
            ProgressStream::Stderr,
            "building /gnu/store/abc-foo.drv: cached",
        );
        match e {
            ProgressEvent::Line { text, redraw, .. } => {
                assert!(text.contains(": cached"));
                assert!(!redraw, "default-parse path uses redraw=false");
            }
            other => panic!("expected Line passthrough, got {other:?}"),
        }
    }

    #[test]
    fn pull_computing_derivation() {
        let e = parse(
            ProgressStream::Stderr,
            "Computing Guix derivation for 'x86_64-linux'...",
        );
        match e {
            ProgressEvent::PullComputingDerivation { system } => {
                assert_eq!(system, "x86_64-linux");
            }
            other => panic!("expected PullComputingDerivation, got {other:?}"),
        }
    }

    #[test]
    fn dryrun_header() {
        let e = parse(
            ProgressStream::Stdout,
            "The following package would be installed:",
        );
        assert!(matches!(e, ProgressEvent::DryRunHeader { .. }));
    }

    #[test]
    fn unknown_falls_through_to_line() {
        let e = parse(ProgressStream::Stderr, "this is something we do not parse");
        match e {
            ProgressEvent::Line { stream, text, .. } => {
                assert!(matches!(stream, ProgressStream::Stderr));
                assert!(text.contains("we do not parse"));
            }
            other => panic!("expected Line passthrough, got {other:?}"),
        }
    }

    #[test]
    fn unknown_line_carries_redraw_flag_through() {
        let e = parse_line(ProgressStream::Stdout, "5% [###]", true);
        match e {
            ProgressEvent::Line { redraw, text, .. } => {
                assert!(redraw, "redraw bit must reach Line");
                assert_eq!(text, "5% [###]");
            }
            other => panic!("expected Line, got {other:?}"),
        }
        let e = parse_line(ProgressStream::Stdout, "5% [###]", false);
        match e {
            ProgressEvent::Line { redraw, .. } => assert!(!redraw),
            other => panic!("expected Line, got {other:?}"),
        }
    }

    #[test]
    fn size_parser_variants() {
        assert_eq!(parse_size_to_bytes("0.1 MB"), Some(100_000));
        assert_eq!(parse_size_to_bytes("1.5 GB"), Some(1_500_000_000));
        assert_eq!(parse_size_to_bytes("128 KB"), Some(128_000));
        assert_eq!(parse_size_to_bytes("42 bytes"), Some(42));
        assert_eq!(parse_size_to_bytes("nope"), None);
    }

    #[test]
    fn channel_shadow_matches_bare_line() {
        let e = parse(
            ProgressStream::Stderr,
            "no code for module (some-channel mod)",
        );
        assert!(matches!(
            e,
            ProgressEvent::KnownBug(KnownBug::ChannelShadow74396)
        ));
    }

    #[test]
    fn channel_shadow_matches_prefixed_line() {
        let e = parse(
            ProgressStream::Stderr,
            "ice-9/boot-9.scm:1685:16: no code for module (px packages libguix)",
        );
        assert!(matches!(
            e,
            ProgressEvent::KnownBug(KnownBug::ChannelShadow74396)
        ));
    }

    #[test]
    fn channel_shadow_does_not_match_unrelated_lines() {
        let cases = [
            "no code for the module (foo bar)",
            "no code for module ()",
            "loaded module (some-channel mod)",
            "guix system: error: failed to build derivation",
            "",
        ];
        for c in cases {
            let e = parse(ProgressStream::Stderr, c);
            assert!(
                !matches!(e, ProgressEvent::KnownBug(_)),
                "false positive on {c:?}: {e:?}"
            );
        }
    }

    /// Whitespace-only parens must not match — same as `()` rejection.
    #[test]
    fn channel_shadow_rejects_whitespace_only_parens() {
        let cases = [
            "no code for module (   )",
            "no code for module (\t)",
            "no code for module ( )",
            "guix prefix no code for module (\t   \t)",
        ];
        for c in cases {
            assert!(
                !is_channel_shadow_line(c),
                "whitespace-only parens must not match: {c:?}"
            );
        }
        assert!(is_channel_shadow_line("no code for module (x)"));
    }
}
