//! Contact sheet: every recipe's image of a case side by side, in recipe order.
use super::catalog::Result;
use std::{fs::File, io, path::Path};

const GAP: usize = 8;
const MISSING_GREY: u8 = 128;

/// A decoded recipe image as tightly packed RGBA8.
pub(super) struct Tile {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Decodes a PNG of any 8-bit colour type to RGBA8.
pub(super) fn decode(path: &Path) -> Result<Tile> {
    let decoder = png::Decoder::new(io::BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| io::Error::other("png too large"))?
    ];
    let info = reader.next_frame(&mut buffer)?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(io::Error::other("sheet tiles must be 8-bit PNG").into());
    }
    let data = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], u8::MAX])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(io::Error::other("indexed PNG tiles are not supported").into());
        }
    };
    Ok(Tile {
        width: info.width as usize,
        height: info.height as usize,
        rgba,
    })
}

/// Lays `tiles` left to right with an 8 px white gap; `None` is a mid-grey tile of `extent`.
/// Returns the sheet as `(width, height, rgba)`.
pub(super) fn compose(tiles: &[Option<Tile>], extent: [u32; 2]) -> (usize, usize, Vec<u8>) {
    let fallback = (extent[0] as usize, extent[1] as usize);
    let size = |tile: &Option<Tile>| tile.as_ref().map_or(fallback, |t| (t.width, t.height));
    let height = tiles.iter().map(|t| size(t).1).fold(0, usize::max);
    let width =
        tiles.iter().map(|t| size(t).0).sum::<usize>() + GAP * tiles.len().saturating_sub(1);
    let mut out = vec![u8::MAX; width * height * 4];
    let mut left = 0;
    for tile in tiles {
        let (tile_w, tile_h) = size(tile);
        for row in 0..tile_h {
            let dst = (row * width + left) * 4;
            let line = &mut out[dst..dst + tile_w * 4];
            match tile {
                Some(t) => line.copy_from_slice(&t.rgba[row * tile_w * 4..(row + 1) * tile_w * 4]),
                None => line.fill(MISSING_GREY),
            }
        }
        left += tile_w + GAP;
    }
    (width, height, out)
}

/// Writes `directory/sheet.png` from each recipe's `image.png`; returns the recipes without one.
pub(super) fn write(directory: &Path, recipes: &[String], extent: [u32; 2]) -> Result<Vec<String>> {
    let mut missing = Vec::new();
    let tiles = recipes
        .iter()
        .map(|recipe| {
            let tile = decode(&directory.join(recipe).join("image.png")).ok();
            if tile.is_none() {
                missing.push(recipe.clone());
            }
            tile
        })
        .collect::<Vec<_>>();
    let (width, height, rgba) = compose(&tiles, extent);
    let mut encoder = png::Encoder::new(
        io::BufWriter::new(File::create(directory.join("sheet.png"))?),
        u32::try_from(width)?,
        u32::try_from(height)?,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(missing)
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
