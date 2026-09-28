//! The logbook: a page for every night visited, and three contents pages
//! that gather what keeps coming back. It never counts anything.

use crate::game::Game;
use crate::talk::{Go, Request};
use crate::ui::{clear, label};
use gtk::prelude::*;
use gtk::{gdk, glib};
use night_sky_core::journal::{Journal, long_date, short_date};
use night_sky_core::questions::days_between;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Book {
    pub root: gtk::Box,
    list: gtk::ListBox,
    content: gtk::Box,
    /// The key behind each list row: a night, or a contents page.
    keys: RefCell<Vec<String>>,
    game: Rc<RefCell<Game>>,
    me: RefCell<std::rc::Weak<Book>>,
    /// Asks the window to go somewhere else: back to the sky, or a course.
    pub on_request: Go,
}

impl Book {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<Book> {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("page");
        let side = gtk::ScrolledWindow::new();
        side.set_width_request(240);
        side.set_hscrollbar_policy(gtk::PolicyType::Never);
        let list = gtk::ListBox::new();
        list.add_css_class("book-side");
        list.set_vexpand(true);
        side.set_child(Some(&list));
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hexpand(true);
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(40);
        content.set_margin_bottom(60);
        content.set_margin_start(56);
        content.set_margin_end(56);
        let clamp = gtk::Box::new(gtk::Orientation::Vertical, 0);
        clamp.set_halign(gtk::Align::Start);
        clamp.set_width_request(560);
        clamp.append(&content);
        scroller.set_child(Some(&clamp));
        root.append(&side);
        root.append(&scroller);
        let book = Rc::new(Book {
            root,
            list,
            content,
            keys: RefCell::new(Vec::new()),
            game: game.clone(),
            me: RefCell::new(std::rc::Weak::new()),
            on_request: RefCell::new(None),
        });
        *book.me.borrow_mut() = Rc::downgrade(&book);
        {
            let weak = Rc::downgrade(&book);
            book.list.connect_row_selected(move |_, row| {
                if let (Some(book), Some(row)) = (weak.upgrade(), row) {
                    let key = book.keys.borrow().get(row.index() as usize).cloned();
                    if let Some(key) = key {
                        book.render(&key);
                    }
                }
            });
        }
        {
            let weak = Rc::downgrade(&book);
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape
                    && let Some(book) = weak.upgrade()
                {
                    book.request(None);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            book.root.add_controller(keys);
        }
        book
    }

    fn request(&self, r: Option<Request>) {
        if let Some(f) = &*self.on_request.borrow() {
            f(r);
        }
    }

    /// Fills the list and opens a page: a night key, or "fixed", "coming",
    /// "weights", "courses". None opens the newest night.
    pub fn open(&self, at: Option<&str>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let game = self.game.borrow();
        let journal = game.journal();
        let mut keys = Vec::new();
        let mut add = |key: &str, text: &str, section: bool| {
            let l = gtk::Label::new(Some(text));
            l.set_xalign(0.0);
            if section {
                l.add_css_class("section");
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&l));
                row.set_selectable(false);
                row.set_activatable(false);
                self.list.append(&row);
            } else {
                self.list.append(&l);
            }
            keys.push(key.to_owned());
        };
        add("", "CONTENTS", true);
        add("fixed", "Fixed stars", false);
        add("coming", "Coming up", false);
        add("weights", "Recurring weights", false);
        add("courses", "Courses", false);
        add("", "NIGHTS", true);
        let nights = journal.nights();
        for n in &nights {
            add(n, &short_date(n), false);
        }
        if nights.is_empty() {
            add("", "None yet", true);
        }
        drop(game);
        let fallback = nights.first().cloned().unwrap_or_else(|| "fixed".into());
        let want = at.map(str::to_owned).unwrap_or_else(|| fallback.clone());
        let index = keys
            .iter()
            .position(|k| *k == want)
            .or_else(|| keys.iter().position(|k| *k == fallback))
            .unwrap_or(1);
        let key = keys[index].clone();
        *self.keys.borrow_mut() = keys;
        if let Some(row) = self.list.row_at_index(index as i32) {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
        self.render(&key);
    }

    fn render(&self, key: &str) {
        clear(&self.content);
        let back = gtk::Button::with_label("Back to the sky  (Esc)");
        back.add_css_class("quiet");
        back.set_halign(gtk::Align::Start);
        self.content.append(&back);
        let me = self.me.borrow().clone();
        back.connect_clicked(move |_| {
            if let Some(book) = me.upgrade() {
                book.request(None);
            }
        });
        let game = self.game.clone();
        let g = game.borrow();
        let journal = g.journal();
        match key {
            "fixed" => self.fixed(journal),
            "coming" => self.coming(journal, g.night()),
            "weights" => self.weights(journal),
            "courses" => self.courses(journal),
            night if night.len() == 10 => self.night(journal, night),
            _ => {}
        }
    }

