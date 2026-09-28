//! The Tonight list: each of the night's finds, what kind of thing it is and
//! where to look for it now, kept up to date as the sky turns. A click turns
//! the view towards one.

use crate::game::{Game, Row};
use crate::ui::{clear, label, wall_clock};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Tonight {
    pub root: gtk::Box,
    rows: gtk::Box,
    shown: RefCell<Vec<Row>>,
}

impl Tonight {
    pub fn new() -> Rc<Tonight> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.add_css_class("tonight");
        root.set_halign(gtk::Align::End);
        root.set_valign(gtk::Align::Start);
        root.set_margin_top(14);
        root.set_margin_end(14);
        root.set_width_request(270);
        let title = label("TONIGHT", "tonight-title");
        let hint = label(
            "Click one, or press Tab, to turn towards it",
            "tonight-hint",
        );
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 1);
        // A long night's list scrolls rather than running off the screen.
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(560);
        scroll.set_child(Some(&rows));
        root.append(&title);
        root.append(&scroll);
        root.append(&hint);
        root.set_visible(false);
        Rc::new(Tonight {
            root,
            rows,
            shown: RefCell::new(Vec::new()),
        })
    }

    /// Refreshes the list from the game; cheap when nothing changed.
    pub fn sync(&self, game: &Rc<RefCell<Game>>) {
        let (visible, rows) = {
            let g = game.borrow();
            (g.show_tonight(), g.tonight_rows(wall_clock()))
        };
        self.root.set_visible(visible);
        if !visible || *self.shown.borrow() == rows {
            return;
        }
        clear(&self.rows);
        for (i, row) in rows.iter().enumerate() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let mark = gtk::Label::new(Some(if row.found { "●" } else { "○" }));
            mark.add_css_class("find-mark");
            mark.set_valign(gtk::Align::Start);
            let words = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let name = label(&row.name, "find-name");
            name.set_max_width_chars(28);
            let detail = if row.found {
                format!("{} · found", row.kind)
            } else {
                format!("{} · {}", row.kind, row.whereabouts)
            };
            words.append(&name);
            // Found things take one line, to leave room for the rest.
            if !row.found {
                words.append(&label(&detail, "find-where"));
            } else {
                name.set_tooltip_text(Some(&detail));
            }
            line.append(&mark);
            line.append(&words);
            let button = gtk::Button::new();
            button.set_child(Some(&line));
            button.add_css_class("find-row");
            button.set_focus_on_click(false);
            if row.found {
                button.add_css_class("found");
            }
            let game = game.clone();
            button.connect_clicked(move |_| game.borrow_mut().turn_to(i, wall_clock()));
            self.rows.append(&button);
        }
        *self.shown.borrow_mut() = rows;
    }
}

/// The evening's three parts at the top left, the one it's at lit, and a
/// line saying what to do now. Wind down can be chosen from here.
pub struct EveningGuide {
    pub root: gtk::Box,
    steps: [gtk::Widget; 3],
    now: gtk::Label,
    shown: RefCell<Option<crate::game::Evening>>,
}

impl EveningGuide {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<EveningGuide> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.add_css_class("evening");
        root.set_halign(gtk::Align::Start);
        root.set_valign(gtk::Align::Start);
        root.set_margin_start(66);
        root.set_margin_top(12);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let set_down = label("Set it down", "evening-step");
        let look_up = label("Look up", "evening-step");
        for l in [&set_down, &look_up] {
            l.set_wrap(false);
        }
        let wind = gtk::Button::with_label("Wind down");
        wind.add_css_class("evening-step");
        wind.add_css_class("evening-button");
        wind.set_focus_on_click(false);
        wind.set_tooltip_text(Some("W"));
        {
            let game = game.clone();
            wind.connect_clicked(move |_| game.borrow_mut().wind_down(wall_clock()));
        }
        let dot = || {
            let d = label("·", "evening-dot");
            d.set_wrap(false);
            d
        };
        row.append(&set_down);
        row.append(&dot());
        row.append(&look_up);
        row.append(&dot());
        row.append(&wind);
        let now = label("", "evening-now");
        now.set_max_width_chars(40);
        now.set_width_chars(34);
        root.append(&row);
        root.append(&now);
        root.set_visible(false);
        Rc::new(EveningGuide {
            root,
            steps: [set_down.upcast(), look_up.upcast(), wind.upcast()],
            now,
            shown: RefCell::new(None),
        })
    }

    /// Follows the game; cheap when nothing changed.
    pub fn sync(&self, game: &Rc<RefCell<Game>>) {
        let evening = game.borrow().evening(wall_clock());
        if *self.shown.borrow() == evening {
            return;
        }
        match &evening {
            None => self.root.set_visible(false),
            Some(e) => {
                self.root.set_visible(true);
                for (k, w) in self.steps.iter().enumerate() {
                    w.remove_css_class("current");
                    w.remove_css_class("done");
                    if k == e.step {
                        w.add_css_class("current");
                    } else if k < e.step {
                        w.add_css_class("done");
                    }
                }
                self.steps[2].set_sensitive(e.step == 1);
                self.now.set_text(&e.now);
            }
        }
        *self.shown.borrow_mut() = evening;
    }
}
