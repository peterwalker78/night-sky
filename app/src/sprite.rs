//! Drawing the wisp: cairo lends it a surface, and the picture goes to the
//! view as a small texture.

use gtk::{cairo, gdk, glib, prelude::*};
use night_sky_core::canvas::{Canvas, Paint, Stop};
use night_sky_core::wisp::Wisp;

/// The wisp's nook, in its own units; it is drawn this many times larger.
pub const NOOK: (f64, f64) = (152.0, 56.0);
pub const SIZE: f64 = 1.9;

struct CairoCanvas<'a> {
    cr: &'a cairo::Context,
}

impl CairoCanvas<'_> {
    fn stops(gradient: &cairo::Gradient, stops: &[Stop]) {
        for s in stops {
            gradient.add_color_stop_rgba(
                s.at,
                s.colour.0,
                s.colour.1,
                s.colour.2,
                s.alpha.clamp(0.0, 1.0),
            );
        }
    }
}

impl Canvas for CairoCanvas<'_> {
    fn save(&mut self) {
        let _ = self.cr.save();
    }
    fn restore(&mut self) {
        let _ = self.cr.restore();
    }
    fn translate(&mut self, dx: f64, dy: f64) {
        self.cr.translate(dx, dy);
    }
    fn scale(&mut self, sx: f64, sy: f64) {
        self.cr.scale(sx, sy);
    }
    fn move_to(&mut self, x: f64, y: f64) {
        self.cr.move_to(x, y);
    }
    fn line_to(&mut self, x: f64, y: f64) {
        self.cr.line_to(x, y);
    }
    fn curve_to(&mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) {
        self.cr.curve_to(c1x, c1y, c2x, c2y, x, y);
    }
    fn arc(&mut self, x: f64, y: f64, radius: f64, from: f64, to: f64) {
        self.cr.arc(x, y, radius, from, to);
    }
    fn rectangle(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.cr.rectangle(x, y, width, height);
    }
    fn close_path(&mut self) {
        self.cr.close_path();
    }
    fn new_path(&mut self) {
        self.cr.new_path();
    }
    fn set_paint(&mut self, paint: Paint<'_>) {
        match paint {
            Paint::Solid { colour, alpha } => {
                self.cr
                    .set_source_rgba(colour.0, colour.1, colour.2, alpha.clamp(0.0, 1.0));
            }
            Paint::Radial {
                inner,
                outer,
                stops,
            } => {
                let gradient = cairo::RadialGradient::new(
                    inner.0, inner.1, inner.2, outer.0, outer.1, outer.2,
                );
                Self::stops(&gradient, stops);
                let _ = self.cr.set_source(&gradient);
            }
            Paint::Linear { from, to, stops } => {
                let gradient = cairo::LinearGradient::new(from.0, from.1, to.0, to.1);
                Self::stops(&gradient, stops);
                let _ = self.cr.set_source(&gradient);
            }
        }
    }
    fn set_line_width(&mut self, width: f64) {
        self.cr.set_line_width(width);
    }
    fn set_round_ends(&mut self, round: bool) {
        let (cap, join) = if round {
            (cairo::LineCap::Round, cairo::LineJoin::Round)
        } else {
            (cairo::LineCap::Butt, cairo::LineJoin::Miter)
        };
        self.cr.set_line_cap(cap);
        self.cr.set_line_join(join);
    }
    fn set_adding(&mut self, adding: bool) {
        self.cr.set_operator(if adding {
            cairo::Operator::Add
        } else {
            cairo::Operator::Over
        });
    }
    fn fill(&mut self) {
        let _ = self.cr.fill();
    }
    fn stroke(&mut self) {
        let _ = self.cr.stroke();
    }
    fn clip(&mut self) {
        self.cr.clip();
    }
}

/// One frame of the wisp at `now` milliseconds, for a display `scale`.
pub fn render(wisp: &mut Wisp, now: f64, scale: f64) -> Option<gdk::Texture> {
    let k = SIZE * scale;
    let (w, h) = ((NOOK.0 * k).ceil() as i32, (NOOK.1 * k).ceil() as i32);
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        cr.scale(k, k);
        let mut canvas = CairoCanvas { cr: &cr };
        wisp.draw(now, true, true, &mut canvas);
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?;
    let bytes = glib::Bytes::from(&data[..]);
    Some(
        gdk::MemoryTexture::new(
            w,
            h,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        )
        .upcast(),
    )
}
