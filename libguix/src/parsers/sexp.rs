//! Parse `guix describe -f channels` and `~/.config/guix/channels.scm`.

use crate::error::GuixError;
use crate::types::Channel;

/// Top-level channels-form shape — preserved across read → mutate → write
/// so the user's stylistic choice (enumerate everything vs layer on top of
/// `%default-channels`) survives a round-trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelsList {
    /// `(list (channel …) …)` — every channel enumerated explicitly.
    Explicit(Vec<Channel>),
    /// `(cons* … %default-channels)` or `(cons … %default-channels)` —
    /// custom channels layered on top of `%default-channels`. The vec
    /// holds the *custom* channels only; defaults are implicit.
    WithDefaults(Vec<Channel>),
}

impl ChannelsList {
    pub fn channels(&self) -> &[Channel] {
        match self {
            ChannelsList::Explicit(v) | ChannelsList::WithDefaults(v) => v,
        }
    }

    pub fn into_channels(self) -> Vec<Channel> {
        match self {
            ChannelsList::Explicit(v) | ChannelsList::WithDefaults(v) => v,
        }
    }
}

/// Back-compat shim for callers that just want the flat channel set
/// (e.g. `guix describe -f channels` consumers).
pub fn parse_channels(input: &str) -> Result<Vec<Channel>, GuixError> {
    Ok(parse_channels_list(input)?.into_channels())
}

/// Locates the first top-level form whose head is `list`, `cons*`, or
/// `cons` and parses it. Preamble forms like `(use-modules …)` are
/// skipped. Tolerates wrapper forms (`(channel-with-substitutes-available
/// (channel …) …)`) by walking one level in to find the inner channel.
pub fn parse_channels_list(input: &str) -> Result<ChannelsList, GuixError> {
    // `lexpr::from_str` only reads the first form; `Parser::value_iter`
    // walks every top-level form so we can skip preambles like
    // `(use-modules …)` and `;;`-comments (lexpr eats line comments).
    let mut parser = lexpr::Parser::from_str(input);
    for v in parser.value_iter() {
        let val = v.map_err(|e| GuixError::Parse(format!("channels: lexpr parse: {e}")))?;

        let head = match val
            .list_iter()
            .and_then(|mut it| it.next().and_then(|h| h.as_symbol().map(str::to_owned)))
        {
            Some(h) => h,
            None => continue,
        };

        match head.as_str() {
            "list" => return Ok(ChannelsList::Explicit(parse_channel_elements(&val)?)),
            "cons*" | "cons" => {
                return Ok(ChannelsList::WithDefaults(parse_cons_elements(
                    &val, &head,
                )?))
            }
            // Skip preamble forms (e.g. `(use-modules …)`, `(define …)`).
            _ => continue,
        }
    }

    Err(GuixError::Parse(
        "channels: no `list` / `cons*` / `cons` form found".into(),
    ))
}

/// Parses `(list (channel …) (channel-with-substitutes-available …) …)`.
/// Drops the head, iterates elements, walks one level into wrappers.
fn parse_channel_elements(val: &lexpr::Value) -> Result<Vec<Channel>, GuixError> {
    let mut iter = val
        .list_iter()
        .ok_or_else(|| GuixError::Parse("channels: list form is not a list".into()))?;
    // Drop head.
    let _ = iter.next();

    let mut out = Vec::new();
    for elt in iter {
        if let Some(ch) = parse_channel_or_wrapper(elt)? {
            out.push(ch);
        }
    }
    Ok(out)
}

/// Parses `(cons* (channel …) … %default-channels)` /
/// `(cons (channel …) %default-channels)`. The last element must be the
/// symbol `%default-channels`; everything before it is a channel/wrapper.
fn parse_cons_elements(val: &lexpr::Value, head_name: &str) -> Result<Vec<Channel>, GuixError> {
    let mut iter = val
        .list_iter()
        .ok_or_else(|| GuixError::Parse(format!("channels: {head_name} form is not a list")))?;
    let _ = iter.next();

    let elements: Vec<&lexpr::Value> = iter.collect();
    let Some((tail, head_elements)) = elements.split_last() else {
        return Err(GuixError::Parse(format!(
            "channels: {head_name} form has no elements"
        )));
    };

    if tail.as_symbol() != Some("%default-channels") {
        return Err(GuixError::Parse(format!(
            "channels: {head_name} form must end in `%default-channels`, got {tail:?}"
        )));
    }

    if head_name == "cons" && head_elements.len() != 1 {
        return Err(GuixError::Parse(format!(
            "channels: `cons` form must have exactly one channel + tail, got {} channels",
            head_elements.len()
        )));
    }

    let mut out = Vec::new();
    for elt in head_elements {
        if let Some(ch) = parse_channel_or_wrapper(elt)? {
            out.push(ch);
        }
    }
    Ok(out)
}

