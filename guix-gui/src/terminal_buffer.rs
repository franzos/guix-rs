//! vt100-backed terminal for the live-progress overlay. See NOTES.md
//! "Build output, `\r`-redraw, ANSI".

use libguix::ProgressEvent;

const ROWS: u16 = 40;
const COLS: u16 = 120;
const SCROLLBACK: usize = 20_000;

pub struct TerminalBuffer {
    parser: vt100::Parser,
}

impl TerminalBuffer {
    pub fn new() -> Self {
        Self {
            parser: vt100::Parser::new(ROWS, COLS, SCROLLBACK),
        }
    }

    /// vt100 doesn't do ONLCR — a bare `\n` doesn't reset the cursor
    /// column, so we emit `\r\n` for line breaks.
    pub fn feed_event(&mut self, evt: &ProgressEvent) {
        match evt {
            ProgressEvent::Line { text, redraw, .. } => {
                self.parser.process(text.as_bytes());
                self.parser.process(if *redraw { b"\r" } else { b"\r\n" });
            }
            ProgressEvent::ExitSummary {
                code,
                duration_secs,
            } => {
                let line = format!("\r\n--- exit {code} after {duration_secs:.2}s ---\r\n");
                self.parser.process(line.as_bytes());
            }
            other => {
                let mut s = format_event(other);
                s.push_str("\r\n");
                self.parser.process(s.as_bytes());
            }
        }
    }

    /// Always exactly [`ROWS`] entries — vt100 fills blanks.
    pub fn rows(&self) -> Vec<String> {
        self.parser
            .screen()
            .rows(0, COLS)
            .map(|r| r.trim_end().to_owned())
            .collect()
    }

    /// vt100 exposes scrollback via a movable offset — we must take
    /// `&mut self` to walk it. Restores offset before returning.
    pub fn scrollback(&mut self) -> Vec<String> {
        let prev = self.parser.screen().scrollback();
        self.parser
            .screen_mut()
            .set_scrollback(SCROLLBACK + usize::from(ROWS));
        let max = self.parser.screen().scrollback();

        let mut out: Vec<String> = Vec::new();
        let mut remaining = max;
        while remaining > 0 {
            self.parser.screen_mut().set_scrollback(remaining);
            let block: Vec<String> = self
                .parser
                .screen()
                .rows(0, COLS)
                .map(|r| r.trim_end().to_owned())
                .collect();
            let take = remaining.min(usize::from(ROWS));
            for row in block.into_iter().take(take) {
                out.push(row);
            }
            remaining = remaining.saturating_sub(usize::from(ROWS));
        }

        self.parser.screen_mut().set_scrollback(prev);
        out
    }

