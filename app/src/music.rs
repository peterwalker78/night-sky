//! Quiet music under the sky, in a few styles. Each style is a folder of
//! slow tracks with a `pack.toml` naming it; the tracks play through in a
//! different order each night, fading in with the sky and out with it.
//! Changing style fades the track playing out before the next begins.

use gtk::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Where the styles are: beside the installed app, or in the source tree.
fn folder() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/app/share/westering/music")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(prefix) = exe.parent().and_then(|p| p.parent())
    {
        places.push(prefix.join("share/westering/music"));
    }
    places.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/music"
    )));
    places.into_iter().find(|p| p.is_dir())
}

#[derive(Deserialize)]
struct PackFile {
    name: String,
    #[serde(default)]
    order: i64,
}

/// One style of music.
pub struct Pack {
    /// The folder's name, kept in the settings.
    pub id: String,
    pub name: String,
    order: i64,
    tracks: Vec<PathBuf>,
}

fn load(dir: &Path, seed: u64) -> Option<Pack> {
    let about: PackFile =
        toml::from_str(&std::fs::read_to_string(dir.join("pack.toml")).ok()?).ok()?;
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "opus" || e == "ogg"))
        .collect();
    if tracks.is_empty() {
        return None;
    }
    tracks.sort();
    let turn = (seed % tracks.len() as u64) as usize;
    tracks.rotate_left(turn);
    Some(Pack {
        id: dir.file_name()?.to_string_lossy().into_owned(),
        name: about.name,
        order: about.order,
        tracks,
    })
}

pub struct Music {
    packs: Vec<Pack>,
    /// The style playing, and the one asked for when that's different.
    pack: usize,
    wanted: usize,
    next: usize,
    stream: Option<gtk::MediaFile>,
    volume: f64,
    pub quiet: bool,
}

/// How loud the music sits at its fullest. The tracks are levelled gently
/// (-20 LUFS), so this can be the whole of it.
pub const FULL: f64 = 1.0;

impl Music {
    /// Every style, each in an order that depends on `seed`, so each night
    /// differs; `style` is the one to start with.
    pub fn new(seed: u64, quiet: bool, style: Option<&str>) -> Music {
        let mut packs: Vec<Pack> = folder()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| load(&e.path(), seed))
            .collect();
        packs.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(&b.name)));
        let pack = style
            .and_then(|s| packs.iter().position(|p| p.id == s))
            .unwrap_or(0);
        Music {
            packs,
            pack,
            wanted: pack,
            next: 0,
            stream: None,
            volume: 0.0,
            quiet,
        }
    }

    /// The styles, by id and name, in their order.
    pub fn styles(&self) -> Vec<(String, String)> {
        self.packs
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect()
    }

    /// Asks for a style by its id; it takes over once the track playing
    /// has faded.
    pub fn want(&mut self, style: Option<&str>) {
        if let Some(i) = style.and_then(|s| self.packs.iter().position(|p| p.id == s)) {
            self.wanted = i;
        }
    }

    /// The style and title of the track playing: "02-moon-unit" is "Moon
    /// unit".
    pub fn playing(&self) -> Option<(String, String)> {
        if self.quiet || self.volume < 0.01 {
            return None;
        }
        let pack = self.packs.get(self.pack)?;
        let path = pack
            .tracks
            .get((self.next + pack.tracks.len() - 1) % pack.tracks.len())?;
        let stem = path.file_stem()?.to_str()?;
        let words = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-');
        let mut title = words.replace('-', " ");
        if let Some(first) = title.get(..1) {
            title = first.to_uppercase() + &title[1..];
        }
        Some((pack.name.clone(), title))
    }

    fn start_next(&mut self) {
        let Some(pack) = self.packs.get(self.pack) else {
            return;
        };
        let path = &pack.tracks[self.next % pack.tracks.len()];
        self.next += 1;
        let stream = gtk::MediaFile::for_filename(path);
        stream.set_volume(self.volume);
        stream.play();
        self.stream = Some(stream);
    }

    /// Eases the volume towards `target` (0 to 1 of full) and moves on to the
    /// next track when one ends. Call it every frame.
    pub fn tick(&mut self, target: f64, dt: f64) {
        let switching = self.wanted != self.pack;
        let target = if self.quiet {
            0.0
        } else if switching && self.stream.is_some() {
            // Fade the old style out first, a little quicker.
            0.0
        } else {
            target.clamp(0.0, 1.0) * FULL
        };
        let k = 1.0 - (-dt / if switching { 0.6 } else { 1.6 }).exp();
        self.volume += (target - self.volume) * k;
        if switching && (self.volume < 0.01 || self.stream.is_none()) {
            if let Some(s) = self.stream.take() {
                s.pause();
            }
            self.pack = self.wanted;
            self.next = 0;
            self.volume = 0.0;
        }
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
                    eprintln!("westering: a track wouldn't play: {e}");
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
