//! Perceptual comparison of RGBA8 sRGB images for golden-image checks.

use crate::fallback::fallback;
use num_traits::ToPrimitive;

const WINDOW: usize = 11;
const SIGMA: f64 = 1.5;
const K1: f64 = 0.01;
const K2: f64 = 0.03;
/// Scale applied to per-pixel ΔE in diff images (ΔE 25.5 saturates).
const DIFF_GAIN: f64 = 10.0;
const D65_WHITE: [f64; 3] = [0.950_47, 1.0, 1.088_83];

/// Mean and 99th-percentile CIE76 colour difference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaEStats {
    /// Mean ΔE over all pixels.
    pub mean: f64,
    /// Nearest-rank 99th percentile ΔE.
    pub p99: f64,
}

fn float(value: usize) -> f64 {
    fallback(value.to_f64(), f64::MAX)
}

fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn pixels(a: &[u8], width: u32, height: u32) -> usize {
    let count = width as usize * height as usize;
    assert_eq!(a.len(), count * 4, "image buffer does not match its extent");
    count
}

fn luma(rgba: &[u8], count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let p = &rgba[i * 4..i * 4 + 3];
            0.2126 * linear(p[0]) + 0.7152 * linear(p[1]) + 0.0722 * linear(p[2])
        })
        .collect()
}

struct Stats {
    mean_a: f64,
    mean_b: f64,
    var_a: f64,
    var_b: f64,
    cov: f64,
}

fn kernel() -> [f64; WINDOW] {
    let centre = float(WINDOW / 2);
    let mut k = [0.0; WINDOW];
    for (i, v) in k.iter_mut().enumerate() {
        let d = float(i) - centre;
        *v = (-d * d / (2.0 * SIGMA * SIGMA)).exp();
    }
    let sum: f64 = k.iter().sum();
    for v in &mut k {
        *v /= sum;
    }
    k
}

/// Separable valid-region Gaussian filter: output is `(w-10) x (h-10)`.
fn filter(src: &[f64], width: usize, height: usize, k: &[f64; WINDOW]) -> Vec<f64> {
    let out_w = width - WINDOW + 1;
    let mut rows = vec![0.0; out_w * height];
    for y in 0..height {
        for x in 0..out_w {
            rows[y * out_w + x] = (0..WINDOW).map(|i| src[y * width + x + i] * k[i]).sum();
        }
    }
    let out_h = height - WINDOW + 1;
    let mut out = vec![0.0; out_w * out_h];
    for y in 0..out_h {
        for x in 0..out_w {
            out[y * out_w + x] = (0..WINDOW).map(|i| rows[(y + i) * out_w + x] * k[i]).sum();
        }
    }
    out
}

/// Mean structural similarity of linear Rec.709 luma over valid 11x11
/// Gaussian windows (σ = 1.5, K1 = 0.01, K2 = 0.03). Images smaller than
/// one window are compared as a single window of their global statistics.
#[must_use]
pub fn ssim_luma(left: &[u8], right: &[u8], width: u32, height: u32) -> f64 {
    let count = pixels(left, width, height);
    pixels(right, width, height);
    let (la, lb) = (luma(left, count), luma(right, count));
    let (cols, rows) = (width as usize, height as usize);
    let (c1, c2) = ((K1).powi(2), (K2).powi(2));
    let score = |s: Stats| {
        ((2.0 * s.mean_a * s.mean_b + c1) * (2.0 * s.cov + c2))
            / ((s.mean_a * s.mean_a + s.mean_b * s.mean_b + c1) * (s.var_a + s.var_b + c2))
    };
    if cols < WINDOW || rows < WINDOW {
        let n = float(count);
        let (ma, mb) = (la.iter().sum::<f64>() / n, lb.iter().sum::<f64>() / n);
        let va = la.iter().map(|v| (v - ma).powi(2)).sum::<f64>() / n;
        let vb = lb.iter().map(|v| (v - mb).powi(2)).sum::<f64>() / n;
        let cov = la
            .iter()
            .zip(&lb)
            .map(|(x, y)| (x - ma) * (y - mb))
            .sum::<f64>()
            / n;
        return score(Stats {
            mean_a: ma,
            mean_b: mb,
            var_a: va,
            var_b: vb,
            cov,
        });
    }
    let taps = kernel();
    let square = |v: &[f64]| v.iter().map(|x| x * x).collect::<Vec<_>>();
    let product = la.iter().zip(&lb).map(|(x, y)| x * y).collect::<Vec<_>>();
    let ma = filter(&la, cols, rows, &taps);
    let mb = filter(&lb, cols, rows, &taps);
    let saa = filter(&square(&la), cols, rows, &taps);
    let sbb = filter(&square(&lb), cols, rows, &taps);
    let sab = filter(&product, cols, rows, &taps);
    let total: f64 = (0..ma.len())
        .map(|i| {
            score(Stats {
                mean_a: ma[i],
                mean_b: mb[i],
                var_a: saa[i] - ma[i] * ma[i],
                var_b: sbb[i] - mb[i] * mb[i],
                cov: sab[i] - ma[i] * mb[i],
            })
        })
        .sum();
    total / float(ma.len())
}

fn lab(rgba: &[u8]) -> [f64; 3] {
    let (r, g, b) = (linear(rgba[0]), linear(rgba[1]), linear(rgba[2]));
    let xyz = [
        0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b,
        0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b,
        0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b,
    ];
    let f = |t: f64| {
        const DELTA: f64 = 6.0 / 29.0;
        if t > DELTA.powi(3) {
            t.cbrt()
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (
        f(xyz[0] / D65_WHITE[0]),
        f(xyz[1] / D65_WHITE[1]),
        f(xyz[2] / D65_WHITE[2]),
    );
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn pixel_delta(a: &[u8], b: &[u8], i: usize) -> f64 {
    let (la, lb) = (lab(&a[i * 4..]), lab(&b[i * 4..]));
    la.iter()
        .zip(&lb)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// CIE76 ΔE between two images (alpha ignored).
#[must_use]
pub fn delta_e76(a: &[u8], b: &[u8], width: u32, height: u32) -> DeltaEStats {
    let count = pixels(a, width, height);
    pixels(b, width, height);
    if count == 0 {
        return DeltaEStats {
            mean: 0.0,
            p99: 0.0,
        };
    }
    let mut deltas = (0..count).map(|i| pixel_delta(a, b, i)).collect::<Vec<_>>();
    let mean = deltas.iter().sum::<f64>() / float(count);
    deltas.sort_by(f64::total_cmp);
    let rank = fallback((float(count) * 0.99).ceil().to_usize(), count);
    DeltaEStats {
        mean,
        p99: deltas[rank.clamp(1, count) - 1],
    }
}

/// Grey PNG of per-pixel ΔE × 10, clamped to 255.
///
/// # Errors
/// Returns the PNG encoder error.
pub fn diff_png(
    a: &[u8],
    b: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, png::EncodingError> {
    let count = pixels(a, width, height);
    pixels(b, width, height);
    let grey = (0..count)
        .map(|i| {
            fallback(
                (pixel_delta(a, b, i) * DIFF_GAIN).clamp(0.0, 255.0).to_u8(),
                u8::MAX,
            )
        })
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&grey)?;
    Ok(out)
}

#[cfg(test)]
#[path = "image_compare_tests.rs"]
mod tests;
