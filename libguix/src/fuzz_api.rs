//! Parser entry points for the fuzz crate. Only built under `cfg(fuzzing)`.

use crate::parsers::lines::Splitter;
use crate::parsers::{progress, recutils, tsv};
use crate::repl::framer::Framer;
use crate::types::ProgressStream;

pub fn lines(data: &[u8]) {
    let mut splitter = Splitter::new();
    let mut frames = Vec::new();
    for chunk in data.chunks(7) {
        splitter.feed(chunk, &mut frames);
    }
    splitter.flush_eof(&mut frames);
    for frame in &frames {
        let _ = progress::parse_line(ProgressStream::Stdout, &frame.text, frame.redraw);
        let _ = progress::parse_line(ProgressStream::Stderr, &frame.text, frame.redraw);
    }
}

pub fn framer(input: &str) {
    let mut framer = Framer::new();
    let mut out = Vec::new();
    framer.feed(input, &mut out);
}

pub fn recutils(input: &str) {
    let _ = recutils::parse(input);
    let _ = recutils::parse_outputs(input);
}

pub fn tsv(input: &str) {
    let _ = tsv::parse_installed(input);
    let _ = tsv::parse_generations(input);
}
