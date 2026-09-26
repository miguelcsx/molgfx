//! Words of a statement's head, with their spans.

use crate::error::Span;

/// One whitespace-separated word.
#[derive(Clone, Copy, Debug)]
pub(super) struct Word<'a> {
    pub(super) text: &'a str,
    pub(super) span: Span,
}

/// Splits `source[span]` into words.
pub(super) fn words(source: &str, span: Span) -> Vec<Word<'_>> {
    let text = &source[span.start..span.end];
    let mut words = Vec::new();
    let mut start = None;
    for (offset, character) in text.char_indices() {
        match (character.is_whitespace(), start) {
            (true, Some(begin)) => {
                words.push(word(source, span.start + begin, span.start + offset));
                start = None;
            }
            (false, None) => start = Some(offset),
            _ => {}
        }
    }
    if let Some(begin) = start {
        words.push(word(source, span.start + begin, span.end));
    }
    words
}

fn word(source: &str, start: usize, end: usize) -> Word<'_> {
    Word {
        text: &source[start..end],
        span: Span::new(start, end),
    }
}

/// The position of the first comma outside quotes in `source[span]`.
pub(super) fn first_comma(source: &str, span: Span) -> Option<usize> {
    let bytes = &source.as_bytes()[span.start..span.end];
    let mut quote = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == b'\\' {
                    index += 1;
                } else if byte == open {
                    quote = None;
                }
            }
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b',' => return Some(span.start + index),
            None => {}
        }
        index += 1;
    }
    None
}

/// `span` with surrounding whitespace removed.
pub(super) fn trim(source: &str, span: Span) -> Span {
    let text = &source[span.start..span.end];
    let leading = text.len() - text.trim_start().len();
    Span::new(
        span.start + leading,
        span.start + leading + text.trim().len(),
    )
}
