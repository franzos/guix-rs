//! Paren-depth framer — splitting on `\n` is wrong because guix
//! pretty-prints values. See NOTES.md "Machine protocol framing".

#[derive(Debug, Default)]
pub(crate) struct Framer {
    depth: i32,
    in_string: bool,
    string_escape: bool,
    in_char_first: bool,
    in_char_name: bool,
    in_line_comment: bool,
    /// Non-nested approximation — Guile nests but the machine protocol
    /// never emits block comments.
    in_block_comment: bool,
    block_pending_pipe: bool,
    pending_hash: bool,
    buf: String,
    started: bool,
}

impl Framer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn feed(&mut self, chunk: &str, out: &mut Vec<String>) {
        for c in chunk.chars() {
            self.buf.push(c);

            if self.in_line_comment {
                if c == '\n' {
                    self.in_line_comment = false;
                }
                continue;
            }
            if self.in_block_comment {
                if self.block_pending_pipe && c == '#' {
                    self.in_block_comment = false;
                    self.block_pending_pipe = false;
                } else {
                    self.block_pending_pipe = c == '|';
                }
                continue;
            }
            if self.in_string {
                if self.string_escape {
                    self.string_escape = false;
                } else if c == '\\' {
                    self.string_escape = true;
                } else if c == '"' {
                    self.in_string = false;
                }
                continue;
            }
            if self.in_char_first {
                // `#\(`, `#\"`, etc. are one-char literals — always take.
                self.in_char_first = false;
                if c.is_alphabetic() {
                    self.in_char_name = true;
                }
                continue;
            }
            if self.in_char_name {
                if c.is_alphabetic() || c == '-' {
                    continue;
                }
                self.in_char_name = false;
            }

            if self.pending_hash {
                self.pending_hash = false;
                match c {
                    '|' => {
                        self.in_block_comment = true;
                        continue;
                    }
                    '\\' => {
                        self.in_char_first = true;
                        continue;
                    }
                    _ => {}
                }
            }

            match c {
                '"' => self.in_string = true,
                ';' => self.in_line_comment = true,
                '#' => {
                    self.pending_hash = true;
                }
                '(' => {
                    self.depth += 1;
                    self.started = true;
                }
                ')' => {
                    if self.depth > 0 {
                        self.depth -= 1;
                    }
                    if self.depth == 0 && self.started {
                        let frame = std::mem::take(&mut self.buf);
                        out.push(frame.trim().to_owned());
                        self.started = false;
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn at_boundary(&self) -> bool {
        self.depth == 0
            && !self.in_string
            && !self.in_char_first
            && !self.in_char_name
            && !self.in_block_comment
            && !self.pending_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_single_form() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(foo bar)\n", &mut out);
        assert_eq!(out, vec!["(foo bar)".to_string()]);
        assert!(f.at_boundary());
    }

    #[test]
    fn frames_across_chunks() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(foo ", &mut out);
        assert!(out.is_empty());
        f.feed("(bar)) ", &mut out);
        assert_eq!(out, vec!["(foo (bar))".to_string()]);
    }

    #[test]
    fn handles_multi_values_fixture() {
        let s = include_str!("../../tests/fixtures/repl-search.sexp");
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed(s, &mut out);
        assert_eq!(out.len(), 3);
        assert!(out[0].starts_with("(repl-version"));
        assert!(out[1].starts_with("(values (non-self-quoting"));
        assert!(out[2].starts_with("(values (value"));
    }

    #[test]
    fn ignores_parens_in_strings() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(values (value \"a ) b\"))\n", &mut out);
        assert_eq!(out, vec!["(values (value \"a ) b\"))".to_string()]);
    }

    #[test]
    fn ignores_parens_in_char_literals() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(foo #\\( bar)\n", &mut out);
        assert_eq!(out, vec!["(foo #\\( bar)".to_string()]);
    }

    #[test]
    fn named_char_literal() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(foo #\\newline bar)\n", &mut out);
        assert_eq!(out, vec!["(foo #\\newline bar)".to_string()]);
    }

    #[test]
    fn ignores_string_escape_quote() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(v \"x\\\"y\")\n", &mut out);
        assert_eq!(out, vec!["(v \"x\\\"y\")".to_string()]);
    }

    #[test]
    fn pretty_printed_value_across_lines() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(values\n  (value\n    (1 2 3)))\n", &mut out);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("(1 2 3)"));
    }

    /// Escape state must survive chunk boundary between `\` and `"`.
    #[test]
    fn fragment_inside_string_with_escaped_quote() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(v \"a\\", &mut out);
        assert!(out.is_empty());
        f.feed("\"b\")\n", &mut out);
        assert_eq!(out, vec!["(v \"a\\\"b\")".to_string()]);
    }

    /// Cross-chunk `#` lookahead — without it, `(` after `#` unbalances.
    #[test]
    fn fragment_inside_char_literal_between_hash_and_backslash() {
        let mut f = Framer::new();
        let mut out = Vec::new();
        f.feed("(foo #", &mut out);
        f.feed("\\( bar)\n", &mut out);
        assert_eq!(out, vec!["(foo #\\( bar)".to_string()]);
    }
}
