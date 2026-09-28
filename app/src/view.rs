//! The widget the sky is drawn on. It holds one finished frame at a time:
//! the dot field's texture, masked into round dots on the GPU, and a few
//! lines of text.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, graphene, gsk, pango};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Centre,
}

pub struct Text {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub size: f64,
    pub bold: bool,
    pub alpha: f64,
    pub align: Align,
    /// Wrap width in pixels.
    pub wrap: Option<f64>,
    pub color: [f32; 3],
}

impl Text {
    pub fn new(x: f64, y: f64, text: impl Into<String>, size: f64, alpha: f64) -> Text {
        Text {
            x,
            y,
            text: text.into(),
            size,
            bold: false,
            alpha,
            align: Align::Left,
            wrap: None,
            color: [0.93, 0.94, 0.97],
        }
    }
    pub fn centred(mut self) -> Text {
        self.align = Align::Centre;
        self
    }
    pub fn bold(mut self) -> Text {
        self.bold = true;
        self
    }
    pub fn wrap(mut self, width: f64) -> Text {
        self.wrap = Some(width);
        self
    }
    pub fn color(mut self, color: [f32; 3]) -> Text {
        self.color = color;
        self
    }
}

/// A small picture drawn over the sky: the wisp.
pub struct Sprite {
    pub texture: gdk::Texture,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub alpha: f64,
}

/// Words in a speech bubble, standing on `bottom` from `x`.
pub struct Bubble {
    pub x: f64,
    pub bottom: f64,
    pub text: String,
    pub width: f64,
    pub alpha: f64,
}

