//! Near-spelling suggestions for unknown words.

/// The candidate nearest to `word` by edit distance, when it is near enough to
/// be a plausible misspelling.
///
/// Words are compared case-insensitively. A candidate qualifies within one
/// edit for short words and within a third of the word's length otherwise;
/// ties go to the earlier candidate. Words longer than 64 bytes are never
/// matched, which bounds the work per candidate.
#[must_use]
pub fn suggest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    const LONGEST: usize = 64;
    if word.is_empty() || word.len() > LONGEST {
        return None;
    }
    let word = word.to_ascii_lowercase();
    let limit = (word.len() / 3).max(1);
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        if candidate.len() > LONGEST || candidate.eq_ignore_ascii_case(&word) {
            continue;
        }
        let distance = distance(word.as_bytes(), candidate.to_ascii_lowercase().as_bytes());
        if distance <= limit && best.is_none_or(|(nearest, _)| distance < nearest) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate.to_owned())
}

/// Levenshtein distance with adjacent transpositions counted as one edit.
fn distance(left: &[u8], right: &[u8]) -> usize {
    let width = right.len() + 1;
    let mut rows = vec![0usize; (left.len() + 1) * width];
    for (column, cell) in rows.iter_mut().take(width).enumerate() {
        *cell = column;
    }
    for row in 1..=left.len() {
        rows[row * width] = row;
        for column in 1..=right.len() {
            let substitution = usize::from(left[row - 1] != right[column - 1]);
            let mut best = (rows[(row - 1) * width + column] + 1)
                .min(rows[row * width + column - 1] + 1)
                .min(rows[(row - 1) * width + column - 1] + substitution);
            if row > 1
                && column > 1
                && left[row - 1] == right[column - 2]
                && left[row - 2] == right[column - 1]
            {
                best = best.min(rows[(row - 2) * width + column - 2] + 1);
            }
            rows[row * width + column] = best;
        }
    }
    rows[left.len() * width + right.len()]
}

#[cfg(test)]
#[path = "suggest_tests.rs"]
mod tests;
