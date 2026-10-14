//! Planning `seek`, `play`, `pause` and `dilate`.
//!
//! Time is not part of the scene's patch stream, so these edit the session's
//! transport state only. Pacing is checked by lowering the authored track to
//! the scene's own clock mapping; nothing here knows what a track plays.

use super::{Planner, scene_error};
use crate::error::{CommandError, ErrorKind};
use crate::ir::Transport;
use molgfx_scene::timeline::{TrackPace, TrackSpec};

impl Planner<'_> {
    pub(super) fn transport(&mut self, command: &Transport) -> Result<(), CommandError> {
        match command {
            Transport::Seek { time } => {
                if time.seconds() < 0.0 {
                    return Err(CommandError::new(
                        ErrorKind::InvalidTime,
                        format!("cannot seek to {time}: playback time is never negative"),
                    ));
                }
                self.state.spec.transport.time = *time;
            }
            Transport::Play => self.state.playing = true,
            Transport::Pause => self.state.playing = false,
            Transport::Dilate { track, rate } => {
                let Some(authored) = self.timeline.tracks.get(track.as_str()) else {
                    return Err(CommandError::new(
                        ErrorKind::UnknownSymbol,
                        format!("there is no timeline track '{track}'"),
                    ));
                };
                if rate.seconds() <= 0.0 {
                    return Err(CommandError::new(
                        ErrorKind::InvalidTime,
                        format!("a dilation must be positive, not {rate}"),
                    ));
                }
                let paced = TrackSpec {
                    pace: TrackPace::Dilation {
                        per_playback_second: rate.seconds() / authored.unit.seconds(),
                    },
                    ..*authored
                };
                paced.warp().map_err(|error| scene_error(&error))?;
                let _ = self
                    .state
                    .spec
                    .transport
                    .dilations
                    .insert(track.clone(), *rate);
            }
        }
        Ok(())
    }
}
