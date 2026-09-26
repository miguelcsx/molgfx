//! Completion candidates for a partly written statement.

use super::Session;
use crate::ir::{FormKind, Name};
use crate::registry;
use serde::Serialize;

/// One completion candidate.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Completion {
    /// The text that replaces the word being written.
    pub text: String,
    /// What kind of thing it is: `verb`, `form`, `control`, `color`,
    /// `selection`, `layer`, `structure`, `keyword` or `query`.
    pub kind: &'static str,
    /// A short description.
    pub detail: String,
}

/// The most candidates returned for one request.
const LIMIT: usize = 50;

/// Words a `MolFrame` query commonly starts with, offered after a comma.
const QUERY_WORDS: &[&str] = &[
    "all",
    "protein",
    "nucleic",
    "ligand",
    "water",
    "ion",
    "hetero",
    "backbone",
    "sidechain",
    "polymer",
    "hydrogen",
    "heavy",
    "chain",
    "resname",
    "resid",
    "name",
    "element",
    "within",
    "byres",
    "not",
    "and",
    "or",
];

impl Session {
    /// Candidates for the word that ends at `cursor` in `text`.
    ///
    /// Only the statement containing the cursor is considered, and at most
    /// fifty candidates are returned, each starting with the word written so
    /// far.
    #[must_use]
    pub fn completions(&self, text: &str, cursor: usize) -> Vec<Completion> {
        let mut cursor = cursor.min(text.len());
        while !text.is_char_boundary(cursor) {
            cursor -= 1;
        }
        let before = &text[..cursor];
        let statement = before
            .rfind([';', '\n'])
            .map_or(before, |separator| &before[separator + 1..])
            .trim_start();
        let word_start = statement
            .rfind(|character: char| character.is_whitespace() || character == ',')
            .map_or(0, |index| index + 1);
        let word = &statement[word_start..];
        let head = &statement[..word_start];
        let verb = head.split_whitespace().next();
        let after_comma = head.contains(',');
        let mut out = Vec::new();
        if let Some(name) = word.strip_prefix('$') {
            self.names(&mut out, name, "$", "selection");
        } else if let Some(name) = word.strip_prefix('@') {
            self.names(&mut out, name, "@", "layer");
        } else {
            match verb {
                None => vocabulary(&mut out, word, registry::verb_names(), "verb", |name| {
                    registry::VERBS
                        .iter()
                        .find(|verb| verb.name == name)
                        .map_or_else(String::new, |verb| verb.synopsis.to_owned())
                }),
                Some("focus") => self.query(&mut out, word),
                Some(_) if after_comma => self.query(&mut out, word),
                Some("show") => show(&mut out, head, word),
                Some("color") if head.split_whitespace().count() == 1 => {
                    vocabulary(&mut out, word, registry::color_words(), "color", |_| {
                        String::new()
                    });
                }
                Some("hide" | "remove" | "opacity") => self.names(&mut out, word, "@", "layer"),
                Some("unselect") => self.names(&mut out, word, "", "selection"),
                Some(_) if head.split_whitespace().last() == Some("in") => {
                    self.names(&mut out, word, "", "structure");
                }
                Some(_) => {}
            }
        }
        out.truncate(LIMIT);
        out
    }

    fn names(&self, out: &mut Vec<Completion>, prefix: &str, sigil: &str, kind: &'static str) {
        let spec = &self.state.spec;
        let entries: Vec<(&Name, String)> = match kind {
            "selection" => spec
                .selections
                .iter()
                .map(|(name, query)| (name, query.source().to_owned()))
                .collect(),
            "layer" => spec
                .layers
                .iter()
                .map(|(name, layer)| {
                    (
                        name,
                        format!("{} of {}", layer.form.kind().name(), layer.target),
                    )
                })
                .collect(),
            _ => spec
                .structures
                .iter()
                .map(|(name, id)| (name, format!("structure {}", id.get())))
                .collect(),
        };
        for (name, detail) in entries {
            if name.as_str().starts_with(prefix) {
                out.push(Completion {
                    text: format!("{sigil}{name}"),
                    kind,
                    detail,
                });
            }
        }
    }

    fn query(&self, out: &mut Vec<Completion>, word: &str) {
        self.names(out, word, "$", "selection");
        self.names(out, word, "@", "layer");
        vocabulary(out, word, QUERY_WORDS.iter().copied(), "query", |_| {
            String::new()
        });
    }
}

fn show(out: &mut Vec<Completion>, head: &str, word: &str) {
    let words: Vec<&str> = head.split_whitespace().collect();
    match words.as_slice() {
        [_] => vocabulary(out, word, registry::form_names(), "form", |_| String::new()),
        [_, form, ..] => {
            let Some(kind) = FormKind::from_name(form) else {
                return;
            };
            if let Some((key, value)) = word.split_once('=') {
                if key == "color" {
                    for color in registry::color_words().filter(|color| color.starts_with(value)) {
                        out.push(Completion {
                            text: format!("color={color}"),
                            kind: "color",
                            detail: String::new(),
                        });
                    }
                }
                return;
            }
            for option in kind.options() {
                if option.name.starts_with(word) {
                    out.push(Completion {
                        text: format!("{}=", option.name),
                        kind: "control",
                        detail: option.summary.to_owned(),
                    });
                }
            }
            vocabulary(
                out,
                word,
                ["color=", "opacity=", "duplicate", "as", "in"].into_iter(),
                "keyword",
                |_| String::new(),
            );
        }
        [] => {}
    }
}

fn vocabulary<'a>(
    out: &mut Vec<Completion>,
    prefix: &str,
    words: impl Iterator<Item = &'a str>,
    kind: &'static str,
    detail: impl Fn(&str) -> String,
) {
    for word in words {
        if word.starts_with(prefix) {
            out.push(Completion {
                text: word.to_owned(),
                kind,
                detail: detail(word),
            });
        }
    }
}
