//! The window's own widgets: the prompt at the foot of the sky, and the
//! style shared by every page.

use crate::game::Game;
use crate::talk::Prompt;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const CSS: &str = r#"
window, .page { background: #070913; color: #dfe3ee; }
.prompt {
  background: rgba(7, 9, 19, 0.78);
  border-radius: 16px;
  padding: 18px 22px 14px 22px;
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.prompt-text { font-size: 17px; color: #eef0f6; }
.prompt entry {
  background: rgba(255, 255, 255, 0.05);
  color: #f2f4f9;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  min-height: 38px;
  font-size: 15px;
  box-shadow: none;
  outline: none;
}
.prompt entry:focus-within { border-color: rgba(240, 214, 168, 0.55); }
button.chip {
  background: rgba(255, 255, 255, 0.06);
  border-radius: 16px;
  color: #dfe3ee;
  padding: 3px 13px;
  border: none;
  box-shadow: none;
  font-size: 14px;
}
button.chip:hover, button.chip:focus { background: rgba(240, 214, 168, 0.16); }
button.name { font-size: 13px; padding: 1px 10px; }
.hint { font-size: 12px; color: rgba(220, 225, 240, 0.48); }
.book-side { background: rgba(255, 255, 255, 0.025); }
.book-side row { padding: 7px 18px; color: #c9d0e2; font-size: 14px; }
.book-side row:selected { background: rgba(240, 214, 168, 0.12); color: #f6efdd; }
.book-side .section { font-size: 11px; color: rgba(220, 225, 240, 0.45); padding: 16px 18px 4px 18px; }
.book-title { font-size: 26px; color: #f3ecd9; }
.book-heading { font-size: 12px; color: rgba(210, 218, 240, 0.6); margin-top: 18px; letter-spacing: 1px; }
.book-body { font-size: 15px; color: #dfe3ee; }
.book-asked { font-size: 14px; color: rgba(210, 218, 240, 0.7); font-style: italic; }
.book-quiet { font-size: 13px; color: rgba(220, 225, 240, 0.5); }
.book-big { font-size: 21px; color: #f3ecd9; }
.menu-card {
  background: rgba(255, 255, 255, 0.035);
  border-radius: 12px;
  margin-top: 4px;
}
.menu-row { padding: 12px 18px; border-bottom: 1px solid rgba(255, 255, 255, 0.04); }
.menu-card > .menu-row:last-child { border-bottom: none; }
.menu-count { font-size: 15px; color: rgba(240, 214, 168, 0.85); min-width: 32px; }
.menu-pillar { font-size: 15px; color: #f3ecd9; font-weight: 600; }
.book-card {
  background: rgba(255, 255, 255, 0.035);
  border-radius: 12px;
  padding: 14px 18px;
}
button.quiet {
  background: rgba(255, 255, 255, 0.05);
  color: #d6dcec;
  border: none;
  box-shadow: none;
  border-radius: 8px;
  padding: 3px 12px;
  font-size: 13px;
}
button.quiet:hover { background: rgba(240, 214, 168, 0.14); }
button.menu-button {
  background: rgba(7, 9, 19, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 10px;
  padding: 7px;
  color: rgba(236, 238, 246, 0.85);
  box-shadow: none;
}
button.menu-button:hover { background: rgba(240, 214, 168, 0.16); }
.tonight {
  background: rgba(7, 9, 19, 0.74);
  border-radius: 14px;
  padding: 12px 10px 10px 10px;
  border: 1px solid rgba(255, 255, 255, 0.06);
}
.tonight-title { font-size: 11px; font-weight: 600; letter-spacing: 2px; color: rgba(240, 214, 168, 0.8); margin: 0 8px 4px 8px; }
.tonight-hint { font-size: 11px; color: rgba(220, 225, 240, 0.42); margin: 6px 8px 0 8px; }
button.find-row {
  background: none;
  border: none;
  box-shadow: none;
  border-radius: 9px;
  padding: 5px 8px;
}
button.find-row:hover { background: rgba(240, 214, 168, 0.1); }
button.find-row.found { opacity: 0.5; }
.find-mark { color: rgba(240, 214, 168, 0.9); font-size: 12px; margin-top: 2px; }
.find-name { font-size: 14px; color: #eef0f6; }
.find-where { font-size: 12px; color: rgba(210, 218, 240, 0.6); }
.course-step { font-size: 12px; color: rgba(210, 218, 240, 0.55); letter-spacing: 1px; }
.course-ask { font-size: 22px; color: #f3ecd9; }
.course entry, .settings entry {
  background: rgba(255, 255, 255, 0.05);
  color: #f2f4f9;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  min-height: 40px;
  font-size: 16px;
}
"#;

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn wall_clock() -> i64 {
    glib::real_time() / 1000
}

/// Removes every child of a box.
pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Takes a widget out of whatever box it sits in, so it can go somewhere else.
pub fn detach(widget: &impl IsA<gtk::Widget>) {
    if let Some(parent) = widget.parent().and_downcast::<gtk::Box>() {
        parent.remove(widget);
    }
}

pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class(class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_xalign(0.0);
    l.set_selectable(false);
    l.set_max_width_chars(72);
    l
}

pub struct PromptBar {
    pub root: gtk::Box,
    text: gtk::Label,
    chips: gtk::FlowBox,
    entry: gtk::Entry,
    names: gtk::Box,
    hint: gtk::Label,
    serial: Cell<u64>,
    showing: RefCell<Option<Prompt>>,
}

impl PromptBar {
    pub fn new(game: &Rc<RefCell<Game>>) -> Rc<PromptBar> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("prompt");
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::End);
        root.set_margin_bottom(84);
        root.set_width_request(560);
        root.set_visible(false);
        let text = label("", "prompt-text");
        text.set_max_width_chars(60);
        let chips = gtk::FlowBox::new();
        chips.set_selection_mode(gtk::SelectionMode::None);
        chips.set_max_children_per_line(3);
        chips.set_column_spacing(6);
        chips.set_row_spacing(6);
        chips.set_homogeneous(false);
        let entry = gtk::Entry::new();
        let names = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let hint = label("", "hint");
        root.append(&text);
        root.append(&entry);
        root.append(&names);
        root.append(&chips);
        root.append(&hint);
        let bar = Rc::new(PromptBar {
            root,
            text,
            chips,
            entry,
            names,
            hint,
            serial: Cell::new(0),
            showing: RefCell::new(None),
        });
        {
            let game = game.clone();
            bar.entry.connect_activate(move |entry| {
                let text = entry.text().to_string();
                entry.set_text("");
                game.borrow_mut().answer_text(&text, wall_clock());
            });
        }
        {
            let game = game.clone();
            let weak = Rc::downgrade(&bar);
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    game.borrow_mut().skip_prompt(wall_clock());
                    return glib::Propagation::Stop;
                }
                // With nothing typed yet, a number picks a chip.
                if let Some(bar) = weak.upgrade()
                    && bar.entry.text().is_empty()
                    && let Some(n) = key.to_unicode().and_then(|c| c.to_digit(10))
                {
                    let chips = bar.showing.borrow().as_ref().map_or(0, |p| p.chips.len());
                    if (1..=chips as u32).contains(&n) {
                        game.borrow_mut().answer_chip(n as usize - 1, wall_clock());
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            });
            bar.entry.add_controller(keys);
        }
        {
            let game = game.clone();
            let weak = Rc::downgrade(&bar);
            bar.entry.connect_changed(move |entry| {
                if let Some(bar) = weak.upgrade() {
                    bar.suggest(&game, &entry.text());
                }
            });
        }
        bar
    }

    fn suggest(&self, game: &Rc<RefCell<Game>>, typed: &str) {
        clear(&self.names);
        let wants = self.showing.borrow().as_ref().is_some_and(|p| p.names);
        if !wants {
            return;
        }
        let Ok(g) = game.try_borrow() else { return };
        for name in g.names_like(typed) {
            let b = gtk::Button::with_label(&name);
            b.add_css_class("chip");
            b.add_css_class("name");
            let entry = self.entry.clone();
            b.connect_clicked(move |_| {
                entry.set_text(&name);
                entry.set_position(-1);
                entry.grab_focus();
            });
            self.names.append(&b);
        }
    }

    /// Shows the game's current prompt, if it has changed. Returns true when
    /// the prompt went away, so the sky can take the keyboard back.
    pub fn sync(&self, game: &Rc<RefCell<Game>>) -> bool {
        let (serial, prompt) = game.borrow().prompt();
        if serial == self.serial.get() {
            return false;
        }
        self.serial.set(serial);
        clear(&self.names);
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        let Some(p) = prompt else {
            self.root.set_visible(false);
            *self.showing.borrow_mut() = None;
            return true;
        };
        self.text.set_text(&p.text);
        self.entry.set_visible(p.entry);
        self.entry.set_text("");
        self.entry.set_placeholder_text(Some(&p.placeholder));
        for (i, chip) in p.chips.iter().enumerate() {
            let b = gtk::Button::with_label(&format!("{}  {chip}", i + 1));
            b.add_css_class("chip");
            let game = game.clone();
            b.connect_clicked(move |_| game.borrow_mut().answer_chip(i, wall_clock()));
            self.chips.insert(&b, -1);
        }
        self.chips.set_visible(!p.chips.is_empty());
        let hint = match (p.entry, p.chips.len()) {
            (false, n) if n > 0 && p.hint.is_empty() => format!("Press 1 to {n}, or click"),
            (true, n) if n > 0 => format!("{} · a number picks one below", p.hint),
            _ => p.hint.clone(),
        };
        self.hint.set_text(&hint);
        self.hint.set_visible(!hint.is_empty());
        self.root.set_visible(true);
        if p.entry {
            self.entry.grab_focus();
        }
        *self.showing.borrow_mut() = Some(p);
        false
    }
}
