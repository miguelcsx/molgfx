//! Writing forms and colours back as command text.

use super::color::ColorValue;
use super::form::Form;
use std::fmt;

/// Writes every control set on `form` as ` name=value`.
pub(crate) fn write_options(formatter: &mut fmt::Formatter<'_>, form: &Form) -> fmt::Result {
    for control in form.controls() {
        write!(formatter, " {control}")?;
    }
    Ok(())
}

/// A colour as the single word a `key=value` control accepts.
pub(crate) fn color_word(color: &ColorValue) -> String {
    color.to_string()
}