    pub fn clear(&mut self) {
        self.parser = vt100::Parser::new(ROWS, COLS, SCROLLBACK);
    }
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn format_event(e: &ProgressEvent) -> String {
    match e {
        ProgressEvent::Line { text, .. } => text.clone(),
        ProgressEvent::SubstituteLookup { url, percent } => {
            format!("substitute: {url} {percent:.1}%")
        }
        ProgressEvent::SubstituteDownload { item, .. } => format!("downloading {item}"),
        ProgressEvent::SubstituteDownloadDone { item, .. } => format!("downloaded {item}"),
        ProgressEvent::BuildStart { drv } => format!("building {drv}"),
        ProgressEvent::BuildPhase { drv, phase } => match drv {
            Some(d) => format!("[{d}] phase {phase}"),
            None => format!("phase {phase}"),
        },
        ProgressEvent::BuildDone { drv } => format!("built {drv}"),
        ProgressEvent::BuildFailed { drv, .. } => format!("FAILED {drv}"),
        ProgressEvent::WouldDownload { bytes, .. } => format!("would download {bytes} bytes"),
        ProgressEvent::WouldBuild { bytes, .. } => format!("would build {bytes} bytes"),
        ProgressEvent::StorePathListed { path } => format!("  - {path}"),
        ProgressEvent::PullComputingDerivation { system } => {
            format!("computing derivation for {system}")
        }
        ProgressEvent::DryRunHeader { text } => text.clone(),
        ProgressEvent::KnownBug(bug) => format!("known bug: {bug:?} ({})", bug.url()),
        ProgressEvent::ExitSummary {
            code,
            duration_secs,
        } => {
            format!("exit {code} after {duration_secs:.2}s")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libguix::ProgressStream;

    fn line(text: &str, redraw: bool) -> ProgressEvent {
        ProgressEvent::Line {
            stream: ProgressStream::Stdout,
            text: text.into(),
            redraw,
        }
    }

    fn non_blank(rows: &[String]) -> Vec<&str> {
        rows.iter()
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Redraws must collapse — pins the `\r` overwrite behaviour.
    #[test]
    fn long_redraw_sequence_collapses_to_one_row() {
        let mut tb = TerminalBuffer::new();
        for i in 1..=20 {
            tb.feed_event(&line(&format!("{i}%"), true));
        }
        tb.feed_event(&line("100%", false));

        let rows = tb.rows();
        let non_blank: Vec<&str> = non_blank(&rows);

        assert_eq!(non_blank, vec!["100%"], "rows were {rows:?}");
    }

    #[test]
    fn mixed_cr_lf_settles_lines_above_redraw_row() {
        let mut tb = TerminalBuffer::new();
        tb.feed_event(&line("alpha", false));
        tb.feed_event(&line("beta", false));
        tb.feed_event(&line("10%", true));
        tb.feed_event(&line("50%", true));
        tb.feed_event(&line("90%", true));

        let rows = tb.rows();
        let non_blank: Vec<&str> = non_blank(&rows);

        assert_eq!(non_blank, vec!["alpha", "beta", "90%"]);
    }

    #[test]
    fn guix_pull_like_pattern_does_not_flood() {
        let mut tb = TerminalBuffer::new();
        for i in 0_i16..50 {
            tb.feed_event(&line(&format!("{i}%"), true));
            if i % 10 == 0 {
                tb.feed_event(&ProgressEvent::SubstituteLookup {
                    url: format!("https://substitutes.example/{i}"),
                    percent: f32::from(i) * 2.0,
                });
            }
        }

        let rows = tb.rows();
        let count = non_blank(&rows).len();
        assert!(
            count <= 10,
            "non-blank row count flooded: {count}, rows = {rows:?}",
        );
        let last = non_blank(&rows).last().copied().unwrap_or("");
        assert_eq!(last, "49%");
    }

    #[test]
    fn clear_resets_state() {
        let mut tb = TerminalBuffer::new();
        tb.feed_event(&line("hello", false));
        tb.feed_event(&line("world", false));
        assert!(!non_blank(&tb.rows()).is_empty());

        tb.clear();
        assert!(
            non_blank(&tb.rows()).is_empty(),
            "rows after clear were {:?}",
            tb.rows()
        );
        assert!(tb.scrollback().is_empty());
    }

    #[test]
    fn exit_summary_renders_as_settled_line() {
        let mut tb = TerminalBuffer::new();
        tb.feed_event(&line("doing work", false));
        tb.feed_event(&ProgressEvent::ExitSummary {
            code: 0,
            duration_secs: 12.34,
        });

        let rows = tb.rows();
        let non_blank: Vec<&str> = non_blank(&rows);
        assert_eq!(non_blank, vec!["doing work", "--- exit 0 after 12.34s ---"]);
    }

    #[test]
    fn scrollback_captures_overflow_in_order() {
        let mut tb = TerminalBuffer::new();
        for i in 0..(ROWS as usize + 5) {
            tb.feed_event(&line(&format!("row {i}"), false));
        }
        let sb = tb.scrollback();
        assert!(!sb.is_empty());
        assert_eq!(sb.first().map(String::as_str), Some("row 0"));
    }
}
