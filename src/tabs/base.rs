use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::app::AppState;

#[derive(Clone)]
pub enum PresetWidget {
    Slider(gtk::Scale),
    DropDown(gtk::DropDown),
    Switch(gtk::Switch),
    Toggle(gtk::ToggleButton),
    Color(gtk::ColorDialogButton),
    Entry(gtk::Entry),
}

/// A Rust-side render job: `(input, scale) -> output path` (`scale` = input size / real size).
pub type RustJob = Box<dyn FnOnce(&std::path::Path, f64) -> Result<PathBuf, String> + Send>;

#[derive(Clone)]
pub struct BaseTab {
    /// The full right-side inspector content (header + controls + process + status).
    pub inspector: gtk::Box,
    /// While true, control changes do not schedule live previews (used when restoring a step's settings).
    pub suppress_live: Rc<Cell<bool>>,
    pub controls: gtk::Box,
    pub status_lbl: gtk::Label,
    pub spinner: gtk::Spinner,
    pub go: gtk::Button,
    pub reset: gtk::Button,
    pub input_path: Rc<RefCell<Option<PathBuf>>>,
    pub action_name: String,
    pub save_subdir: Option<String>,
    pub sliders: Rc<RefCell<Vec<(gtk::Label, gtk::Scale, f64)>>>,
    pub preset_widgets: Rc<RefCell<Vec<(String, PresetWidget)>>>,
    pub app: AppState,
    live_preview: Rc<Cell<bool>>,
    live_delay_ms: Rc<Cell<u64>>,
    live_trigger: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    live_timer: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    processing: Rc<Cell<bool>>,
}

impl BaseTab {
    pub fn new(action_name: &str, save_subdir: Option<&str>, title: &str, app: AppState) -> Self {
        // ---- inspector root ----
        let inspector = gtk::Box::new(gtk::Orientation::Vertical, 8);
        inspector.set_margin_top(12);
        inspector.set_margin_bottom(12);
        inspector.set_margin_start(12);
        inspector.set_margin_end(12);

        // ---- header: title + Presets + Reset ----
        let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let safe_title = title
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let header = gtk::Label::new(Some(&format!("<b>{}</b>", safe_title)));
        header.set_use_markup(true);
        header.set_xalign(0.0);
        header.set_hexpand(true);
        header_row.append(&header);

        let presets_btn = gtk::MenuButton::new();
        presets_btn.set_label("Presets ▾");
        presets_btn.set_tooltip_text(Some("Save or load preset values"));
        presets_btn.add_css_class("flat");
        header_row.append(&presets_btn);

        let reset = gtk::Button::with_label("↺");
        reset.set_tooltip_text(Some("Reset sliders to defaults"));
        reset.add_css_class("flat");
        header_row.append(&reset);
        inspector.append(&header_row);

        // ---- controls area (sub-tabs append here) ----
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 6);
        inspector.append(&controls);

        // ---- process button ----
        let act_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let go = gtk::Button::with_label("Process (Ctrl+Enter)");
        go.add_css_class("suggested-action");
        go.set_sensitive(false);
        go.set_hexpand(true);
        act_row.append(&go);
        inspector.append(&act_row);

        // ---- status ----
        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        status_row.set_margin_top(4);
        status_row.set_margin_bottom(4);
        let spinner = gtk::Spinner::new();
        spinner.set_visible(false);
        status_row.append(&spinner);
        let status_lbl = gtk::Label::new(None);
        status_lbl.set_xalign(0.0);
        status_lbl.set_wrap(true);
        status_lbl.set_selectable(true);
        status_lbl.set_hexpand(true);
        status_row.append(&status_lbl);
        inspector.append(&status_row);

        let sliders: Rc<RefCell<Vec<(gtk::Label, gtk::Scale, f64)>>> =
            Rc::new(RefCell::new(Vec::new()));
        {
            let sliders_c = sliders.clone();
            reset.connect_clicked(move |_| {
                for (_, s, d) in sliders_c.borrow().iter() {
                    s.set_value(*d);
                }
            });
        }

