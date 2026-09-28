//! Photographs of what's been found. Close in on something already found and
//! a real photograph of it takes over from the dots, at its true size on the
//! sky and turned as it sits there tonight. The Moon and Mercury wear
//! tonight's phase.

use gtk::{gdk, glib, prelude::*};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct Credit {
    pub id: String,
    pub title: String,
    pub credit: String,
    pub licence: String,
    pub source: String,
    /// Degrees north is turned clockwise from straight up in the file.
    #[serde(default)]
    pub north: Option<f64>,
}

#[derive(Deserialize)]
struct Credits {
    image: Vec<Credit>,
}

/// How a disc is lit: which way the Sun is in the picture, and how far round.
#[derive(Clone, Copy, PartialEq)]
pub struct Phase {
    /// Towards the Sun, in the picture's own frame (y down), unit length.
    pub sun: (f64, f64),
    /// The phase angle, degrees: 0 full, 180 new.
    pub angle: f64,
}

pub struct Photos {
    dir: Option<PathBuf>,
    credits: Vec<Credit>,
    cache: Option<(String, gdk::Texture)>,
}

fn folder() -> Option<PathBuf> {
    let mut places = vec![PathBuf::from("/app/share/night-sky/images")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(prefix) = exe.parent().and_then(|p| p.parent())
    {
        places.push(prefix.join("share/night-sky/images"));
    }
    places.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/images"
    )));
    places
        .into_iter()
        .find(|p| p.join("credits.toml").is_file())
}

/// The share of the picture's width the disc of a Moon or planet fills.
pub const DISC: f64 = 0.88;

impl Photos {
    pub fn load() -> Photos {
        let dir = folder();
        let credits = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("credits.toml")).ok())
            .and_then(|text| toml::from_str::<Credits>(&text).ok())
            .map(|c| c.image)
            .unwrap_or_default();
        Photos {
            dir,
            credits,
            cache: None,
        }
    }

    pub fn credits(&self) -> &[Credit] {
        &self.credits
    }

    pub fn credit(&self, id: &str) -> Option<&Credit> {
        self.credits.iter().find(|c| c.id == id)
    }

    /// The photograph for `id`, lit to `phase` if given. Kept while it's the
    /// one being looked at.
    pub fn texture(&mut self, id: &str, phase: Option<Phase>) -> Option<gdk::Texture> {
        self.credit(id)?;
        // Round the lighting so small drifts of the sky don't redo the work.
        let key = match phase {
            Some(p) => format!(
                "{id}:{:.0}:{:.0}",
                p.angle / 2.0,
                p.sun.1.atan2(p.sun.0).to_degrees() / 3.0
            ),
            None => id.to_owned(),
        };
        if let Some((k, t)) = &self.cache
            && *k == key
        {
            return Some(t.clone());
        }
        let path = self.dir.as_ref()?.join(format!("{id}.jpg"));
        let photo = gdk::Texture::from_filename(&path).ok()?;
        let texture = match phase {
            Some(p) => lit(&photo, p),
            None => photo,
        };
        self.cache = Some((key, texture.clone()));
        Some(texture)
    }
}

/// Darkens the part of a disc turned away from the Sun, leaving a trace of
/// earthshine, with a soft terminator.
fn lit(photo: &gdk::Texture, phase: Phase) -> gdk::Texture {
    let (w, h) = (photo.width() as usize, photo.height() as usize);
    let stride = w * 4;
    let mut data = vec![0u8; stride * h];
    photo.download(&mut data, stride);
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let radius = w.min(h) as f64 / 2.0 * DISC;
    let angle = phase.angle.to_radians();
    let (si, ci) = (angle.sin(), angle.cos());
    for y in 0..h {
        for x in 0..w {
            let u = (x as f64 + 0.5 - cx) / radius;
            let v = (y as f64 + 0.5 - cy) / radius;
            let rho2 = u * u + v * v;
            if rho2 > 1.08 {
                continue;
            }
            let z = (1.0 - rho2.min(1.0)).sqrt();
            let light = (u * phase.sun.0 + v * phase.sun.1) * si + z * ci;
            let t = (light * 7.0 + 0.5).clamp(0.0, 1.0);
            let day = t * t * (3.0 - 2.0 * t);
            let keep = (day + (1.0 - day) * 0.05) as f32;
            let i = y * stride + x * 4;
            for c in &mut data[i..i + 3] {
                *c = (*c as f32 * keep) as u8;
            }
        }
    }
    gdk::MemoryTexture::new(
        w as i32,
        h as i32,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(data),
        stride,
    )
    .upcast()
}
