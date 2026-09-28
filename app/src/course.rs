//! Charting a course for a weight, only ever when the user asks: four short
//! steps (a wish, the best of it, what gets in the way, and an if-then
//! plan), with the user's own earlier words beside each one.

use crate::game::Game;
use crate::talk::{Go, Request};
use crate::ui::{clear, detach, label};
use gtk::prelude::*;
use gtk::{gdk, glib};
use night_sky_core::journal::{Course as Record, Journal};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

const STEPS: [(&str, &str, &str); 4] = [
    (
        "WISH",
        "If this weight got lighter, what would that look like?",
        "One line.",
    ),
    (
        "THE BEST OF IT",
        "Picture the best of it. What would be different on an ordinary Tuesday?",
        "Take a moment with it before you write.",
    ),
    (
        "WHAT GETS IN THE WAY",
        "What in you tends to get in the way?",
        "Not other people, not luck: the thing you do, feel or put off.",
    ),
    (
        "THE PLAN",
        "When that happens, what will you do instead?",
        "Say it as: if this, then I'll do that.",
    ),
];

#[derive(Default)]
struct State {
    weight: u32,
    id: u32,
    step: usize,
    fields: [String; 4],
}

pub struct Course {
    pub root: gtk::Box,
    game: Rc<RefCell<Game>>,
    me: RefCell<Weak<Course>>,
    state: RefCell<State>,
    main: gtk::Box,
    side: gtk::Box,
    words: gtk::Box,
    entry: gtk::Entry,
    then: gtk::Entry,
    check: gtk::CheckButton,
    pub on_request: Go,
}