        let preset_widgets: Rc<RefCell<Vec<(String, PresetWidget)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let base = Self {
            inspector,
            controls,
            status_lbl,
            spinner,
            go,
            reset,
            input_path: Rc::new(RefCell::new(None)),
            action_name: action_name.to_string(),
            save_subdir: save_subdir.map(|s| s.to_string()),
            sliders,
            preset_widgets,
            app,
            live_preview: Rc::new(Cell::new(false)),
            live_delay_ms: Rc::new(Cell::new(400)),
            live_trigger: Rc::new(RefCell::new(None)),
            live_timer: Rc::new(RefCell::new(None)),
            processing: Rc::new(Cell::new(false)),
            suppress_live: Rc::new(Cell::new(false)),
        };

        // ---- Presets popover ----
        {
            let popover = gtk::Popover::new();
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
            vbox.set_margin_top(8);
            vbox.set_margin_bottom(8);
            vbox.set_margin_start(8);
            vbox.set_margin_end(8);
            vbox.set_size_request(300, -1);

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("preset name"));
            vbox.append(&entry);

            let save_btn = gtk::Button::with_label("Save current as preset");
            save_btn.set_hexpand(true);
            vbox.append(&save_btn);
            vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            let hdr = gtk::Label::new(Some("Saved presets"));
            hdr.set_xalign(0.0);
            hdr.add_css_class("section-label");
            vbox.append(&hdr);

            let list = gtk::ListBox::new();
            list.set_selection_mode(gtk::SelectionMode::None);
            let list_scroll = gtk::ScrolledWindow::new();
            list_scroll.set_vexpand(true);
            list_scroll.set_min_content_height(140);
            list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            list_scroll.set_child(Some(&list));
            vbox.append(&list_scroll);

            popover.set_child(Some(&vbox));
            presets_btn.set_popover(Some(&popover));

