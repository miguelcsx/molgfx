//! Fixed-size content identities for runtime geometry sources.
use super::{ControlValue, OptionKind};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// SHA-256 content identity of a separately bound geometry source.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash([u8; 32]);

impl ControlValue for ContentHash {
    const KIND: OptionKind = OptionKind::ContentHash;
    type Native = molgfx_scene::DataSource;
    fn parse(text: &str) -> Result<Self, String> {
        if text.len() != 64 {
            return Err("a content identity requires 64 hexadecimal digits".to_owned());
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let upper = hex(pair[0])
                .ok_or_else(|| "a content identity requires hexadecimal digits".to_owned())?;
            let lower = hex(pair[1])
                .ok_or_else(|| "a content identity requires hexadecimal digits".to_owned())?;
            bytes[index] = upper * 16 + lower;
        }
        Ok(Self(bytes))
    }
    fn native(self) -> Self::Native {
        molgfx_scene::DataSource::new(self.word())
    }
    fn word(self) -> String {
        let mut text = String::with_capacity(64);
        for byte in self.0 {
            let _ = write!(text, "{byte:02x}");
        }
        text
    }
}

impl TryFrom<String> for ContentHash {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}
impl From<ContentHash> for String {
    fn from(value: ContentHash) -> Self {
        value.word()
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
