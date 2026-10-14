use super::{Args, Encoder, Quality, Sink};

fn argv(list: &[&str]) -> impl Iterator<Item = String> {
    list.iter()
        .map(|item| (*item).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
}

const REQUIRED: [&str; 9] = [
    "--structure",
    "a.pdb",
    "--xtc",
    "a.xtc",
    "--psf",
    "a.psf",
    "--dcd",
    "a.dcd",
    "--out",
];

#[test]
fn parse_accepts_required_paths_and_defaults() {
    let mut list = REQUIRED.to_vec();
    list.push("out");
    let args = Args::parse(argv(&list)).expect("required paths parse");
    assert!((args.seconds - 12.0).abs() < f64::EPSILON);
    assert_eq!((args.fps, args.width, args.height), (60, 1920, 1080));
    assert_eq!(args.quality, Quality::Converged);
    assert_eq!(args.sink, Sink::Ffmpeg);
    assert!(args.labels);
}

#[test]
fn parse_selects_the_encoder_and_defaults_to_x264() {
    let mut list = REQUIRED.to_vec();
    list.push("out");
    assert_eq!(
        Args::parse(argv(&list)).expect("defaults").encoder,
        Encoder::X264
    );

    list.extend(["--encoder", "videotoolbox"]);
    let args = Args::parse(argv(&list)).expect("videotoolbox");
    assert_eq!(args.encoder, Encoder::VideoToolbox);
}

#[test]
fn parse_rejects_unknown_encoder() {
    let mut list = REQUIRED.to_vec();
    list.extend(["out", "--encoder", "nvenc"]);
    let error = Args::parse(argv(&list)).expect_err("unknown encoder");
    assert!(error.contains("unknown encoder nvenc"));
}

#[test]
fn parse_rejects_odd_frame_size() {
    let mut list = REQUIRED.to_vec();
    list.extend(["out", "--width", "1921"]);
    assert!(Args::parse(argv(&list)).is_err());
}

#[test]
fn parse_rejects_unknown_flag() {
    let mut list = REQUIRED.to_vec();
    list.extend(["out", "--speed", "2"]);
    let error = Args::parse(argv(&list)).expect_err("unknown flag");
    assert!(error.contains("unknown flag --speed"));
}