/// Returns `Ok(None)` only when the element is non-list garbage we want
/// to skip silently (shouldn't happen for well-formed files; defensive).
///
/// For wrapper forms (head other than `channel`), walks one level in
/// looking for an inner `(channel …)`. The wrapper itself is dropped on
/// the floor for the in-memory model — Phase 1a's writer doesn't have to
/// round-trip wrappers (that's 1b).
fn parse_channel_or_wrapper(val: &lexpr::Value) -> Result<Option<Channel>, GuixError> {
    let Some(mut it) = val.list_iter() else {
        return Ok(None);
    };
    let Some(head) = it.next() else {
        return Ok(None);
    };
    let head_sym = head.as_symbol();

    if head_sym == Some("channel") {
        return parse_channel(val).map(Some);
    }

    // Wrapper: scan the remaining elements for the first `(channel …)`.
    for inner in it {
        if let Some(mut ii) = inner.list_iter() {
            if let Some(ih) = ii.next() {
                if ih.as_symbol() == Some("channel") {
                    return parse_channel(inner).map(Some);
                }
            }
        }
    }
    Ok(None)
}

fn parse_channel(val: &lexpr::Value) -> Result<Channel, GuixError> {
    let mut iter = val
        .list_iter()
        .ok_or_else(|| GuixError::Parse("channel: not a list".into()))?;
    let head = iter
        .next()
        .ok_or_else(|| GuixError::Parse("channel: empty".into()))?;
    if head.as_symbol() != Some("channel") {
        return Err(GuixError::Parse(format!(
            "channel: expected `channel` head, got {head:?}"
        )));
    }

    let mut name = None;
    let mut url = None;
    let mut branch = None;
    let mut commit = None;
    let mut intro_commit = None;
    let mut intro_fpr = None;

    for field in iter {
        let mut fi = match field.list_iter() {
            Some(it) => it,
            None => continue,
        };
        let Some(key) = fi.next().and_then(|v| v.as_symbol().map(str::to_owned)) else {
            continue;
        };
        let value = fi.next();
        match (key.as_str(), value) {
            ("name", Some(v)) => name = sym_or_string(v),
            ("url", Some(v)) => url = v.as_str().map(str::to_owned),
            ("branch", Some(v)) => branch = v.as_str().map(str::to_owned),
            ("commit", Some(v)) => commit = v.as_str().map(str::to_owned),
            ("introduction", Some(v)) => {
                let (c, f) = parse_make_introduction(v);
                intro_commit = c;
                intro_fpr = f;
            }
            _ => {}
        }
    }

    Ok(Channel {
        name: name.ok_or_else(|| GuixError::Parse("channel: missing name".into()))?,
        url: url.ok_or_else(|| GuixError::Parse("channel: missing url".into()))?,
        branch,
        commit,
        introduction_commit: intro_commit,
        introduction_fingerprint: intro_fpr,
    })
}

/// `'foo` parses as `(quote foo)` — drill through.
fn sym_or_string(v: &lexpr::Value) -> Option<String> {
    if let Some(mut it) = v.list_iter() {
        if let Some(h) = it.next() {
            if h.as_symbol() == Some("quote") {
                if let Some(inner) = it.next() {
                    return inner.as_symbol().map(str::to_owned);
                }
            }
        }
    }
    if let Some(s) = v.as_symbol() {
        return Some(s.to_owned());
    }
    v.as_str().map(str::to_owned)
}

