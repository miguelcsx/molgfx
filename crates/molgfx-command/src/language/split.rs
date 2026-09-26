//! Statement boundaries.

use crate::error::{CommandError, CommandErrors, ErrorKind, Span};

/// The trimmed, non-empty, non-comment statements of `source`.
///
/// `;` and line breaks end a statement unless they are inside a quoted
/// string; a statement whose first character is `#` runs to the end of its
/// line and is dropped.
pub(super) fn statements(source: &str) -> Result<Vec<Span>, CommandErrors> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut quote: Option<(u8, usize)> = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some((open, _)) = quote {
            if byte == b'\\' {
                index += 1;
            } else if byte == open {
                quote = None;
            } else if byte == b'\n' {
                break;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' => quote = Some((byte, index)),
            b'#' if source[start..index].trim().is_empty() => {
                let end = source[index..]
                    .find('\n')
                    .map_or(bytes.len(), |offset| index + offset);
                start = end;
                index = end;
                continue;
            }
            b';' | b'\n' => {
                push_trimmed(source, start, index, &mut spans);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if let Some((_, open)) = quote {
        return Err(CommandErrors::one(
            CommandError::new(ErrorKind::Lexical, "this quoted string is never closed")
                .at(Span::new(open, open + 1)),
        ));
    }
    push_trimmed(source, start, bytes.len(), &mut spans);
    Ok(spans)
}

fn push_trimmed(source: &str, start: usize, end: usize, spans: &mut Vec<Span>) {
    let text = &source[start..end];
    let leading = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        spans.push(Span::new(start + leading, start + leading + trimmed.len()));
    }
}
