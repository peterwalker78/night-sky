//! Quiet music under the sky: a handful of slow tracks, played through in a
//! different order each night, fading in with the sky and out with it.

use gtk::prelude::*;
use std::path::PathBuf;

/// Where the tracks are: beside the installed app, or in the source tree.
fn folder() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/app/share/night-sky/music")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(prefix) = exe.parent().and_then(|p| p.parent())
    {
        places.push(prefix.join("share/night-sky/music"));
    }
    places.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/music"
    )));
    places.into_iter().find(|p| p.is_dir())
}

pub struct Music {
    tracks: Vec<PathBuf>,
    next: usize,
    stream: Option<gtk::MediaFile>,
    volume: f64,
    pub quiet: bool,
}

/// How loud the music sits at its fullest: well under anything else.
pub const FULL: f64 = 0.55;

impl Music {
    /// The tracks in an order that depends on `seed`, so each night differs.
    pub fn new(seed: u64, quiet: bool) -> Music {
        let mut tracks: Vec<PathBuf> = folder()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "opus" || e == "ogg"))
            .collect();
        tracks.sort();
        if !tracks.is_empty() {
            let turn = (seed % tracks.len() as u64) as usize;
            tracks.rotate_left(turn);
        }
        Music {
            tracks,
            next: 0,
            stream: None,
            volume: 0.0,
            quiet,
        }
    }

    fn start_next(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        let path = &self.tracks[self.next % self.tracks.len()];
        self.next += 1;
        let stream = gtk::MediaFile::for_filename(path);
        stream.set_volume(self.volume);
        stream.play();
        self.stream = Some(stream);
    }

    /// Eases the volume towards `target` (0 to 1 of full) and moves on to the
    /// next track when one ends. Call it every frame.
    pub fn tick(&mut self, target: f64, dt: f64) {
        let target = if self.quiet {
            0.0
        } else {
            target.clamp(0.0, 1.0) * FULL
        };
        let k = 1.0 - (-dt / 1.6).exp();
        self.volume += (target - self.volume) * k;
        if self.volume < 0.002 && target == 0.0 {
            if let Some(s) = &self.stream
                && s.is_playing()
            {
                s.pause();
            }
            return;
        }
        match &self.stream {
            Some(s) if s.is_ended() || s.error().is_some() => {
                if let Some(e) = s.error() {
                    eprintln!("night-sky: a track wouldn't play: {e}");
                }
                self.start_next()
            }
            Some(s) => {
                if !s.is_playing() {
                    s.play();
                }
                s.set_volume(self.volume);
            }
            None => self.start_next(),
        }
    }
}
