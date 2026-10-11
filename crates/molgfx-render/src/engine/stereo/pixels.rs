//! Pixel layout conversion; source images remain complete physical eye views.
use super::{RenderError, StereoLayout};

pub(super) fn compose(
    left: &[u8],
    right: &[u8],
    width: u32,
    height: u32,
    layout: StereoLayout,
) -> Result<(u32, Vec<u8>), RenderError> {
    let count = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| RenderError::InvalidImageSize)?;
    if width == 0 || height == 0 || left.len() != count || right.len() != count {
        return Err(RenderError::InvalidStereo {
            reason: "invalid eye pixel buffer",
        });
    }
    if layout == StereoLayout::HdSideBySide && !width.is_multiple_of(2) {
        return Err(RenderError::InvalidStereo {
            reason: "compressed side-by-side requires an even eye width",
        });
    }
    let full_pair = matches!(layout, StereoLayout::SideBySide | StereoLayout::CrossEyed);
    let output_width = if full_pair {
        width.checked_mul(2).ok_or(RenderError::InvalidImageSize)?
    } else {
        width
    };
    let output_count = if full_pair {
        count.checked_mul(2).ok_or(RenderError::InvalidImageSize)?
    } else {
        count
    };
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(output_count)
        .map_err(|_| RenderError::InvalidImageSize)?;
    let width = usize::try_from(width).map_err(|_| RenderError::InvalidImageSize)?;
    let out_width = usize::try_from(output_width).map_err(|_| RenderError::InvalidImageSize)?;
    for row in 0..usize::try_from(height).map_err(|_| RenderError::InvalidImageSize)? {
        for column in 0..out_width {
            let source = (row * width + column % width) * 4;
            let a = &left[source..source + 4];
            let b = &right[source..source + 4];
            match layout {
                StereoLayout::SideBySide | StereoLayout::CrossEyed => {
                    let right_eye = (column >= width) ^ (layout == StereoLayout::CrossEyed);
                    pixels.extend_from_slice(if right_eye { b } else { a });
                }
                StereoLayout::HdSideBySide => {
                    let eye = if column >= width / 2 { right } else { left };
                    let source = (row * width + (column % (width / 2)) * 2) * 4;
                    for channel in 0..4 {
                        let average =
                            u8::midpoint(eye[source + channel], eye[source + channel + 4]);
                        pixels.push(average);
                    }
                }
                StereoLayout::RedCyan | StereoLayout::RedBlue | StereoLayout::RedGreen => {
                    pixels.extend([
                        a[0],
                        if layout == StereoLayout::RedBlue {
                            0
                        } else {
                            b[1]
                        },
                        if layout == StereoLayout::RedGreen {
                            0
                        } else {
                            b[2]
                        },
                        a[3].max(b[3]),
                    ]);
                }
                _ => {
                    let right_eye = match layout {
                        StereoLayout::Checkerboard => !(row + column).is_multiple_of(2),
                        StereoLayout::Columns => !column.is_multiple_of(2),
                        StereoLayout::Rows => !row.is_multiple_of(2),
                        StereoLayout::Right => true,
                        _ => false,
                    };
                    pixels.extend_from_slice(if right_eye { b } else { a });
                }
            }
        }
    }
    Ok((output_width, pixels))
}

#[cfg(test)]
#[path = "pixels_tests.rs"]
mod tests;
