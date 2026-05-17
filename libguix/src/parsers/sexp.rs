//! Parse `guix describe -f channels`.

use crate::error::GuixError;
use crate::types::Channel;

pub fn parse_channels(input: &str) -> Result<Vec<Channel>, GuixError> {
    let val = lexpr::from_str(input)
        .map_err(|e| GuixError::Parse(format!("describe channels: lexpr: {e}")))?;

    let mut iter = val
        .list_iter()
        .ok_or_else(|| GuixError::Parse("describe channels: top form is not a list".into()))?;

    let head = iter
        .next()
        .ok_or_else(|| GuixError::Parse("describe channels: empty top form".into()))?;
    if head.as_symbol() != Some("list") {
        return Err(GuixError::Parse(format!(
            "describe channels: expected `list` head, got {head:?}"
        )));
    }

    let mut out = Vec::new();
    for ch in iter {
        out.push(parse_channel(ch)?);
    }
    Ok(out)
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
}
