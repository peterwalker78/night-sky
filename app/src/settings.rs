//! The menu: what Night Sky is for, music and motion, how often the sky asks,
//! where you are, what's kept and why, the logbook, and credits.

use crate::game::Game;
use crate::talk::{Go, Request};
use crate::ui::{clear, label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use night_sky_core::journal::Ask;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::{Rc, Weak};

pub struct Settings {
    pub root: gtk::Box,
    body: gtk::Box,
    game: Rc<RefCell<Game>>,
    me: RefCell<Weak<Settings>>,
    forget_armed: Cell<bool>,
    pub on_request: Go,
}

/// What the app is for, in a few plain sentences.
pub const PURPOSE: &str = "Night Sky is a small ritual for the end of the day: ten or fifteen minutes with the real sky over you. Something to find, something to learn, somewhere to set down what's on your mind, and then a nudge to go outside and look at the real thing.

The sky is never the same two nights running. The Moon moves and changes shape, planets wander, meteor showers come and go, and over the year the seasons turn the whole sky round, so there is always something new up there. The more nights you come, the more you'll know your way around it.

Over weeks, your logbook becomes a quiet record of your evenings: what you saw, what was on your mind, the people who come up again and again, and the things you looked forward to. It's meant to leave you a little calmer than it found you, and then let you go.";

const CREDITS: &str = "Night Sky is free software under the GNU GPL, version 3 or later. \
Stars from the Yale Bright Star Catalogue (Hoffleit and Warren), through the CDS in Strasbourg. \
Constellation figures from d3-celestial by Olaf Frohn (BSD licence). \
Positions of the Sun, Moon and planets after Paul Schlyter's method. \
Meteor showers from the International Meteor Organization's working list. \
Music, all dedicated to the public domain (CC0): \"Ease into Night\", \"Moon Unit\", \"Into The Mist\" and \"Calm Currents\" by HoliznaCC0; \
\"Chill lofi inspired\" and \"Lofi Hip Hop Loop\" by omfgdude.";

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            n += copy_tree(&entry.path(), &target)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|e| e == "md" || e == "toml")
        {
            std::fs::copy(entry.path(), target)?;
            n += 1;
        }
    }
    Ok(n)
}

