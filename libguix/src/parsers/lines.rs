//! Splits guix output on both `\n` and `\r`, tags frames with a
//! `redraw` bit, and strips CSI ANSI. See NOTES.md "Build output".

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub text: String,
    /// `true` if the frame ended with `\r` — upstream wants in-place overwrite.
    pub redraw: bool,
}

pub(crate) struct Splitter {
    buf: Vec<u8>,
}

impl Splitter {
    pub(crate) fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
        }
    }

    pub(crate) fn feed(&mut self, chunk: &[u8], out: &mut Vec<Frame>) {
        for &b in chunk {
            match b {
                b'\n' => self.flush_into(out, false),
                b'\r' => self.flush_into(out, true),
                _ => self.buf.push(b),
            }
        }
    }

    /// EOF tail has no terminator — mark non-redraw so a partial last
    /// frame still surfaces.
    pub(crate) fn flush_eof(&mut self, out: &mut Vec<Frame>) {
        self.flush_into(out, false);
    }

    fn flush_into(&mut self, out: &mut Vec<Frame>, redraw: bool) {
        if self.buf.is_empty() {
            return;
        }
        let raw = String::from_utf8_lossy(&self.buf).into_owned();
        self.buf.clear();
        let cleaned = strip_ansi(&raw);
        if !cleaned.is_empty() && !is_pure_chrome(&cleaned) {
            out.push(Frame {
                text: cleaned,
                redraw,
            });
        }
    }
}

/// Drops bar-only frames (Unicode block glyphs / ASCII `[#]`) — many
/// fonts can't render them. See NOTES.md.
fn is_pure_chrome(s: &str) -> bool {
    !s.chars().any(char::is_alphanumeric)
}

