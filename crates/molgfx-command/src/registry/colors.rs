//! The named colours.

use super::schemes;

/// Named colours, in sRGB.
///
/// A name resolves to one fixed sRGB triple, so a scene that says `color
/// firebrick` reads the same everywhere. The set covers the conventional
/// element and presentation names the reference engines use most; it is
/// deliberately smaller than either engine's full palette table, and every
/// entry here is a colour a caller can also spell as `#rrggbb`.
pub const NAMED_COLORS: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("blue", [51, 102, 255]),
    ("brown", [140, 90, 50]),
    ("cyan", [0, 200, 220]),
    ("deep_teal", [0, 120, 130]),
    ("firebrick", [178, 34, 34]),
    ("forest", [34, 139, 34]),
    ("gold", [255, 215, 0]),
    ("gray", [128, 128, 128]),
    ("green", [51, 190, 80]),
    ("grey", [128, 128, 128]),
    ("hot_pink", [255, 105, 180]),
    ("indigo", [75, 0, 130]),
    ("ivory", [255, 255, 240]),
    ("khaki", [240, 230, 140]),
    ("lime", [0, 255, 0]),
    ("magenta", [220, 40, 200]),
    ("marine", [0, 102, 153]),
    ("maroon", [128, 0, 0]),
    ("navy", [0, 0, 128]),
    ("olive", [128, 128, 0]),
    ("orange", [255, 140, 20]),
    ("orchid", [218, 112, 214]),
    ("pink", [255, 150, 190]),
    ("plum", [221, 160, 221]),
    ("purple", [140, 70, 190]),
    ("red", [230, 40, 40]),
    ("salmon", [250, 128, 114]),
    ("sea_green", [46, 139, 87]),
    ("sienna", [160, 82, 45]),
    ("sky_blue", [135, 206, 235]),
    ("slate", [110, 120, 200]),
    ("steel_blue", [70, 130, 180]),
    ("tan", [210, 180, 140]),
    ("teal", [0, 140, 140]),
    ("tomato", [255, 99, 71]),
    ("turquoise", [64, 224, 208]),
    ("violet", [238, 130, 238]),
    ("wheat", [245, 222, 179]),
    ("white", [255, 255, 255]),
    ("yellow", [250, 220, 40]),
];

/// The sRGB value of a named colour.
#[must_use]
pub fn named_color(name: &str) -> Option<[u8; 3]> {
    NAMED_COLORS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, rgb)| *rgb)
}

/// Every word that names a colour: schemes first, then named colours.
pub fn color_words() -> impl Iterator<Item = &'static str> {
    schemes()
        .into_iter()
        .chain(NAMED_COLORS.iter().map(|(name, _)| *name))
}
