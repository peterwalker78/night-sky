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
        root.append(&title);
        root.append(&rows);
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
            words.append(&label(&detail, "find-where"));
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
