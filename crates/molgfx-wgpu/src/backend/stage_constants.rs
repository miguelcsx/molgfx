//! The pipeline-overridable constants each entry point of a WGSL module uses.
//!
//! WebGPU lets a pipeline stage carry a value for any override its module
//! declares, and a render pipeline here builds both of its stages from one
//! list. Safari, however, fails a stage that carries a value for an override
//! its own entry point never reaches ("Vertex library failed creation"), which
//! left every render pipeline with a fragment-only override invalid there. So
//! each stage receives only the constants its entry point uses.
//!
//! The scan reads the source text, so the browser runtime carries no WGSL
//! front end. It follows references from an entry point through the
//! module-scope declarations, which covers every way WGSL lets an override be
//! reached: from a function body, from another override's initializer, from
//! the type of a module-scope variable, and from the entry point's own
//! attributes. A test checks it against naga's reachability for every unit of
//! the shader library.

use std::collections::{BTreeMap, BTreeSet};

/// A shader module and the overrides each of its entry points uses.
#[derive(Debug)]
pub struct WgpuShaderModule {
    pub(crate) raw: wgpu::ShaderModule,
    overrides: EntryOverrides,
}

impl WgpuShaderModule {
    pub(crate) fn new(raw: wgpu::ShaderModule, source: &str) -> Self {
        Self {
            raw,
            overrides: EntryOverrides::scan(source),
        }
    }

