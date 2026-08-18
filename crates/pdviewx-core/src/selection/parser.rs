//! String parser for the typed selection IR.

use super::{PropertyComparison, Select};
use crate::CoreError;
use pdviewx_math::Vec3;

pub(super) fn parse(source: &str) -> Result<Select, CoreError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, cursor: 0 };
    let selection = parser.parse_or()?;
    if parser.cursor != parser.tokens.len() {
        return Err(error("unexpected token after selection"));
    }
    Ok(selection)
}

#[derive(Clone, Copy)]
enum Token<'a> {
    Word(&'a str),
    Number { value: f32, source: &'a str },
    Operator(PropertyComparison),
    Open,
    Close,
}

fn tokenize(source: &str) -> Result<Vec<Token<'_>>, CoreError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        match bytes[cursor] {
            b'(' => {
                tokens.push(Token::Open);
                cursor += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                cursor += 1;
            }
            b'<' | b'>' | b'=' => {
                let first = bytes[cursor];
                let has_equal = bytes.get(cursor + 1) == Some(&b'=');
                let comparison = match (first, has_equal) {
                    (b'<', false) => PropertyComparison::Less,
                    (b'<', true) => PropertyComparison::LessOrEqual,
                    (b'>', false) => PropertyComparison::Greater,
                    (b'>', true) => PropertyComparison::GreaterOrEqual,
                    (b'=', _) => PropertyComparison::Equal,
                    _ => return Err(error("invalid property comparison")),
                };
                cursor += if has_equal { 2 } else { 1 };
                tokens.push(Token::Operator(comparison));
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                tokens.push(Token::Word(&source[start..cursor]));
            }
            byte if byte.is_ascii_digit() || matches!(byte, b'.' | b'+' | b'-') => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_digit()
                        || matches!(bytes[cursor], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    cursor += 1;
                }
                let value = source[start..cursor]
                    .parse::<f32>()
                    .map_err(|_| error("malformed numeric selection value"))?;
                tokens.push(Token::Number {
                    value,
                    source: &source[start..cursor],
                });
            }
            _ => return Err(error("unsupported character in selection")),
        }
    }
    if tokens.is_empty() {
        return Err(error("selection string is empty"));
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    cursor: usize,
}

