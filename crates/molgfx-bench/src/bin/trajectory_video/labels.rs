//! Burned-in captions as an ASS subtitle file. ffmpeg's `ass` filter reads the file
//! once when the filter graph starts, so it must exist before the encoder is spawned.

use std::error::Error;
use std::fmt::Write as _;
use std::path::Path;

use crate::track::Tick;

/// Time units, largest first. `format_time` picks the first one the value reaches.
const UNITS: [(f64, &str); 6] = [
    (1.0, "s"),
    (1e-3, "ms"),
    (1e-6, "µs"),
    (1e-9, "ns"),
    (1e-12, "ps"),
    (1e-15, "fs"),
];

/// A physical time in the largest unit that keeps the number at or above one.
pub(crate) fn format_time(seconds: f64) -> String {
    match UNITS.iter().find(|(scale, _)| seconds.abs() >= *scale) {
        Some((scale, unit)) => format!("{:.2} {unit}", seconds / *scale),
        None => format!("{:.2} fs", seconds / 1e-15),
    }
}

/// Writes the captions: each track's name and physical time, the video's own clock,
/// and the footer.
///
/// # Errors
/// Returns an error when the file cannot be written.
pub(crate) fn write(
    path: &Path,
    size: (u32, u32),
    fps: u32,
    ticks: &[Tick],
    titles: &[String; 2],
    seconds: f64,
    footer: &str,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = size;
    let title = height / 28;
    let small = height / 50;
    let mut text = format!(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: {width}\n\
         PlayResY: {height}\n\
         ScaledBorderAndShadow: yes\n\
         \n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Title,Helvetica,{title},&H00FFFFFF,&H000000FF,&H00000000,&H99000000,0,0,0,0,100,100,0,0,1,2,1,7,40,40,30,1\n\
         Style: Clock,Helvetica,{title},&H00FFFFFF,&H000000FF,&H00000000,&H99000000,0,0,0,0,100,100,0,0,1,2,1,9,40,40,30,1\n\
         Style: Footer,Helvetica,{small},&H00DDDDDD,&H000000FF,&H00000000,&H99000000,0,0,0,0,100,100,0,0,1,2,1,2,40,40,30,1\n\
         \n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    );
    for tick in ticks {
        let start = stamp(u64::from(tick.index) * 100 / u64::from(fps));
        let end = stamp(u64::from(tick.index + 1) * 100 / u64::from(fps));
        let lines = format!(
            "{}  {}\\N{}  {}",
            titles[0],
            format_time(tick.sim[0]),
            titles[1],
            format_time(tick.sim[1])
        );
        writeln!(text, "Dialogue: 0,{start},{end},Title,,0,0,0,,{lines}")?;
        writeln!(
            text,
            "Dialogue: 0,{start},{end},Clock,,0,0,0,,video {:.2} s of {seconds:.2} s",
            tick.playback
        )?;
    }
    let frames = u64::try_from(ticks.len())?;
    writeln!(
        text,
        "Dialogue: 0,{},{},Footer,,0,0,0,,{footer}",
        stamp(0),
        stamp(frames * 100 / u64::from(fps))
    )?;
    std::fs::write(path, text)?;
    Ok(())
}

/// `H:MM:SS.cc` for a time in centiseconds, the precision ASS uses.
fn stamp(centiseconds: u64) -> String {
    let hours = centiseconds / 360_000;
    let rest = centiseconds % 360_000;
    format!(
        "{hours}:{:02}:{:02}.{:02}",
        rest / 6000,
        rest / 100 % 60,
        rest % 100
    )
}