impl Settings {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Settings> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("page");
        root.add_css_class("settings");
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_vexpand(true);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        body.set_margin_top(48);
        body.set_margin_start(72);
        body.set_margin_end(72);
        body.set_margin_bottom(48);
        body.set_width_request(560);
        body.set_halign(gtk::Align::Start);
        scroller.set_child(Some(&body));
        root.append(&scroller);
        let settings = Rc::new(Settings {
            root,
            body,
            game: game.clone(),
            me: RefCell::new(Weak::new()),
            forget_armed: Cell::new(false),
            on_request: RefCell::new(None),
        });
        *settings.me.borrow_mut() = Rc::downgrade(&settings);
        let me = Rc::downgrade(&settings);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(s) = me.upgrade()
            {
                s.request(None);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        settings.root.add_controller(keys);
        settings
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    pub fn open(&self) {
        self.forget_armed.set(false);
        clear(&self.body);
        let back = gtk::Button::with_label("Back to the sky  (Esc)");
        back.add_css_class("quiet");
        back.set_halign(gtk::Align::Start);
        let me = self.me.borrow().clone();
        back.connect_clicked(move |_| {
            if let Some(s) = me.upgrade() {
                s.request(None);
            }
        });
        self.body.append(&back);
        let title = label("Menu", "book-title");
        title.set_margin_top(18);
        self.body.append(&title);

        let settings = self.game.borrow().journal().settings.clone();

        self.body
            .append(&label("WHAT NIGHT SKY IS FOR", "book-heading"));
        self.body.append(&label(PURPOSE, "book-body"));

        self.body.append(&label("ASK ME THINGS", "book-heading"));
        let ask = gtk::DropDown::from_strings(&[
            "Sometimes (at most two a visit)",
            "Rarely (now and then)",
            "Never",
        ]);
        ask.set_halign(gtk::Align::Start);
        ask.set_selected(match settings.ask {
            Ask::Sometimes => 0,
            Ask::Rarely => 1,
            Ask::Never => 2,
        });
        let game = self.game.clone();
        ask.connect_selected_notify(move |d| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.ask = match d.selected() {
                0 => Ask::Sometimes,
                1 => Ask::Rarely,
                _ => Ask::Never,
            };
            let _ = j.save_settings();
        });
        self.body.append(&ask);

        self.body.append(&label("MUSIC", "book-heading"));
        let music =
            gtk::CheckButton::with_label("Play quiet music under the sky (M turns it off or on)");
        music.set_active(!settings.quiet);
        let game = self.game.clone();
        music.connect_toggled(move |b| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.quiet = !b.is_active();
            let _ = j.save_settings();
        });
        self.body.append(&music);
        let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume.set_value(settings.volume.unwrap_or(1.0));
        volume.set_width_request(260);
        volume.set_halign(gtk::Align::Start);
        volume.set_tooltip_text(Some("Volume"));
        let game = self.game.clone();
        volume.connect_value_changed(move |v| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.volume = Some(v.value());
            let _ = j.save_settings();
        });
        self.body.append(&volume);

        self.body.append(&label("MOTION", "book-heading"));
        let calm = gtk::CheckButton::with_label(
            "Calmer: the wisp stays on its moss and the stars twinkle less",
        );
        calm.set_active(settings.calm);
        let game = self.game.clone();
        calm.connect_toggled(move |b| {
            let mut g = game.borrow_mut();
            let j = g.journal_mut();
            j.settings.calm = b.is_active();
            let _ = j.save_settings();
        });
        self.body.append(&calm);

        self.body.append(&label("WHERE YOU ARE", "book-heading"));
        let now = match (settings.lat, settings.lon) {
            (Some(lat), Some(lon)) => format!("Set by hand: {lat:.2}, {lon:.2}."),
            _ => "Worked out from your time zone, which can be a few hundred kilometres out. That moves the sky by a few degrees; type a nearer place's latitude and longitude if you like.".into(),
        };
        self.body.append(&label(&now, "book-quiet"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let lat = gtk::Entry::new();
        lat.set_placeholder_text(Some("Latitude, e.g. 53.48"));
        let lon = gtk::Entry::new();
        lon.set_placeholder_text(Some("Longitude, e.g. -2.24"));
        if let (Some(a), Some(b)) = (settings.lat, settings.lon) {
            lat.set_text(&format!("{a}"));
            lon.set_text(&format!("{b}"));
        }
        let set = gtk::Button::with_label("Use these");
        set.add_css_class("quiet");
        let clear_place = gtk::Button::with_label("Use the time zone");
        clear_place.add_css_class("quiet");
        row.append(&lat);
        row.append(&lon);
        row.append(&set);
        row.append(&clear_place);
        self.body.append(&row);
        let note = label("", "book-quiet");
        self.body.append(&note);
        {
            let game = self.game.clone();
            let note = note.clone();
            set.connect_clicked(move |_| {
                let (a, b) = (lat.text().trim().parse::<f64>(), lon.text().trim().parse::<f64>());
                match (a, b) {
                    (Ok(a), Ok(b)) if (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b) => {
                        let mut g = game.borrow_mut();
                        let j = g.journal_mut();
                        j.settings.lat = Some(a);
                        j.settings.lon = Some(b);
                        let _ = j.save_settings();
                        note.set_text("Saved. The sky will use it next time it opens.");
                    }
                    _ => note.set_text("Latitude runs from -90 to 90 and longitude from -180 to 180, north and east positive."),
                }
            });
        }
        {
            let game = self.game.clone();
            let note = note.clone();
            clear_place.connect_clicked(move |_| {
                let mut g = game.borrow_mut();
                let j = g.journal_mut();
                j.settings.lat = None;
                j.settings.lon = None;
                let _ = j.save_settings();
                note.set_text("Back to the time zone, from next time.");
            });
        }

        self.body
            .append(&label("WHAT'S KEPT, AND WHY", "book-heading"));
        {
            let g = self.game.borrow();
            let j = g.journal();
            let nights = j.nights().len();
            let kept = [
                (
                    format!("A page for each night ({nights})"),
                    "what you found, the weights you set down and what you wrote, so the logbook can show you your evenings.",
                ),
                (
                    format!("What you've found ({})", j.found.len()),
                    "so each night offers things you haven't seen yet, and later visits show you more about the ones you have.",
                ),
                (
                    format!("Weights ({})", j.weights.len()),
                    "so you can bring one back, mark it sorted, or chart a course for it.",
                ),
                (
                    format!("Names ({})", j.people.len()),
                    "so the people who come up again and again gather on one page, and a star can carry someone's name.",
                ),
                (
                    format!("Plans ({})", j.plans.len()),
                    "so the sky can mention one as its night comes near, and ask once afterwards how it went.",
                ),
                (
                    format!("Courses ({})", j.courses.len()),
                    "the plans you've charted for a weight, and how they're going.",
                ),
                (
                    format!("Questions asked ({})", j.asked.len()),
                    "so the same one isn't asked again within a month.",
                ),
                (
                    format!("Your drawings ({})", j.drawings.len()),
                    "so the shapes you've drawn stay in the sky.",
                ),
                (
                    "Settings".to_owned(),
                    "these choices, and which tips the wisp has already given you.",
                ),
            ];
            for (what, why) in kept {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
                row.set_margin_top(4);
                row.append(&label(&what, "book-body"));
                row.append(&label(why, "book-quiet"));
                self.body.append(&row);
            }
        }

        self.body.append(&label("YOUR LOGBOOK", "book-heading"));
        let dir = self.game.borrow().journal().dir().to_owned();
        self.body.append(&label(
            "Everything you write stays on this computer, as plain text files. Nothing is ever sent anywhere: the app has no network access at all.",
            "book-quiet",
        ));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let open_book = gtk::Button::with_label("Open the logbook  (L)");
        open_book.add_css_class("quiet");
        {
            let me = self.me.borrow().clone();
            open_book.connect_clicked(move |_| {
                if let Some(s) = me.upgrade() {
                    s.request(Some(Request::Book(None)));
                }
            });
        }
        row.append(&open_book);
        let export = gtk::Button::with_label("Save a copy…");
        export.add_css_class("quiet");
        let forget = gtk::Button::with_label("Forget everything");
        forget.add_css_class("quiet");
        row.append(&export);
        row.append(&forget);
        self.body.append(&row);
        let said = label("", "book-quiet");
        self.body.append(&said);
        {
            let said = said.clone();
            let dir = dir.clone();
            export.connect_clicked(move |b| {
                let dialog = gtk::FileDialog::builder()
                    .title("Choose where to put a copy")
                    .build();
                let window = b.root().and_downcast::<gtk::Window>();
                let said = said.clone();
                let dir = dir.clone();
                dialog.select_folder(window.as_ref(), gio::Cancellable::NONE, move |r| {
                    if let Ok(folder) = r
                        && let Some(path) = folder.path()
                    {
                        let target = path.join("Night Sky logbook");
                        match copy_tree(&dir, &target) {
                            Ok(n) => {
                                said.set_text(&format!("Copied {n} files to {}.", target.display()))
                            }
                            Err(e) => said.set_text(&format!("That didn't work: {e}.")),
                        }
                    }
                });
            });
        }
        {
            let game = self.game.clone();
            let me = self.me.borrow().clone();
            forget.connect_clicked(move |b| {
                let Some(s) = me.upgrade() else { return };
                if !s.forget_armed.get() {
                    s.forget_armed.set(true);
                    b.set_label("Press again to forget everything");
                    return;
                }
                let r = game.borrow_mut().forget_everything();
                s.forget_armed.set(false);
                b.set_label("Forget everything");
                said.set_text(match r {
                    Ok(()) => "Forgotten. The logbook is empty.",
                    Err(_) => "Some files couldn't be removed.",
                });
            });
        }

        self.body.append(&label("ABOUT", "book-heading"));
        self.body.append(&label(CREDITS, "book-quiet"));
        let photos = crate::eyepiece::Photos::load();
        if !photos.credits().is_empty() {
            self.body.append(&label("PHOTOGRAPHS", "book-heading"));
            for c in photos.credits() {
                self.body.append(&label(
                    &format!("{}: {}. {}. {}", c.title, c.credit, c.licence, c.source),
                    "book-quiet",
                ));
            }
        }
        back.grab_focus();
    }
}
