//! Categorical-grid declarations and generational style edits.

use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::Command;
use molgfx_scene::SegmentationId;

pub(super) fn parse(source: &str, span: Span) -> Result<Option<Command>, CommandError> {
    let statement = &source[span.start..span.end];
    let (verb, payload) = statement
        .split_once(char::is_whitespace)
        .map_or((statement, ""), |(verb, rest)| (verb, rest.trim()));
    if verb != "segment" {
        return Ok(None);
    }
    let error = |message: String| CommandError::new(ErrorKind::Syntax, message).at(span);
    let (action, rest) = payload
        .split_once(char::is_whitespace)
        .map_or((payload, ""), |(action, rest)| (action, rest.trim()));
    if action == "style" {
        let (identity, json) = rest.split_once(char::is_whitespace).ok_or_else(|| {
            error("segment style requires INDEX:GENERATION and a JSON style array".to_owned())
        })?;
        let (index, generation) = identity
            .split_once(':')
            .ok_or_else(|| error("segmentation identity must be INDEX:GENERATION".to_owned()))?;
        let id = SegmentationId {
            index: index
                .parse()
                .map_err(|cause| error(format!("invalid segmentation index: {cause}")))?,
            generation: generation
                .parse()
                .map_err(|cause| error(format!("invalid segmentation generation: {cause}")))?,
        };
        let styles = serde_json::from_str(json.trim())
            .map_err(|cause| error(format!("invalid segment styles: {cause}")))?;
        return Ok(Some(Command::SegmentStyle { id, styles }));
    }
    let segmentation = serde_json::from_str(payload)
        .map_err(|cause| error(format!("invalid segmentation specification: {cause}")))?;
    Ok(Some(Command::Segment { segmentation }))
}

#[cfg(test)]
#[path = "segment_tests.rs"]
mod tests;