#[derive(Default)]
pub struct Frame {
    pub sprite: Option<Sprite>,
    pub bubble: Option<Bubble>,
    pub texture: Option<gdk::Texture>,
    pub cols: usize,
    pub rows: usize,
    pub pitch: f32,
    pub background: [f32; 3],
    pub texts: Vec<Text>,
    /// Over everything: 0 is none, 1 is black.
    pub veil: f32,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SkyView {
        pub frame: RefCell<Frame>,
        pub tile: RefCell<Option<(f32, i32, gdk::Texture)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SkyView {
        const NAME: &'static str = "NightSkyView";
        type Type = super::SkyView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SkyView {}

    impl WidgetImpl for SkyView {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let frame = self.frame.borrow();
            let bg = frame.background;
            snapshot.append_color(
                &gdk::RGBA::new(bg[0], bg[1], bg[2], 1.0),
                &graphene::Rect::new(0.0, 0.0, w, h),
            );
            if let Some(texture) = &frame.texture {
                let pitch = frame.pitch;
                let scale = widget.scale_factor();
                let tile = {
                    let mut cached = self.tile.borrow_mut();
                    match &*cached {
                        Some((p, s, t)) if *p == pitch && *s == scale => t.clone(),
                        _ => {
                            let t = crate::field::dot_tile(pitch, scale as f64);
                            *cached = Some((pitch, scale, t.clone()));
                            t
                        }
                    }
                };
                let bounds = graphene::Rect::new(
                    0.0,
                    0.0,
                    frame.cols as f32 * pitch,
                    frame.rows as f32 * pitch,
                );
                let cell = graphene::Rect::new(0.0, 0.0, pitch, pitch);
                snapshot.push_mask(gsk::MaskMode::Alpha);
                snapshot.push_repeat(&bounds, Some(&cell));
                snapshot.append_texture(&tile, &cell);
                snapshot.pop();
                snapshot.pop();
                snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Nearest, &bounds);
                snapshot.pop();
            }
            if let Some(sprite) = &frame.sprite {
                snapshot.push_opacity(sprite.alpha.clamp(0.0, 1.0));
                snapshot.append_texture(
                    &sprite.texture,
                    &graphene::Rect::new(
                        sprite.x as f32,
                        sprite.y as f32,
                        sprite.width as f32,
                        sprite.height as f32,
                    ),
                );
                snapshot.pop();
            }
            if let Some(b) = &frame.bubble
                && b.alpha > 0.004
            {
                let layout = widget.create_pango_layout(Some(&b.text));
                let mut font = widget
                    .pango_context()
                    .font_description()
                    .unwrap_or_default();
                font.set_absolute_size(15.0 * pango::SCALE as f64);
                layout.set_font_description(Some(&font));
                layout.set_width((b.width * pango::SCALE as f64) as i32);
                layout.set_wrap(pango::WrapMode::WordChar);
                let (lw, lh) = layout.pixel_size();
                let pad = 14.0f32;
                let rect = graphene::Rect::new(
                    b.x as f32,
                    b.bottom as f32 - lh as f32 - 2.0 * pad,
                    lw as f32 + 2.0 * pad,
                    lh as f32 + 2.0 * pad,
                );
                let a = b.alpha as f32;
                snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, 14.0));
                snapshot.append_color(&gdk::RGBA::new(0.05, 0.06, 0.11, 0.86 * a), &rect);
                snapshot.pop();
                // A small tail down towards the wisp.
                let tail =
                    graphene::Rect::new(b.x as f32 + 34.0, b.bottom as f32 - 7.0, 12.0, 12.0);
                snapshot.save();
                snapshot.translate(&graphene::Point::new(tail.x() + 6.0, tail.y() + 6.0));
                snapshot.rotate(45.0);
                snapshot.append_color(
                    &gdk::RGBA::new(0.05, 0.06, 0.11, 0.86 * a),
                    &graphene::Rect::new(-6.0, -6.0, 12.0, 12.0),
                );
                snapshot.restore();
                snapshot.save();
                snapshot.translate(&graphene::Point::new(rect.x() + pad, rect.y() + pad));
                snapshot.append_layout(&layout, &gdk::RGBA::new(1.0, 0.96, 0.88, 0.95 * a));
                snapshot.restore();
            }
            for t in &frame.texts {
                if t.alpha <= 0.004 || t.text.is_empty() {
                    continue;
                }
                let layout = widget.create_pango_layout(Some(&t.text));
                let mut font = widget
                    .pango_context()
                    .font_description()
                    .unwrap_or_default();
                font.set_absolute_size(t.size * pango::SCALE as f64);
                font.set_weight(if t.bold {
                    pango::Weight::Semibold
                } else {
                    pango::Weight::Normal
                });
                layout.set_font_description(Some(&font));
                if let Some(wrap) = t.wrap {
                    layout.set_width((wrap * pango::SCALE as f64) as i32);
                    layout.set_wrap(pango::WrapMode::WordChar);
                    if t.align == Align::Centre {
                        layout.set_alignment(pango::Alignment::Center);
                    }
                }
                let (lw, _) = layout.pixel_size();
                let x = match (t.align, t.wrap) {
                    (Align::Left, _) => t.x,
                    // Pango centres wrapped lines within the wrap width itself.
                    (Align::Centre, Some(wrap)) => t.x - wrap / 2.0,
                    (Align::Centre, None) => t.x - lw as f64 / 2.0,
                };
                snapshot.save();
                snapshot.translate(&graphene::Point::new(x as f32, t.y as f32));
                let c = t.color;
                snapshot.append_layout(&layout, &gdk::RGBA::new(c[0], c[1], c[2], t.alpha as f32));
                snapshot.restore();
            }
            if frame.veil > 0.0 {
                snapshot.append_color(
                    &gdk::RGBA::new(0.0, 0.0, 0.0, frame.veil.min(1.0)),
                    &graphene::Rect::new(0.0, 0.0, w, h),
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct SkyView(ObjectSubclass<imp::SkyView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SkyView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl SkyView {
    pub fn show(&self, frame: Frame) {
        *self.imp().frame.borrow_mut() = frame;
        self.queue_draw();
    }
}

use gtk::glib;
