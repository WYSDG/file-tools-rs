//! Small non-blocking notifications ("toasts") shown at the bottom of the window.
//! A toast can carry action buttons (e.g. a ✓ / ✕ confirmation such as "Overwrite?").

use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub struct Toast {
    revealer: gtk::Revealer,
    label: gtk::Label,
    actions: gtk::Box,
    generation: Cell<u64>,
    _keep: RefCell<Vec<Rc<dyn Fn()>>>,
}

impl Toast {
    pub fn new() -> Rc<Self> {
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        revealer.set_transition_duration(160);
        revealer.set_halign(gtk::Align::Center);
        revealer.set_valign(gtk::Align::End);
        revealer.set_margin_bottom(46);
        revealer.set_can_target(true);

        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        bx.add_css_class("toast");
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        label.set_max_width_chars(60);
        label.set_xalign(0.0);
        bx.append(&label);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bx.append(&actions);
        revealer.set_child(Some(&bx));

        Rc::new(Self { revealer, label, actions, generation: Cell::new(0), _keep: RefCell::new(Vec::new()) })
    }

    pub fn widget(&self) -> &gtk::Revealer {
        &self.revealer
    }

    /// Plain message that disappears by itself.
    pub fn say(self: &Rc<Self>, msg: &str) {
        self.show(msg, Vec::new(), 3);
    }

    /// Message with action buttons `(label, callback)`. Clicking any button dismisses the toast.
    pub fn show(self: &Rc<Self>, msg: &str, actions: Vec<(String, Rc<dyn Fn()>)>, secs: u32) {
        let gen = self.generation.get() + 1;
        self.generation.set(gen);
        self.label.set_text(msg);
        while let Some(c) = self.actions.first_child() {
            self.actions.remove(&c);
        }
        for (text, cb) in actions {
            let b = gtk::Button::with_label(&text);
            b.add_css_class("toast-btn");
            let me = self.clone();
            b.connect_clicked(move |_| {
                me.hide();
                cb();
            });
            self.actions.append(&b);
        }
        self.revealer.set_reveal_child(true);
        let me = self.clone();
        gtk::glib::timeout_add_local_once(Duration::from_secs(secs.max(1) as u64), move || {
            // Only hide if no newer toast replaced this one.
            if me.generation.get() == gen {
                me.revealer.set_reveal_child(false);
            }
        });
    }

    pub fn hide(&self) {
        self.generation.set(self.generation.get() + 1);
        self.revealer.set_reveal_child(false);
    }
}