            // Save
            {
                let base_c = base.clone();
                let pop_c = popover.clone();
                let entry_c = entry.clone();
                save_btn.connect_clicked(move |_| {
                    let name = entry_c.text().to_string();
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        return;
                    }
                    let data = base_c.get_preset_data();
                    base_c
                        .app
                        .presets
                        .borrow_mut()
                        .set(&base_c.action_name, &name, data);
                    base_c.app.presets.borrow().save();
                    base_c.set_status(&format!("Saved preset '{}'", name), false);
                    entry_c.set_text("");
                    pop_c.popdown();
                });
            }

            // Rebuild on show
            {
                let base_c = base.clone();
                let list_c = list.clone();
                let pop_c = popover.clone();
                popover.connect_show(move |_| {
                    while let Some(c) = list_c.first_child() {
                        list_c.remove(&c);
                    }

                    let names = base_c.app.presets.borrow().list(&base_c.action_name);
                    if names.is_empty() {
                        let empty = gtk::Label::new(Some("(no saved presets)"));
                        empty.set_xalign(0.0);
                        empty.set_margin_top(8);
                        empty.set_margin_bottom(8);
                        empty.add_css_class("section-label");
                        list_c.append(&empty);
                        return;
                    }

                    for preset_name in names {
                        let row = gtk::ListBoxRow::new();
                        row.set_activatable(false);

                        let h = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                        h.set_margin_start(6);
                        h.set_margin_end(6);
                        h.set_margin_top(4);
                        h.set_margin_bottom(4);

                        let lbl = gtk::Label::new(Some(&preset_name));
                        lbl.set_xalign(0.0);
                        lbl.set_hexpand(true);
                        lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        lbl.set_max_width_chars(24);
                        lbl.set_tooltip_text(Some(&preset_name));
                        h.append(&lbl);

                        let load_btn = gtk::Button::with_label("Load");
                        load_btn.add_css_class("flat");
                        load_btn.set_valign(gtk::Align::Center);
                        h.append(&load_btn);

                        let del_btn = gtk::Button::with_label("×");
                        del_btn.add_css_class("flat");
                        del_btn.set_valign(gtk::Align::Center);
                        del_btn.set_tooltip_text(Some("Delete this preset"));
                        h.append(&del_btn);

                        row.set_child(Some(&h));
                        list_c.append(&row);

                        {
                            let base_l = base_c.clone();
                            let pn = preset_name.clone();
                            let pop_l = pop_c.clone();
                            load_btn.connect_clicked(move |_| {
                                let data = base_l
                                    .app
                                    .presets
                                    .borrow()
                                    .get(&base_l.action_name, &pn)
                                    .cloned();
                                if let Some(v) = data {
                                    base_l.apply_preset_data(&v);
                                    base_l.set_status(&format!("Loaded preset '{}'", pn), false);
                                    base_l.schedule_live_process();
                                }
                                pop_l.popdown();
                            });
                        }
                        {
                            let base_d = base_c.clone();
                            let pn = preset_name.clone();
                            let pop_d = pop_c.clone();
                            del_btn.connect_clicked(move |_| {
                                base_d
                                    .app
                                    .presets
                                    .borrow_mut()
                                    .delete(&base_d.action_name, &pn);
                                base_d.app.presets.borrow().save();
                                pop_d.popdown();
                            });
                        }
                    }
                });
            }
        }

        base
    }

    // ---- Preset API ----
    pub fn register_preset(&self, name: &str, w: PresetWidget) {
        self.preset_widgets.borrow_mut().push((name.to_string(), w));
    }
    pub fn register_slider(&self, name: &str, s: &gtk::Scale) {
        self.register_preset(name, PresetWidget::Slider(s.clone()));
    }
    pub fn register_dropdown(&self, name: &str, d: &gtk::DropDown) {
        self.register_preset(name, PresetWidget::DropDown(d.clone()));
    }
    pub fn register_switch(&self, name: &str, s: &gtk::Switch) {
        self.register_preset(name, PresetWidget::Switch(s.clone()));
    }
    pub fn register_toggle(&self, name: &str, t: &gtk::ToggleButton) {
        self.register_preset(name, PresetWidget::Toggle(t.clone()));
    }
    pub fn register_color(&self, name: &str, c: &gtk::ColorDialogButton) {
        self.register_preset(name, PresetWidget::Color(c.clone()));
    }
    pub fn register_entry(&self, name: &str, e: &gtk::Entry) {
        self.register_preset(name, PresetWidget::Entry(e.clone()));
    }

    pub fn get_preset_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (name, w) in self.preset_widgets.borrow().iter() {
            let v = match w {
                PresetWidget::Slider(s) => serde_json::json!(s.value()),
                PresetWidget::DropDown(d) => serde_json::json!(d.selected()),
                PresetWidget::Switch(s) => serde_json::json!(s.is_active()),
                PresetWidget::Toggle(t) => serde_json::json!(t.is_active()),
                PresetWidget::Color(c) => {
                    let r = c.rgba();
                    serde_json::json!({ "r": r.red(), "g": r.green(), "b": r.blue() })
                }
                PresetWidget::Entry(e) => serde_json::json!(e.text().to_string()),
            };
            map.insert(name.clone(), v);
        }
        serde_json::Value::Object(map)
    }

    /// Restore saved settings without triggering a re-render.
    pub fn apply_preset_quiet(&self, data: &serde_json::Value) {
        self.suppress_live.set(true);
        self.apply_preset_data(data);
        self.suppress_live.set(false);
    }

    pub fn apply_preset_data(&self, data: &serde_json::Value) {
        let Some(map) = data.as_object() else { return };
        for (name, w) in self.preset_widgets.borrow().iter() {
            let Some(v) = map.get(name) else { continue };
            match w {
                PresetWidget::Slider(s) => {
                    if let Some(n) = v.as_f64() {
                        s.set_value(n);
                    }
                }
                PresetWidget::DropDown(d) => {
                    if let Some(n) = v.as_u64() {
                        d.set_selected(n as u32);
                    }
                }
                PresetWidget::Switch(s) => {
                    if let Some(b) = v.as_bool() {
                        s.set_active(b);
                    }
                }
                PresetWidget::Toggle(t) => {
                    if let Some(b) = v.as_bool() {
                        t.set_active(b);
                    }
                }
                PresetWidget::Color(c) => {
                    if let (Some(r), Some(g), Some(b)) = (
                        v.get("r").and_then(|x| x.as_f64()),
                        v.get("g").and_then(|x| x.as_f64()),
                        v.get("b").and_then(|x| x.as_f64()),
                    ) {
                        c.set_rgba(&gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, 1.0));
                    }
                }
                PresetWidget::Entry(e) => {
                    if let Some(s) = v.as_str() {
                        e.set_text(s);
                    }
                }
            }
        }
    }

    // ---- live preview ----
    pub fn set_live_preview(&self, enabled: bool, delay_ms: u64) {
        self.live_preview.set(enabled);
        self.live_delay_ms.set(delay_ms);
    }
    pub fn set_live_trigger(&self, f: Rc<dyn Fn()>) {
        // Tag everything started by the trigger as "live" (see `AppState::commit_step`).
        let app = self.app.clone();
        *self.live_trigger.borrow_mut() = Some(Rc::new(move || {
            app.live_run.set(true);
            f();
            app.live_run.set(false);
        }));
    }
    pub fn schedule_live_process(&self) {
        if self.suppress_live.get() {
            return;
        }
        Self::schedule_impl(
            &self.live_preview,
            &self.live_timer,
            &self.live_trigger,
            &self.processing,
            &self.live_delay_ms,
        );
    }
    fn schedule_impl(
        live_preview: &Rc<Cell<bool>>,
        live_timer: &Rc<RefCell<Option<gtk::glib::SourceId>>>,
        live_trigger: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
        processing: &Rc<Cell<bool>>,
        live_delay: &Rc<Cell<u64>>,
    ) {
        if !live_preview.get() {
            return;
        }
        let trigger = match live_trigger.borrow().clone() {
            Some(t) => t,
            None => return,
        };
        if let Some(id) = live_timer.borrow_mut().take() {
            id.remove();
        }
        let timer_c = live_timer.clone();
        let processing_c = processing.clone();
        let delay = live_delay.get();
        let id = gtk::glib::timeout_add_local(Duration::from_millis(delay), move || {
            // A job is still running: keep this timer alive and retry next tick,
            // so the latest slider value is never silently dropped.
            if processing_c.get() {
                return gtk::glib::ControlFlow::Continue;
            }
            *timer_c.borrow_mut() = None;
            trigger();
            gtk::glib::ControlFlow::Break
        });
        *live_timer.borrow_mut() = Some(id);
    }

    // ---- shared ----
    pub fn set_status(&self, msg: &str, is_err: bool) {
        self.status_lbl.set_text(msg);
        self.status_lbl.remove_css_class("status-ok");
        self.status_lbl.remove_css_class("status-err");
        if !msg.is_empty() {
            self.status_lbl
                .add_css_class(if is_err { "status-err" } else { "status-ok" });
        }
    }

    pub fn set_input(&self, path: Option<PathBuf>, _display: Option<String>) {
        *self.input_path.borrow_mut() = path.clone();
        if path.is_some() {
            self.go.set_sensitive(true);
        } else {
            self.go.set_sensitive(false);
        }
    }

    /// Live preview for tabs whose commands can be rebuilt for any input size.
    ///
    /// `build(input, scale, full)` returns the commands plus the output path. `scale` is how much
    /// smaller the input is than the original (1.0 = full size; use it for pixel-valued
    /// parameters). Big images are previewed on a downscaled copy and the full-resolution
    /// commit is deferred until something needs it (`AppState::flush_pending`); small images
    /// keep the simple direct-commit behaviour.
    pub fn run_live_proxy(
        &self,
        app: &AppState,
        build: Rc<dyn Fn(&std::path::Path, f64, bool) -> (Vec<Vec<String>>, PathBuf)>,
    ) {
        let Some(input) = self.get_input() else {
            self.set_status("Load an image first", true);
            return;
        };
        match crate::util::preview_proxy(&input) {
            None => {
                let (cmds, out) = build(&input, 1.0, true);
                self.clone().spawn_processing(app.clone(), input, out, cmds);
            }
            Some((proxy, scale)) => {
                let (cmds, out) = build(&proxy, scale, false);
                self.clone().spawn_processing_preview(app.clone(), proxy, out, cmds);

                let (base, app2) = (self.clone(), app.clone());
                app.set_pending_commit(Rc::new(move || {
                    let (cmds, out) = build(&input, 1.0, true);
                    base.set_status("Finalizing…", false);
                    for cmd in &cmds {
                        match Command::new(&cmd[0]).args(&cmd[1..]).output() {
                            Ok(o) if o.status.success() => {}
                            Ok(o) => {
                                base.set_status(&String::from_utf8_lossy(&o.stderr), true);
                                return;
                            }
                            Err(e) => {
                                base.set_status(&format!("spawn error: {}", e), true);
                                return;
                            }
                        }
                    }
                    base.set_status("Done.", false);
                    app2.commit_step(&base.action_name, out, base.get_preset_data(), true);
                }));
            }
        }
    }

    /// Like `run_live_proxy`, but for tools that render in Rust instead of via ImageMagick.
    /// `make(full)` reads the controls and returns a job `(input, scale) -> output path`.
    /// Big images are previewed on a downscaled copy; the full-resolution commit is deferred.
    pub fn run_live_rust(&self, app: &AppState, make: Rc<dyn Fn(bool) -> RustJob>) {
        let Some(input) = self.get_input() else {
            self.set_status("Load an image first", true);
            return;
        };
        match crate::util::preview_proxy(&input) {
            None => {
                let job = make(true);
                self.clone().spawn_work(
                    app.clone(),
                    Box::new(move || job(&input, 1.0)),
                    None,
                    true,
                );
            }
            Some((proxy, scale)) => {
                let job = make(false);
                self.clone()
                    .spawn_work(app.clone(), Box::new(move || job(&proxy, scale)), None, false);

                let (base, app2) = (self.clone(), app.clone());
                app.set_pending_commit(Rc::new(move || {
                    base.set_status("Finalizing…", false);
                    let job = make(true);
                    match job(&input, 1.0) {
                        Ok(out) => {
                            base.set_status("Done.", false);
                            app2.commit_step(&base.action_name, out, base.get_preset_data(), true);
                        }
                        Err(e) => base.set_status(&e, true),
                    }
                }));
            }
        }
    }

    /// Explicit "Process" for Rust-rendered tools: full resolution, committed.
    pub fn run_rust_full(&self, app: &AppState, make: Rc<dyn Fn(bool) -> RustJob>) {
        let Some(input) = self.get_input() else {
            self.set_status("Load an image first", true);
            return;
        };
        let job = make(true);
        self.clone()
            .spawn_work(app.clone(), Box::new(move || job(&input, 1.0)), None, true);
    }

    /// A job for this tool is currently running.
    pub fn is_busy(&self) -> bool {
        self.processing.get()
    }

    pub fn get_input(&self) -> Option<PathBuf> {
        self.input_path.borrow().clone()
    }

    pub fn save_dir(&self) -> PathBuf {
        match &self.save_subdir {
            Some(s) => crate::config::sub_dir(s),
            None => crate::config::save_dir(),
        }
    }

    pub fn tracked_slider(&self, label: &str, lo: f64, hi: f64, default: f64) -> gtk::Scale {
        let lbl = gtk::Label::new(Some(&format!("{}: {}", label, default as i64)));
        lbl.set_xalign(0.0);

        let s = gtk::Scale::with_range(gtk::Orientation::Horizontal, lo, hi, 1.0);
        s.set_value(default);
        s.set_round_digits(0);

        // A plain scroll over a slider scrolls the page instead of moving the slider (Ctrl+scroll
        // still adjusts it). Otherwise a trackpad swipe over a slider gets "stuck" on it.
        {
            let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
            sc.set_propagation_phase(gtk::PropagationPhase::Capture);
            sc.connect_scroll(|ctrl, _dx, dy| {
                if ctrl.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    return gtk::glib::Propagation::Proceed;
                }
                if let Some(w) = ctrl.widget() {
                    if let Some(sw) = w
                        .ancestor(gtk::ScrolledWindow::static_type())
                        .and_then(|a| a.downcast::<gtk::ScrolledWindow>().ok())
                    {
                        let adj = sw.vadjustment();
                        adj.set_value(adj.value() + dy * adj.step_increment().max(24.0));
                    }
                }
                gtk::glib::Propagation::Stop
            });
            s.add_controller(sc);
        }

        let lbl_c = lbl.clone();
        let sc_c = s.clone();
        let ls = label.to_string();

        let lp = self.live_preview.clone();
        let lt = self.live_timer.clone();
        let ltr = self.live_trigger.clone();
        let proc = self.processing.clone();
        let ld = self.live_delay_ms.clone();

        s.connect_value_changed(move |sc| {
            let v = sc.value().round();
            if (v - default).abs() < 0.5 {
                lbl_c.set_text(&format!("{}: {}", ls, v as i64));
                lbl_c.remove_css_class("modified");
                sc_c.remove_css_class("modified");
            } else {
                lbl_c.set_text(&format!("{}: {} *", ls, v as i64));
                lbl_c.add_css_class("modified");
                sc_c.add_css_class("modified");
            }
            BaseTab::schedule_impl(&lp, &lt, &ltr, &proc, &ld);
        });

        self.controls.append(&lbl);
        self.controls.append(&s);
        self.sliders
            .borrow_mut()
            .push((lbl.clone(), s.clone(), default));
        s
    }

    pub fn slider_widgets(&self, scale: &gtk::Scale) -> Vec<gtk::Widget> {
        for (lbl, s, _) in self.sliders.borrow().iter() {
            if s == scale {
                return vec![lbl.clone().upcast(), s.clone().upcast()];
            }
        }
        vec![scale.clone().upcast()]
    }

    // ---- processing ----
    pub fn spawn_processing(
        self,
        app: AppState,
        input: PathBuf,
        out: PathBuf,
        cmds: Vec<Vec<String>>,
    ) {
        self.spawn_impl(app, input, out, cmds, None, true);
    }
    pub fn spawn_processing_with_callback(
        self,
        app: AppState,
        input: PathBuf,
        out: PathBuf,
        cmds: Vec<Vec<String>>,
        on_complete: Option<Rc<dyn Fn()>>,
    ) {
        self.spawn_impl(app, input, out, cmds, on_complete, true);
    }
    pub fn spawn_processing_preview(
        self,
        app: AppState,
        input: PathBuf,
        out: PathBuf,
        cmds: Vec<Vec<String>>,
    ) {
        self.spawn_impl(app, input, out, cmds, None, false);
    }

    fn spawn_impl(
        self,
        app: AppState,
        _input: PathBuf,
        out: PathBuf,
        cmds: Vec<Vec<String>>,
        on_complete: Option<Rc<dyn Fn()>>,
        commit: bool,
    ) {
        let work: Box<dyn FnOnce() -> Result<PathBuf, String> + Send> = Box::new(move || {
            for cmd in cmds.iter() {
                // Internal (non-process) step: clear colours hidden under transparent pixels.
                if cmd[0] == "@clean-alpha" {
                    crate::util::clear_hidden_colours_in_place(std::path::Path::new(&cmd[1]))?;
                    continue;
                }
                let output = Command::new(&cmd[0]).args(&cmd[1..]).output();
                match output {
                    Ok(o) if o.status.success() => {}
                    Ok(o) => return Err(String::from_utf8_lossy(&o.stderr).to_string()),
                    Err(e) => return Err(format!("spawn error: {}", e)),
                }
            }
            Ok(out)
        });
        self.spawn_work(app, work, on_complete, commit);
    }

    /// Run any blocking job off the UI thread with the usual spinner/status handling.
    /// `commit` = add the result to the history; otherwise only show it as a live preview.
    pub fn spawn_work(
        self,
        app: AppState,
        work: Box<dyn FnOnce() -> Result<PathBuf, String> + Send>,
        on_complete: Option<Rc<dyn Fn()>>,
        commit: bool,
    ) {
        let gen = app.live_gen.get();
        let live = app.live_run.get();
        let step_params = self.get_preset_data();
        self.set_status("Processing… 0s", false);
        self.go.set_sensitive(false);
        self.spinner.set_visible(true);
        self.spinner.start();
        self.processing.set(true);

        {
            let status_c = self.status_lbl.clone();
            let spinner_c = self.spinner.clone();
            let start = Instant::now();
            gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
                if !spinner_c.is_visible() {
                    return gtk::glib::ControlFlow::Break;
                }
                let s = start.elapsed().as_secs();
                status_c.set_text(&format!("Processing… {}s", s));
                gtk::glib::ControlFlow::Continue
            });
        }

        let spinner = self.spinner.clone();
        let base = self.clone();
        let processing = self.processing.clone();

        gtk::glib::MainContext::default().spawn_local(async move {
            let result: Result<PathBuf, String> = gtk::gio::spawn_blocking(work)
                .await
                .unwrap_or_else(|_| Err("task panicked".to_string()));

            spinner.stop();
            spinner.set_visible(false);
            base.go.set_sensitive(true);
            processing.set(false);

            match result {
                Ok(out_path) => {
                    base.set_status("Done.", false);
                    if commit {
                        let label = base.action_name.clone();
                        app.commit_step(&label, out_path, step_params, live);
                        if let Some(cb) = on_complete {
                            cb();
                        }
                    } else if app.live_gen.get() == gen {
                        // (Ignored if the preview was finalized/replaced while it was rendering.)
                        app.show_result_preview(&out_path);
                    }
                }
                Err(e) => base.set_status(&e, true),
            }
        });
    }
}