    /// The members of `constants` that `entry` uses.
    pub(crate) fn stage_constants<'a>(
        &self,
        entry: &str,
        constants: &[(&'a str, f64)],
    ) -> Vec<(&'a str, f64)> {
        self.overrides.filter(entry, constants)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Function,
    Override,
    Variable,
    /// A type, constant or other declaration, none of which can reach an
    /// override: WGSL allows override-sized arrays only in the type of a
    /// module-scope variable, and constants are fixed before any override.
    Other,
}

#[derive(Debug)]
struct Declaration {
    kind: Kind,
    references: Vec<String>,
}

/// Module-scope declarations and the names each one refers to.
#[derive(Debug, Default)]
pub(crate) struct EntryOverrides {
    declarations: BTreeMap<String, Declaration>,
}

impl EntryOverrides {
    pub(crate) fn scan(source: &str) -> Self {
        Self {
            declarations: declarations(&tokens(source)),
        }
    }

    /// The overrides `entry` reaches, or `None` when the module declares no
    /// function of that name.
    pub(crate) fn used_by(&self, entry: &str) -> Option<BTreeSet<&str>> {
        let start = self.declarations.get_key_value(entry)?;
        if start.1.kind != Kind::Function {
            return None;
        }
        let mut seen = BTreeSet::from([start.0.as_str()]);
        let mut pending = vec![start.1];
        while let Some(declaration) = pending.pop() {
            for reference in &declaration.references {
                if let Some((name, next)) = self.declarations.get_key_value(reference.as_str())
                    && seen.insert(name.as_str())
                {
                    pending.push(next);
                }
            }
        }
        Some(
            seen.into_iter()
                .filter(|name| {
                    self.declarations
                        .get(*name)
                        .is_some_and(|declaration| declaration.kind == Kind::Override)
                })
                .collect(),
        )
    }

    /// Keeps the constants `entry` uses. A name the module does not declare as
    /// an override is kept for the device to judge, and an entry point the
    /// scan cannot find keeps every constant: the scan narrows only what it
    /// can prove unused.
    pub(crate) fn filter<'a>(
        &self,
        entry: &str,
        constants: &[(&'a str, f64)],
    ) -> Vec<(&'a str, f64)> {
        let Some(used) = self.used_by(entry) else {
            return constants.to_vec();
        };
        constants
            .iter()
            .copied()
            .filter(|(name, _)| {
                used.contains(name)
                    || self
                        .declarations
                        .get(*name)
                        .is_none_or(|declaration| declaration.kind != Kind::Override)
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token<'a> {
    Identifier(&'a str),
    Punctuation(u8),
}

/// Identifiers and punctuation, with comments and numeric literals removed.
fn tokens(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let next = bytes.get(index + 1).copied();
        if byte == b'/' && next == Some(b'/') {
            while bytes.get(index).is_some_and(|byte| *byte != b'\n') {
                index += 1;
            }
        } else if byte == b'/' && next == Some(b'*') {
            // Block comments nest in WGSL.
            let mut depth = 0usize;
            while index < bytes.len() {
                match (bytes.get(index), bytes.get(index + 1)) {
                    (Some(b'/'), Some(b'*')) => {
                        depth += 1;
                        index += 2;
                    }
                    (Some(b'*'), Some(b'/')) => {
                        depth = depth.saturating_sub(1);
                        index += 2;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => index += 1,
                }
            }
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            while bytes
                .get(index)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                index += 1;
            }
            out.push(Token::Identifier(&source[start..index]));
        } else if byte.is_ascii_digit() {
            // A literal such as `1u`, `0x7fu` or `1.0e-4` names nothing.
            index += 1;
            while let Some(&byte) = bytes.get(index) {
                let exponent_sign = (byte == b'-' || byte == b'+')
                    && matches!(bytes.get(index - 1), Some(b'e' | b'E' | b'p' | b'P'));
                if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.' || exponent_sign {
                    index += 1;
                } else {
                    break;
                }
            }
        } else {
            if !byte.is_ascii_whitespace() {
                out.push(Token::Punctuation(byte));
            }
            index += 1;
        }
    }
    out
}

/// The index of the token closing the group opened at `open`.
fn closing(tokens: &[Token<'_>], open: usize, left: u8, right: u8) -> usize {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if *token == Token::Punctuation(left) {
            depth += 1;
        } else if *token == Token::Punctuation(right) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index;
            }
        }
    }
    tokens.len().saturating_sub(1)
}

/// The index of the next `;` at or after `from`.
fn semicolon(tokens: &[Token<'_>], from: usize) -> usize {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, token)| **token == Token::Punctuation(b';'))
        .map_or(tokens.len().saturating_sub(1), |(index, _)| index)
}

/// The identifiers in `range` that can name a module-scope declaration:
/// member names after `.` and attribute names after `@` cannot.
fn references(tokens: &[Token<'_>], start: usize, end: usize) -> Vec<String> {
    let mut out = Vec::new();
    for index in start..=end.min(tokens.len().saturating_sub(1)) {
        if let Some(Token::Identifier(name)) = tokens.get(index) {
            let previous = index.checked_sub(1).and_then(|index| tokens.get(index));
            if !matches!(previous, Some(Token::Punctuation(b'.' | b'@'))) {
                out.push((*name).to_owned());
            }
        }
    }
    out
}

fn declarations(tokens: &[Token<'_>]) -> BTreeMap<String, Declaration> {
    let mut out = BTreeMap::new();
    // Attribute arguments before a declaration belong to it: an entry point's
    // `@workgroup_size(...)` may name an override.
    let mut attributes = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        match *token {
            Token::Punctuation(b'@') => {
                index += 2;
                if tokens.get(index) == Some(&Token::Punctuation(b'(')) {
                    let end = closing(tokens, index, b'(', b')');
                    attributes.extend(references(tokens, index + 1, end));
                    index = end + 1;
                }
            }
            Token::Identifier("fn") => {
                let Some(Token::Identifier(name)) = tokens.get(index + 1) else {
                    index += 1;
                    continue;
                };
                let open = tokens
                    .iter()
                    .enumerate()
                    .skip(index + 2)
                    .find(|(_, token)| **token == Token::Punctuation(b'{'))
                    .map_or(tokens.len(), |(open, _)| open);
                let end = closing(tokens, open, b'{', b'}');
                let mut found = references(tokens, index + 2, end);
                found.append(&mut attributes);
                let _ = out.insert(
                    (*name).to_owned(),
                    Declaration {
                        kind: Kind::Function,
                        references: found,
                    },
                );
                index = end + 1;
            }
            Token::Identifier("struct") => {
                let name = tokens.get(index + 1).copied();
                let open = index + 2;
                let end = closing(tokens, open, b'{', b'}');
                if let Some(Token::Identifier(name)) = name {
                    let _ = out.insert(
                        name.to_owned(),
                        Declaration {
                            kind: Kind::Other,
                            references: Vec::new(),
                        },
                    );
                }
                attributes.clear();
                index = end + 1;
            }
            Token::Identifier(keyword @ ("override" | "var" | "const" | "alias")) => {
                let mut name_at = index + 1;
                if tokens.get(name_at) == Some(&Token::Punctuation(b'<')) {
                    name_at = closing(tokens, name_at, b'<', b'>') + 1;
                }
                let end = semicolon(tokens, name_at);
                if let Some(Token::Identifier(name)) = tokens.get(name_at) {
                    let kind = match keyword {
                        "override" => Kind::Override,
                        "var" => Kind::Variable,
                        _ => Kind::Other,
                    };
                    let found = if kind == Kind::Other {
                        Vec::new()
                    } else {
                        references(tokens, name_at + 1, end)
                    };
                    let _ = out.insert(
                        (*name).to_owned(),
                        Declaration {
                            kind,
                            references: found,
                        },
                    );
                }
                attributes.clear();
                index = end + 1;
            }
            Token::Identifier("enable" | "requires" | "diagnostic" | "const_assert") => {
                attributes.clear();
                index = semicolon(tokens, index) + 1;
            }
            _ => index += 1,
        }
    }
    out
}

#[cfg(test)]
#[path = "stage_constants_tests.rs"]
mod tests;