    fn heading(&self, text: &str) {
        self.content.append(&label(text, "book-heading"));
    }

    fn title(&self, text: &str) {
        let l = label(text, "book-title");
        l.set_margin_top(18);
        self.content.append(&l);
    }

    fn night(&self, journal: &Journal, key: &str) {
        self.title(&long_date(key));
        let Some(night) = journal.night(key) else {
            return;
        };
        if !night.moon.is_empty() {
            self.content.append(&label(&night.moon, "book-quiet"));
        }
        if !night.finds.is_empty() {
            self.heading("FOUND");
            self.content
                .append(&label(&night.finds.join(" · "), "book-body"));
        }
        if !night.weights.is_empty() {
            self.heading("WEIGHTS");
            for w in &night.weights {
                if let Some(weight) = journal.weight(w.weight) {
                    self.weight_row(weight.id, &weight.text, weight.sorted);
                    self.looks(weight);
                }
            }
        }
        if !night.answers.is_empty() {
            self.heading("THE SKY ASKED");
            for a in &night.answers {
                let asked = label(&a.prompt, "book-asked");
                asked.set_margin_top(8);
                self.content.append(&asked);
                self.content.append(&label(&a.text, "book-body"));
            }
        }
        if !night.drawings.is_empty() {
            self.heading("DRAWN");
            self.content
                .append(&label(&night.drawings.join(" · "), "book-body"));
        }
        let plans: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| night.plans.contains(&p.id))
            .collect();
        if !plans.is_empty() {
            self.heading("PLANNED");
            for p in plans {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                self.content.append(&label(
                    &format!("{}{who} · {}, {}", p.what, p.event, short_date(&p.date)),
                    "book-body",
                ));
            }
        }
    }

    /// How a weight sat when it was looked back at, later.
    fn looks(&self, weight: &night_sky_core::journal::Weight) {
        if weight.looks.is_empty() {
            return;
        }
        let later: Vec<String> = weight
            .looks
            .iter()
            .map(|k| format!("{} on {}", k.answer, short_date(&k.night)))
            .collect();
        self.content.append(&label(
            &format!("Looking back: {}", later.join(" · ")),
            "book-quiet",
        ));
    }

    fn weight_row(&self, id: u32, text: &str, sorted: bool) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_margin_top(4);
        let l = label(text, "book-body");
        l.set_hexpand(true);
        if sorted {
            l.set_opacity(0.55);
        }
        row.append(&l);
        let sort = gtk::Button::with_label(if sorted {
            "Not sorted after all"
        } else {
            "Sorted"
        });
        sort.add_css_class("quiet");
        let course = gtk::Button::with_label("Chart a course");
        course.add_css_class("quiet");
        row.append(&sort);
        row.append(&course);
        self.content.append(&row);
        let me = self.me.borrow().clone();
        let game = self.game.clone();
        sort.connect_clicked(move |_| {
            {
                let mut g = game.borrow_mut();
                let j = g.journal_mut();
                if let Some(w) = j.weights.iter_mut().find(|w| w.id == id) {
                    w.sorted = !w.sorted;
                }
                let _ = j.save_weights();
            }
            // Re-draw whatever page this row sits on, once this click is done.
            let me = me.clone();
            glib::idle_add_local_once(move || {
                if let Some(book) = me.upgrade()
                    && let Some(row) = book.list.selected_row()
                {
                    let key = book.keys.borrow().get(row.index() as usize).cloned();
                    if let Some(key) = key {
                        book.render(&key);
                    }
                }
            });
        });
        let me = self.me.borrow().clone();
        course.connect_clicked(move |_| {
            if let Some(book) = me.upgrade() {
                book.request(Some(Request::Course(id)));
            }
        });
    }

    fn fixed(&self, journal: &Journal) {
        self.title("Fixed stars");
        let people = journal.fixed_stars();
        if people.is_empty() {
            self.content.append(&label(
                "The people whose names come up on more than one night will gather here.",
                "book-quiet",
            ));
            return;
        }
        for p in people {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.add_css_class("book-card");
            card.set_margin_top(12);
            card.append(&label(&p.name, "book-big"));
            if !p.stars.is_empty() {
                let names: Vec<String> = p
                    .stars
                    .iter()
                    .map(|hr| {
                        self.game
                            .borrow()
                            .star_name(*hr)
                            .unwrap_or_else(|| format!("HR {hr}"))
                    })
                    .collect();
                card.append(&label(
                    &format!("Their star: {}", names.join(", ")),
                    "book-quiet",
                ));
            }
            for m in &p.mentions {
                let Some(night) = journal.night(&m.night) else {
                    continue;
                };
                if let Some(a) = night.answers.iter().find(|a| {
                    a.question == m.question
                        && a.person
                            .as_deref()
                            .is_some_and(|n| n.eq_ignore_ascii_case(&p.name))
                }) {
                    let l = label(
                        &format!("{} · {}", short_date(&m.night), a.prompt),
                        "book-asked",
                    );
                    l.set_margin_top(4);
                    card.append(&l);
                } else if m.question == "plan" {
                    card.append(&label(
                        &format!("{} · a plan together", short_date(&m.night)),
                        "book-asked",
                    ));
                }
            }
            self.content.append(&card);
        }
    }

    fn coming(&self, journal: &Journal, today: &str) {
        self.title("Coming up");
        let mut ahead: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| days_between(today, &p.date) >= 0)
            .collect();
        ahead.sort_by(|a, b| a.date.cmp(&b.date));
        let mut past: Vec<_> = journal
            .plans
            .iter()
            .filter(|p| days_between(today, &p.date) < 0)
            .collect();
        past.sort_by(|a, b| b.date.cmp(&a.date));
        if ahead.is_empty() && past.is_empty() {
            self.content.append(&label(
                "Plans made under the sky will wait here until their night comes.",
                "book-quiet",
            ));
            return;
        }
        if !ahead.is_empty() {
            self.heading("AHEAD");
            for p in ahead {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
                card.add_css_class("book-card");
                card.set_margin_top(8);
                card.append(&label(&format!("{}{who}", p.what), "book-body"));
                card.append(&label(
                    &format!("{} · {}", long_date(&p.date), p.event),
                    "book-quiet",
                ));
                self.content.append(&card);
            }
        }
        if !past.is_empty() {
            self.heading("DONE AND GONE");
            for p in past {
                let who = p
                    .who
                    .as_deref()
                    .map(|w| format!(", with {w}"))
                    .unwrap_or_default();
                let how = match (p.outcome.as_deref(), p.note.as_deref()) {
                    (_, Some(note)) => note.to_owned(),
                    (Some("went"), None) => "It happened.".into(),
                    _ => String::new(),
                };
                let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
                card.add_css_class("book-card");
                card.set_margin_top(8);
                card.append(&label(&format!("{}{who}", p.what), "book-body"));
                card.append(&label(
                    &format!("{} · {}", short_date(&p.date), p.event),
                    "book-quiet",
                ));
                if !how.is_empty() {
                    card.append(&label(&how, "book-asked"));
                }
                self.content.append(&card);
            }
        }
    }

    fn weights(&self, journal: &Journal) {
        self.title("Recurring weights");
        let recurring: Vec<_> = journal
            .weights
            .iter()
            .filter(|w| w.nights.len() >= 3)
            .collect();
        if recurring.is_empty() {
            self.content.append(&label(
                "Weights you bring back on three or more nights gather here, with a way to chart a course if you'd like one.",
                "book-quiet",
            ));
            return;
        }
        for w in recurring {
            self.weight_row(w.id, &w.text, w.sorted);
            self.looks(w);
            let dates: Vec<String> = w.nights.iter().map(|n| short_date(n)).collect();
            self.content
                .append(&label(&dates.join(" · "), "book-quiet"));
        }
    }

    fn courses(&self, journal: &Journal) {
        self.title("Courses");
        let started = |c: &&night_sky_core::journal::Course| {
            !c.wish.is_empty() || !c.outcome.is_empty() || !c.obstacle.is_empty()
        };
        let courses: Vec<_> = journal.courses.iter().filter(started).collect();
        if courses.is_empty() {
            self.content.append(&label(
                "Any weight in the logbook has a Chart a course button. Courses you chart will be kept here.",
                "book-quiet",
            ));
            return;
        }
        // Newest first; ones still being charted at the top.
        let mut courses = courses;
        courses.sort_by_key(|c| (!c.draft, std::cmp::Reverse(c.id)));
        for c in courses {
            let weight = journal.weight(c.weight).map(|w| w.text.as_str());
            let card = crate::course::course_card(c, weight);
            card.set_margin_top(12);
            if c.draft {
                card.prepend(&label("STILL BEING CHARTED", "course-step"));
            }
            if !c.checks.is_empty() {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                row.set_margin_top(10);
                row.append(&label("HOW IT'S GONE", "course-step"));
                for k in &c.checks {
                    row.append(&label(
                        &format!("{} · {}", short_date(&k.night), k.answer),
                        "book-asked",
                    ));
                }
                card.append(&row);
            }
            let button = gtk::Button::with_label(if c.draft {
                "Carry on charting"
            } else {
                "Change the plan"
            });
            button.add_css_class("quiet");
            button.set_halign(gtk::Align::Start);
            button.set_margin_top(10);
            let me = self.me.borrow().clone();
            let weight_id = c.weight;
            button.connect_clicked(move |_| {
                if let Some(book) = me.upgrade() {
                    book.request(Some(Request::Course(weight_id)));
                }
            });
            card.append(&button);
            self.content.append(&card);
        }
    }
}