fn parse_make_introduction(v: &lexpr::Value) -> (Option<String>, Option<String>) {
    let mut commit = None;
    let mut fpr = None;
    let Some(mut it) = v.list_iter() else {
        return (commit, fpr);
    };
    if it.next().and_then(lexpr::Value::as_symbol) != Some("make-channel-introduction") {
        return (commit, fpr);
    }
    if let Some(c) = it.next() {
        commit = c.as_str().map(str::to_owned);
    }
    if let Some(fpr_form) = it.next() {
        if let Some(mut fi) = fpr_form.list_iter() {
            if fi.next().and_then(lexpr::Value::as_symbol) == Some("openpgp-fingerprint") {
                if let Some(s) = fi.next().and_then(lexpr::Value::as_str) {
                    fpr = Some(s.to_owned());
                }
            }
        }
    }
    (commit, fpr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channels_fixture() {
        let s = include_str!("../../tests/fixtures/describe-channels.scm");
        let chans = parse_channels(s).unwrap();
        assert_eq!(chans.len(), 3);
        assert_eq!(chans[0].name, "pantherx");
        assert_eq!(chans[0].url, "https://codeberg.org/gofranz/panther.git");
        assert_eq!(chans[0].branch.as_deref(), Some("master"));
        assert_eq!(
            chans[0].commit.as_deref(),
            Some("d7dd8f5d95ad0c8ba6ca0928cf6627e8ad39a31c")
        );
        assert_eq!(
            chans[0].introduction_commit.as_deref(),
            Some("54b4056ac571611892c743b65f4c47dc298c49da")
        );
        assert!(chans[0]
            .introduction_fingerprint
            .as_deref()
            .unwrap()
            .starts_with("A36A D41E"));
        assert_eq!(chans[1].name, "guix");
        assert_eq!(chans[2].name, "nonguix");
    }

    #[test]
    fn list_head_yields_explicit() {
        let s = include_str!("../../tests/fixtures/channels/list-three.scm");
        let cl = parse_channels_list(s).unwrap();
        match cl {
            ChannelsList::Explicit(v) => assert_eq!(v.len(), 3),
            other => panic!("expected Explicit, got {other:?}"),
        }
    }

    #[test]
    fn cons_star_head_yields_with_defaults() {
        let s = include_str!("../../tests/fixtures/channels/cons-star-defaults.scm");
        let cl = parse_channels_list(s).unwrap();
        match cl {
            ChannelsList::WithDefaults(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].name, "pantherx");
            }
            other => panic!("expected WithDefaults, got {other:?}"),
        }
    }

    #[test]
    fn cons_head_yields_with_defaults() {
        let s = include_str!("../../tests/fixtures/channels/cons-single.scm");
        let cl = parse_channels_list(s).unwrap();
        match cl {
            ChannelsList::WithDefaults(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].name, "pantherx");
            }
            other => panic!("expected WithDefaults, got {other:?}"),
        }
    }

    #[test]
    fn minimal_channel_only_name_and_url() {
        let s = include_str!("../../tests/fixtures/channels/minimal-channel.scm");
        let chans = parse_channels(s).unwrap();
        assert_eq!(chans.len(), 1);
        assert_eq!(chans[0].name, "guix-pod");
        assert!(chans[0].branch.is_none());
        assert!(chans[0].commit.is_none());
        assert!(chans[0].introduction_commit.is_none());
    }

    #[test]
    fn no_introduction_is_lenient() {
        let s = include_str!("../../tests/fixtures/channels/no-introduction.scm");
        let chans = parse_channels(s).unwrap();
        assert_eq!(chans.len(), 1);
        assert!(chans[0].introduction_commit.is_none());
    }

    #[test]
    fn preamble_use_modules_is_skipped() {
        let s = include_str!("../../tests/fixtures/channels/lock-with-use-modules.scm");
        let cl = parse_channels_list(s).unwrap();
        match cl {
            ChannelsList::Explicit(v) => assert_eq!(v.len(), 2),
            other => panic!("expected Explicit, got {other:?}"),
        }
    }

    #[test]
    fn wrapper_form_walks_one_level_in() {
        // Real-world: `(channel-with-substitutes-available (channel …) "url")`.
        let s = include_str!("../../tests/fixtures/channels/wrapped-and-commented.scm");
        let cl = parse_channels_list(s).unwrap();
        let chans = cl.into_channels();
        assert!(
            chans.iter().any(|c| c.name == "guix"),
            "expected wrapped `guix` channel to surface"
        );
        assert!(
            chans.iter().any(|c| c.name == "nonguix"),
            "expected `nonguix` channel"
        );
    }
}
