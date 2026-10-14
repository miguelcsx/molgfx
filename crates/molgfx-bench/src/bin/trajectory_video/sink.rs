//! Where rendered frames go: raw RGBA into ffmpeg, one PNG per frame, or nowhere.
//!
//! A writer thread takes frames from a bounded queue, so the renderer works on the
//! next frame while the previous one is piped or encoded. A full queue stalls the
//! renderer rather than growing memory.

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};

use molgfx::Image;

use crate::args::{Args, Encoder, Sink};

/// Frames that may wait for the writer before the renderer blocks. At 1080p each
/// frame is 8 MB, so this bounds the memory the overlap costs.
const QUEUE: usize = 2;

/// Where rendered frames go, opened once per run.
///
/// The writing happens on a thread of its own; `push` only queues a frame.
pub(crate) struct Output {
    frames: Option<SyncSender<(u32, Image)>>,
    writer: Option<JoinHandle<Result<(), String>>>,
}

/// The side that writes frames, owned by the writer thread.
enum Writer {
    /// ffmpeg reads raw RGBA from stdin and writes `video.mp4` in the output directory.
    Ffmpeg { child: Child, input: ChildStdin },
    /// One `frame_NNNNNN.png` per frame under `frames/`.
    Png { dir: PathBuf },
    /// Frames are rendered and dropped. This measures the renderer alone.
    Discard,
}

impl Output {
    /// Opens the sink that `args` selects and starts its writer thread.
    ///
    /// The ffmpeg child runs inside the output directory, so the caption file and the
    /// video are named by relative paths.
    ///
    /// # Errors
    /// Returns an error when ffmpeg cannot start or the frame directory cannot be created.
    pub(crate) fn open(args: &Args) -> Result<Self, Box<dyn Error>> {
        let mut sink = Writer::open(args)?;
        let (frames, queue) = mpsc::sync_channel(QUEUE);
        let writer = thread::spawn(move || {
            for (index, image) in queue {
                sink.write(&image, index)?;
            }
            sink.finish()
        });
        Ok(Self {
            frames: Some(frames),
            writer: Some(writer),
        })
    }

    /// Queues one rendered frame. Waits only while the writer is `QUEUE` frames behind.
    ///
    /// # Errors
    /// Returns the writer's own error when it has stopped.
    pub(crate) fn push(&mut self, image: Image, index: u32) -> Result<(), Box<dyn Error>> {
        let queued = self
            .frames
            .as_ref()
            .is_some_and(|frames| frames.send((index, image)).is_ok());
        if queued {
            return Ok(());
        }
        Err(self.stopped().into())
    }

    /// Closes the queue and reports why the writer stopped.
    fn stopped(&mut self) -> String {
        self.frames = None;
        match self.writer.take().map(JoinHandle::join) {
            Some(Ok(Err(message))) => message,
            Some(Err(_)) => "the frame writer panicked".to_owned(),
            _ => "the frame writer stopped".to_owned(),
        }
    }

    /// Closes the queue, waits for the writer to drain it, and for ffmpeg to finish.
    ///
    /// # Errors
    /// Returns the writer's error when a frame cannot be written or ffmpeg exits
    /// unsuccessfully.
    pub(crate) fn finish(mut self) -> Result<(), Box<dyn Error>> {
        self.frames = None;
        let Some(writer) = self.writer.take() else {
            return Ok(());
        };
        writer
            .join()
            .map_err(|_| "the frame writer panicked")?
            .map_err(Into::into)
    }
}

impl Writer {
    /// Opens the sink that `args` selects.
    ///
    /// # Errors
    /// Returns an error when ffmpeg cannot start or the frame directory cannot be created.
    fn open(args: &Args) -> Result<Self, Box<dyn Error>> {
        match args.sink {
            Sink::Ffmpeg => {
                let mut command = Command::new(&args.ffmpeg);
                command
                    .current_dir(&args.out)
                    .args(["-hide_banner", "-loglevel", "error", "-y"])
                    .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                    .arg(format!("{}x{}", args.width, args.height))
                    .args(["-r", &args.fps.to_string(), "-i", "-"]);
                if args.labels {
                    command.args(["-vf", "ass=labels.ass"]);
                }
                let codec: &[&str] = match args.encoder {
                    Encoder::X264 => &["-c:v", "libx264", "-preset", "medium", "-crf", "18"],
                    Encoder::VideoToolbox => &["-c:v", "h264_videotoolbox", "-b:v", "20M"],
                };
                command
                    .args(codec)
                    .args([
                        "-pix_fmt",
                        "yuv420p",
                        "-movflags",
                        "+faststart",
                        "video.mp4",
                    ])
                    .stdin(Stdio::piped());
                let mut child = command
                    .spawn()
                    .map_err(|error| format!("cannot start {}: {error}", args.ffmpeg))?;
                let input = child.stdin.take().ok_or("ffmpeg has no stdin")?;
                Ok(Self::Ffmpeg { child, input })
            }
            Sink::Png => {
                let dir = args.out.join("frames");
                std::fs::create_dir_all(&dir)?;
                Ok(Self::Png { dir })
            }
            Sink::Discard => Ok(Self::Discard),
        }
    }

    /// Writes one frame.
    ///
    /// # Errors
    /// Returns an error when ffmpeg has stopped reading or a PNG cannot be written.
    fn write(&mut self, image: &Image, index: u32) -> Result<(), String> {
        match self {
            Self::Ffmpeg { input, .. } => input.write_all(image.pixels()).map_err(|error| {
                format!("ffmpeg stopped reading frames ({error}); its message is above")
            }),
            Self::Png { dir } => image
                .save(dir.join(format!("frame_{index:06}.png")))
                .map_err(|error| error.to_string()),
            Self::Discard => Ok(()),
        }
    }

    /// Closes the stream and waits for ffmpeg to finish the video.
    ///
    /// # Errors
    /// Returns an error when ffmpeg exits unsuccessfully.
    fn finish(self) -> Result<(), String> {
        if let Self::Ffmpeg { mut child, input } = self {
            drop(input);
            let status = child.wait().map_err(|error| error.to_string())?;
            if !status.success() {
                return Err(format!("ffmpeg exited with {status}"));
            }
        }
        Ok(())
    }
}