impl Parser<'_> {
    fn parse_or(&mut self) -> Result<Select, CoreError> {
        let mut expression = self.parse_and()?;
        while self.take_word("or") {
            expression = expression.or(self.parse_and()?);
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Select, CoreError> {
        let mut expression = self.parse_unary()?;
        while self.take_word("and") {
            expression = expression.and(self.parse_unary()?);
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Select, CoreError> {
        if self.take_word("not") {
            return Ok(self.parse_unary()?.negate());
        }
        if self.take_word("within") {
            let distance = self.take_number()?;
            self.require_word("of")?;
            return Select::within(distance, self.parse_unary()?);
        }
        if self.take_word("residues_within") {
            let distance = self.take_number()?;
            self.require_word("of")?;
            return Select::residues_within(distance, self.parse_unary()?);
        }
        if self.take_word("beyond") {
            let distance = self.take_number()?;
            self.require_word("of")?;
            return Select::beyond(distance, self.parse_unary()?);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Select, CoreError> {
        if matches!(self.tokens.get(self.cursor), Some(Token::Open)) {
            self.cursor += 1;
            let expression = self.parse_or()?;
            if !matches!(self.tokens.get(self.cursor), Some(Token::Close)) {
                return Err(error("selection group is missing a closing parenthesis"));
            }
            self.cursor += 1;
            return Ok(expression);
        }
        let Some(Token::Word(word)) = self.tokens.get(self.cursor).copied() else {
            return Err(error("expected a selection keyword"));
        };
        self.cursor += 1;
        if word.eq_ignore_ascii_case("all") {
            Ok(Select::all())
        } else if word.eq_ignore_ascii_case("none") {
            Ok(Select::none())
        } else if word.eq_ignore_ascii_case("polymer") {
            Ok(Select::polymer())
        } else if word.eq_ignore_ascii_case("protein") {
            Ok(Select::protein())
        } else if word.eq_ignore_ascii_case("nucleic") {
            Ok(Select::nucleic())
        } else if word.eq_ignore_ascii_case("ligand")
            || word.eq_ignore_ascii_case("ligands")
            || word.eq_ignore_ascii_case("nonpolymer")
        {
            Ok(Select::ligands())
        } else if word.eq_ignore_ascii_case("water") {
            Ok(Select::water())
        } else if word.eq_ignore_ascii_case("branched") {
            Ok(Select::branched())
        } else if word.eq_ignore_ascii_case("chain") {
            Select::chain(self.take_text()?)
        } else if word.eq_ignore_ascii_case("resname") || word.eq_ignore_ascii_case("residue_name")
        {
            Select::residue_name(self.take_text()?)
        } else if word.eq_ignore_ascii_case("atom") || word.eq_ignore_ascii_case("name") {
            Select::atom_name(self.take_text()?)
        } else if word.eq_ignore_ascii_case("resid") || word.eq_ignore_ascii_case("residue") {
            Ok(Select::residue(self.take_integer()?))
        } else if word.eq_ignore_ascii_case("element") {
            Select::element(self.take_text()?)
        } else if word.eq_ignore_ascii_case("helix") {
            Ok(Select::helix())
        } else if word.eq_ignore_ascii_case("sheet") || word.eq_ignore_ascii_case("strand") {
            Ok(Select::sheet())
        } else if word.eq_ignore_ascii_case("coil") {
            Ok(Select::coil())
        } else if word.eq_ignore_ascii_case("hydrogen") {
            Ok(Select::hydrogen())
        } else if word.eq_ignore_ascii_case("heavy") {
            Ok(Select::heavy())
        } else if word.eq_ignore_ascii_case("backbone") {
            Ok(Select::backbone())
        } else if word.eq_ignore_ascii_case("terminus") {
            Ok(Select::terminus())
        } else if word.eq_ignore_ascii_case("b_factor") || word.eq_ignore_ascii_case("bfactor") {
            Select::b_factor(self.take_comparison()?, self.take_number()?)
        } else if word.eq_ignore_ascii_case("occupancy") {
            Select::occupancy(self.take_comparison()?, self.take_number()?)
        } else if word.eq_ignore_ascii_case("in_sphere") {
            let center = self.take_vec3()?;
            Select::in_sphere(center, self.take_number()?)
        } else if word.eq_ignore_ascii_case("in_box") {
            let min = self.take_vec3()?;
            Select::in_box(min, self.take_vec3()?)
        } else {
            Err(error("unknown selection keyword"))
        }
    }

    fn take_word(&mut self, expected: &str) -> bool {
        let Some(Token::Word(word)) = self.tokens.get(self.cursor) else {
            return false;
        };
        if !word.eq_ignore_ascii_case(expected) {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn require_word(&mut self, expected: &str) -> Result<(), CoreError> {
        self.take_word(expected)
            .then_some(())
            .ok_or_else(|| error("spatial selection requires `of`"))
    }

    fn take_number(&mut self) -> Result<f32, CoreError> {
        let Some(Token::Number { value, .. }) = self.tokens.get(self.cursor).copied() else {
            return Err(error("selection predicate requires a number"));
        };
        self.cursor += 1;
        Ok(value)
    }

    fn take_integer(&mut self) -> Result<i32, CoreError> {
        let Some(Token::Number { source, .. }) = self.tokens.get(self.cursor).copied() else {
            return Err(error("residue number must be an integer"));
        };
        self.cursor += 1;
        source
            .parse::<i32>()
            .map_err(|_| error("residue number must be an integer"))
    }

    fn take_text(&mut self) -> Result<&str, CoreError> {
        let Some(Token::Word(word)) = self.tokens.get(self.cursor).copied() else {
            return Err(error("selection predicate requires a value"));
        };
        self.cursor += 1;
        Ok(word)
    }

    fn take_comparison(&mut self) -> Result<PropertyComparison, CoreError> {
        let Some(Token::Operator(comparison)) = self.tokens.get(self.cursor).copied() else {
            return Err(error("property predicate requires a comparison"));
        };
        self.cursor += 1;
        Ok(comparison)
    }

    fn take_vec3(&mut self) -> Result<Vec3, CoreError> {
        Ok(Vec3::new(
            self.take_number()?,
            self.take_number()?,
            self.take_number()?,
        ))
    }
}

const fn error(reason: &'static str) -> CoreError {
    CoreError::InvalidSelection { reason }
}