/// `ESC [ <params> <final-in-0x40..=0x7e>`. Unmatched sequences fall through.
pub(crate) fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() {
                let b = bytes[j];
                j += 1;
                if (0x40..=0x7e).contains(&b) {
                    break;
                }
            }
            i = j;
            continue;
        }
        if let Some(c) = s[i..].chars().next() {
            out.push(c);
            i += c.len_utf8();
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_all(input: &[u8]) -> Vec<Frame> {
        let mut s = Splitter::new();
        let mut out = Vec::new();
        s.feed(input, &mut out);
        s.flush_eof(&mut out);
        out
    }

    fn texts(frames: &[Frame]) -> Vec<&str> {
        frames.iter().map(|f| f.text.as_str()).collect()
    }

    fn frame(text: &str, redraw: bool) -> Frame {
        Frame {
            text: text.to_owned(),
            redraw,
        }
    }

    #[test]
    fn strip_ansi_clear_line() {
        assert_eq!(strip_ansi("hello\x1b[Kworld"), "helloworld");
    }

    #[test]
    fn strip_ansi_sgr() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn strip_ansi_no_escape_passthrough() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn splits_on_lf() {
        assert_eq!(texts(&split_all(b"a\nb\nc")), vec!["a", "b", "c"]);
    }

    #[test]
    fn splits_on_cr() {
        assert_eq!(texts(&split_all(b"a\rb\rc")), vec!["a", "b", "c"]);
    }

    #[test]
    fn crlf_does_not_double() {
        assert_eq!(texts(&split_all(b"a\r\nb\r\n")), vec!["a", "b"]);
    }

    #[test]
    fn drops_empty_frames() {
        assert!(split_all(b"\r\r\r").is_empty());
    }

    #[test]
    fn ansi_stripped_per_frame() {
        let out = split_all(b"\x1b[31mred\x1b[0m\nplain\n");
        assert_eq!(texts(&out), vec!["red", "plain"]);
    }

    #[test]
    fn mixed_terminators_tag_redraw_flag() {
        let frames = split_all(b"a\rb\nc\rd\n");
        assert_eq!(
            frames,
            vec![
                frame("a", true),
                frame("b", false),
                frame("c", true),
                frame("d", false),
            ]
        );
    }

    #[test]
    fn cr_redraw_sequence_then_lf_close() {
        let input = b"5%\r10%\r50%\r100%\n";
        let frames = split_all(input);
        assert_eq!(
            frames,
            vec![
                frame("5%", true),
                frame("10%", true),
                frame("50%", true),
                frame("100%", false),
            ]
        );
    }

    #[test]
    fn substitute_progress_splits_into_three_frames() {
        let input = b"substitute: \rsubstitute: \x1b[Klooking ... 0.0%\rsubstitute: \x1b[Klooking ... 50.0%\rsubstitute: \x1b[Klooking ... 100.0%\n";
        let frames = split_all(input);
        assert_eq!(frames.len(), 4);
        assert!(frames[1].text.ends_with("0.0%"));
        assert!(frames[2].text.ends_with("50.0%"));
        assert!(frames[3].text.ends_with("100.0%"));
        assert!(frames[0].redraw);
        assert!(frames[1].redraw);
        assert!(frames[2].redraw);
        assert!(!frames[3].redraw);
        assert!(!frames[3].text.contains('\x1b'));
        assert!(!frames[3].text.contains('['));
    }

    #[test]
    fn pure_chrome_unicode_bar_is_dropped() {
        let bar = "\u{2595}\u{2588}\u{2588}\u{2588}\u{2588}\u{258f}";
        let input = format!("{bar}\n").into_bytes();
        let frames = split_all(&input);
        assert!(frames.is_empty(), "bar-only frame should drop: {frames:?}");
    }

    #[test]
    fn pure_chrome_ascii_bar_is_dropped() {
        assert!(split_all(b"[####]\n").is_empty());
        assert!(split_all(b"[####] \n").is_empty());
    }

    #[test]
    fn bar_with_percent_is_kept() {
        let out = split_all(b"[####] 50%\n");
        assert_eq!(texts(&out), vec!["[####] 50%"]);
        assert!(!out[0].redraw);
    }

    #[test]
    fn status_text_is_kept_even_after_bar_drops() {
        let input = "\u{2595}\u{2588}\u{2588}\u{258f}\rlooking for substitutes...\n";
        let frames = split_all(input.as_bytes());
        assert_eq!(texts(&frames), vec!["looking for substitutes..."]);
    }

    #[test]
    fn partial_chunk_stitches_across_feeds() {
        let mut s = Splitter::new();
        let mut out = Vec::new();
        s.feed(b"hel", &mut out);
        assert!(out.is_empty());
        s.feed(b"lo\nworld", &mut out);
        assert_eq!(texts(&out), vec!["hello"]);
        s.flush_eof(&mut out);
        assert_eq!(texts(&out), vec!["hello", "world"]);
    }

    #[test]
    fn lone_cr_at_start_is_noop() {
        assert_eq!(texts(&split_all(b"\rabc\n")), vec!["abc"]);
    }

    /// Pins CSI reassembly across `feed()` boundaries.
    #[test]
    fn csi_sequence_spans_chunk_boundary() {
        let mut s = Splitter::new();
        let mut out = Vec::new();
        s.feed(b"hi\x1b", &mut out);
        assert!(out.is_empty(), "no terminator yet, no frame: {out:?}");
        s.feed(b"[Kbye\n", &mut out);
        assert_eq!(out.len(), 1);
        let frame = &out[0];
        assert!(!frame.text.contains('\x1b'), "ESC leaked: {frame:?}");
        assert!(!frame.text.contains('['), "`[` leaked: {frame:?}");
        assert_eq!(frame.text, "hibye");
        assert!(!frame.redraw);
    }

    #[test]
    #[allow(clippy::naive_bytecount)]
    fn fixture_dry_build_hello_yields_many_substitute_frames() {
        let raw = include_bytes!("../../tests/fixtures/dry-build-hello.txt");
        let frames = split_all(raw);
        let lf_count = raw.iter().filter(|&&b| b == b'\n').count();
        assert!(
            frames.len() > lf_count,
            "expected splitter to surface more frames ({} found) than `\\n` count ({lf_count})",
            frames.len()
        );
        for f in &frames {
            assert!(
                !f.text.contains('\x1b'),
                "ANSI not stripped in frame: {f:?}"
            );
        }
        let hundred = frames.iter().filter(|f| f.text.contains("100.0%")).count();
        assert!(
            hundred >= 2,
            "expected multiple `100.0%` frames, got {hundred}"
        );
    }
}
