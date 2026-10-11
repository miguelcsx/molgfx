//! The named colours.

use super::schemes;

/// Named colours, in sRGB.
///
/// A name resolves to one fixed sRGB triple, so a scene that says `color
/// firebrick` reads the same everywhere. Every name the reference engines also
/// define resolves to that engine's value — the `PyMOL` `reg_named_color` table
/// — so a script written for one engine draws the same colour here. The set is
/// deliberately smaller than either engine's full table, and the remaining
/// entries are the conventional CSS names a caller can also spell as `#rrggbb`.
pub const NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("blue", [0, 0, 255]),
    ("brown", [140, 90, 50]),
    ("cyan", [0, 255, 255]),
    ("deep_teal", [0, 120, 130]),
    ("firebrick", [178, 34, 34]),
    ("forest", [51, 153, 51]),
    ("gold", [255, 215, 0]),
    ("gray", [128, 128, 128]),
    ("green", [0, 255, 0]),
    ("grey", [128, 128, 128]),
    ("hot_pink", [255, 105, 180]),
    ("indigo", [75, 0, 130]),
    ("ivory", [255, 255, 240]),
    ("khaki", [240, 230, 140]),
    ("lime", [128, 255, 128]),
    ("magenta", [255, 0, 255]),
    ("marine", [0, 128, 255]),
    ("maroon", [128, 0, 0]),
    ("navy", [0, 0, 128]),
    ("olive", [196, 178, 0]),
    ("orange", [255, 128, 0]),
    ("orchid", [218, 112, 214]),
    ("pink", [255, 150, 190]),
    ("plum", [221, 160, 221]),
    ("purple", [191, 0, 191]),
    ("red", [255, 0, 0]),
    ("salmon", [255, 153, 153]),
    ("sea_green", [46, 139, 87]),
    ("sienna", [160, 82, 45]),
    ("sky_blue", [135, 206, 235]),
    ("slate", [128, 128, 255]),
    ("steel_blue", [70, 130, 180]),
    ("tan", [210, 180, 140]),
    ("teal", [0, 191, 191]),
    ("tomato", [255, 99, 71]),
    ("turquoise", [64, 224, 208]),
    ("violet", [238, 130, 238]),
    ("wheat", [245, 222, 179]),
    ("white", [255, 255, 255]),
    ("yellow", [255, 255, 0]),
];

/// The sRGB value of a named colour.
///
/// The element words the reference engines also expose as plain colour names
/// (`nitrogen`, `oxygen`, `carbon`, `hydrogen`, `sulfur`) resolve to the same
/// table the element scheme uses, so a script that says `color oxygen` and one
/// that colours by element agree. They are derived rather than repeated, so the
/// element table stays the single source of that colour.
#[must_use]
pub fn named_color(name: &str) -> Option<[u8; 3]> {
    if let Some(rgb) = NAMED_COLORS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, rgb)| *rgb)
    {
        return Some(rgb);
    }
    Some(molgfx_scene::color::element_rgb(element_atomic_number(
        name,
    )?))
}

/// Atomic number of the element word the reference engines name as a colour.
fn element_atomic_number(name: &str) -> Option<u8> {
    Some(match name {
        "hydrogen" => 1,
        "carbon" => 6,
        "nitrogen" => 7,
        "oxygen" => 8,
        "sulfur" => 16,
        _ => return None,
    })
}

/// Every word that names a colour: schemes, named colours, then the element
/// words the reference engines also accept.
pub fn color_words() -> impl Iterator<Item = &'static str> {
    schemes()
        .into_iter()
        .chain(NAMED_COLORS.iter().map(|(name, _)| *name))
        .chain(["hydrogen", "carbon", "nitrogen", "oxygen", "sulfur"])
}