impl Course {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Course> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 48);
        root.add_css_class("page");
        root.add_css_class("course");
        root.set_margin_top(56);
        root.set_margin_start(72);
        root.set_margin_end(56);
        let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
        main.set_width_request(560);
        main.set_hexpand(true);
        let side = gtk::Box::new(gtk::Orientation::Vertical, 6);
        side.set_width_request(280);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 6);
        side.append(&label("YOUR WORDS", "course-step"));
        side.append(&label(
            "Click one, or press Ctrl and its number, to bring it into the box.",
            "book-quiet",
        ));
        side.append(&words);
        root.append(&main);
        root.append(&side);
        let course = Rc::new(Course {
            root,
            game: game.clone(),
            me: RefCell::new(Weak::new()),
            state: RefCell::new(State::default()),
            main,
            side,
            words,
            entry: gtk::Entry::new(),
            then: gtk::Entry::new(),
            check: gtk::CheckButton::with_label("Ask me how it's going in a week"),
            on_request: RefCell::new(None),
        });
        *course.me.borrow_mut() = Rc::downgrade(&course);
        for entry in [&course.entry, &course.then] {
            let me = Rc::downgrade(&course);
            entry.connect_activate(move |_| {
                if let Some(c) = me.upgrade() {
                    c.next();
                }
            });
        }
        let me = Rc::downgrade(&course);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(c) = me.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if key == gdk::Key::Escape {
                c.leave();
                return glib::Propagation::Stop;
            }
            if state.contains(gdk::ModifierType::CONTROL_MASK)
                && let Some(n) = key.to_unicode().and_then(|ch| ch.to_digit(10))
            {
                c.pull(n as usize);
                return glib::Propagation::Stop;
            }
            if c.state.borrow().step == 4 && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
                c.finish();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        course.root.add_controller(keys);
        course
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    /// Starts or picks up a course for a weight.
    pub fn open(&self, weight: u32) {
        let mut game = self.game.borrow_mut();
        let night = game.night().to_owned();
        let journal = game.journal_mut();
        let existing = journal
            .courses
            .iter()
            .find(|c| c.weight == weight && c.draft)
            .cloned();
        let record = existing.unwrap_or_else(|| {
            let id = Journal::next_id(journal.courses.iter().map(|c| c.id));
            let fresh = Record {
                id,
                weight,
                made: night,
                draft: true,
                ..Record::default()
            };
            journal.courses.push(fresh.clone());
            fresh
        });
        let fields = [
            record.wish.clone(),
            record.outcome.clone(),
            record.obstacle.clone(),
            record.plan.clone(),
        ];
        let step = fields.iter().position(|f| f.is_empty()).unwrap_or(3);
        *self.state.borrow_mut() = State {
            weight,
            id: record.id,
            step,
            fields,
        };
        drop(game);
        self.show();
    }

    fn words_for(&self) -> Vec<String> {
        let game = self.game.borrow();
        let journal = game.journal();
        let state = self.state.borrow();
        let mut out = Vec::new();
        if let Some(w) = journal.weight(state.weight) {
            out.push(w.text.clone());
            for night in w.nights.iter().rev() {
                if let Some(page) = journal.night(night) {
                    out.extend(page.answers.iter().map(|a| a.text.clone()));
                }
            }
        }
        for c in journal
            .courses
            .iter()
            .filter(|c| c.weight == state.weight && c.id != state.id)
        {
            out.extend([c.wish.clone(), c.obstacle.clone(), c.plan.clone()]);
        }
        for f in &state.fields {
            out.retain(|x| x != f);
        }
        out.retain(|x| !x.trim().is_empty());
        let mut seen = std::collections::HashSet::new();
        out.retain(|x| seen.insert(x.to_lowercase()));
        out.truncate(9);
        out
    }

    fn show(&self) {
        clear(&self.main);
        clear(&self.words);
        detach(&self.entry);
        detach(&self.then);
        detach(&self.check);
        let state = self.state.borrow();
        let weight = self
            .game
            .borrow()
            .journal()
            .weight(state.weight)
            .map(|w| w.text.clone())
            .unwrap_or_default();
        let top = label(&format!("A course for “{weight}”"), "book-quiet");
        self.main.append(&top);
        self.side.set_visible(state.step < 4);
        if state.step == 4 {
            drop(state);
            self.card();
            return;
        }
        let (step, ask, sub) = STEPS[state.step];
        let s = label(
            &format!("{} · {} OF 4", step, state.step + 1),
            "course-step",
        );
        s.set_margin_top(28);
        self.main.append(&s);
        self.main.append(&label(ask, "course-ask"));
        self.main.append(&label(sub, "book-quiet"));
        if state.step == 3 {
            let row1 = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row1.append(&label("If", "course-ask"));
            self.entry.set_hexpand(true);
            let obstacle = state.fields[2].trim_end_matches('.').to_owned();
            let (lhs, rhs) = state
                .fields
                .get(3)
                .and_then(|p| p.split_once(" → "))
                .map(|(a, b)| (a.to_owned(), b.to_owned()))
                .unwrap_or((obstacle, String::new()));
            self.entry.set_text(&lhs);
            row1.append(&self.entry);
            let row2 = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row2.append(&label("then I'll", "course-ask"));
            self.then.set_hexpand(true);
            self.then.set_text(&rhs);
            self.then
                .set_placeholder_text(Some("go for a walk, ring someone, write it down…"));
            row2.append(&self.then);
            row1.set_margin_top(12);
            self.main.append(&row1);
            self.main.append(&row2);
        } else {
            self.entry.set_text(&state.fields[state.step]);
            self.entry.set_placeholder_text(None);
            self.entry.set_margin_top(12);
            self.main.append(&self.entry);
        }
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_margin_top(16);
        let next = gtk::Button::with_label("Next  (Enter)");
        next.add_css_class("quiet");
        let back = gtk::Button::with_label("A step back");
        back.add_css_class("quiet");
        back.set_sensitive(state.step > 0);
        let leave = gtk::Button::with_label("Leave it for now  (Esc)");
        leave.add_css_class("quiet");
        buttons.append(&next);
        buttons.append(&back);
        buttons.append(&leave);
        self.main.append(&buttons);
        let me = self.me.borrow().clone();
        next.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.next();
            }
        });
        let me = self.me.borrow().clone();
        back.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.back();
            }
        });
        let me = self.me.borrow().clone();
        leave.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.leave();
            }
        });
        drop(state);
        for (i, w) in self.words_for().into_iter().enumerate() {
            let b = gtk::Button::with_label(&format!("{}  {}", i + 1, w));
            b.add_css_class("quiet");
            if let Some(l) = b.child().and_downcast::<gtk::Label>() {
                l.set_wrap(true);
                l.set_xalign(0.0);
                l.set_max_width_chars(34);
            }
            let me = self.me.borrow().clone();
            b.connect_clicked(move |_| {
                if let Some(c) = me.upgrade() {
                    c.pull(i + 1);
                }
            });
            self.words.append(&b);
        }
        let entry = if self.state.borrow().step == 3 && !self.entry.text().is_empty() {
            self.then.clone()
        } else {
            self.entry.clone()
        };
        glib::idle_add_local_once(move || {
            entry.grab_focus();
        });
    }

    /// Brings one of the user's own lines into the box.
    fn pull(&self, n: usize) {
        if let Some(w) = self.words_for().get(n.wrapping_sub(1)) {
            let target = if self.state.borrow().step == 3 && self.then.has_focus() {
                &self.then
            } else {
                &self.entry
            };
            target.set_text(w);
            target.set_position(-1);
        }
    }

    fn take_field(&self) {
        let mut state = self.state.borrow_mut();
        let step = state.step;
        if step < 4 {
            state.fields[step] = if step == 3 {
                format!("{} → {}", self.entry.text().trim(), self.then.text().trim())
            } else {
                self.entry.text().trim().to_owned()
            };
        }
    }

    fn save(&self, draft: bool, check: bool) {
        let state = self.state.borrow();
        let mut game = self.game.borrow_mut();
        let journal = game.journal_mut();
        if let Some(c) = journal.courses.iter_mut().find(|c| c.id == state.id) {
            c.wish = state.fields[0].clone();
            c.outcome = state.fields[1].clone();
            c.obstacle = state.fields[2].clone();
            let (lhs, rhs) = state.fields[3].split_once(" → ").unwrap_or(("", ""));
            if !lhs.is_empty() {
                c.obstacle = lhs.to_owned();
            }
            c.plan = rhs.to_owned();
            c.draft = draft;
            if !draft {
                c.check_after = check.then(|| {
                    let today = c.made.clone();
                    add_days(&today, 7)
                });
            }
        }
        if let Err(e) = journal.save_courses() {
            eprintln!("night-sky: couldn't save the course: {e}");
        }
    }

    fn next(&self) {
        self.take_field();
        let empty = {
            let s = self.state.borrow();
            match s.step {
                3 => self.then.text().trim().is_empty(),
                n => s.fields[n].is_empty(),
            }
        };
        if empty {
            return;
        }
        self.state.borrow_mut().step += 1;
        self.save(true, false);
        self.show();
    }

    fn back(&self) {
        self.take_field();
        {
            let mut s = self.state.borrow_mut();
            s.step = s.step.saturating_sub(1);
        }
        self.save(true, false);
        self.show();
    }

    fn leave(&self) {
        if self.state.borrow().step < 4 {
            self.take_field();
            self.save(true, false);
        }
        self.request(Some(Request::Book(Some("courses".into()))));
    }

    fn card(&self) {
        let state = self.state.borrow();
        let (lhs, rhs) = state.fields[3].split_once(" → ").unwrap_or(("", ""));
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("book-card");
        card.set_margin_top(28);
        card.append(&label(&state.fields[0], "book-body"));
        card.append(&label(
            &format!(
                "If {}, then I'll {}.",
                lhs.trim_end_matches('.'),
                rhs.trim_end_matches('.')
            ),
            "book-big",
        ));
        self.main.append(&card);
        self.check.set_margin_top(16);
        self.main.append(&self.check);
        let done = gtk::Button::with_label("Keep it  (Enter)");
        done.add_css_class("quiet");
        done.set_halign(gtk::Align::Start);
        done.set_margin_top(12);
        self.main.append(&done);
        let me = self.me.borrow().clone();
        done.connect_clicked(move |_| {
            if let Some(c) = me.upgrade() {
                c.finish();
            }
        });
        let d = done.clone();
        glib::idle_add_local_once(move || {
            d.grab_focus();
        });
    }

    fn finish(&self) {
        self.save(false, self.check.is_active());
        self.request(Some(Request::Book(Some("courses".into()))));
    }
}

/// A night key some days on.
fn add_days(key: &str, days: i64) -> String {
    let mut p = key.split('-').map(|x| x.parse::<i64>().unwrap_or(1));
    let (y, m, d) = (
        p.next().unwrap_or(2000),
        p.next().unwrap_or(1),
        p.next().unwrap_or(1),
    );
    let at = night_sky_core::time::midnight_utc(y as i32, m as u32, d as u32)
        + days * night_sky_core::time::DAY
        + night_sky_core::time::HOUR;
    night_sky_core::questions::key_of(at, 0)
}
