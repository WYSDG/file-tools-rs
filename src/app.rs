use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox, Notebook,
    Orientation, Paned, Scale, ScrolledWindow,
};
use gtk4 as gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config;
use crate::settings::{Layout, TabPresets};
use crate::settings_panel::{show_settings_popover, Settings};
use crate::tabs::adjust::AdjustTab;
use crate::tabs::ascii::AsciiTab;
use crate::tabs::convert::ConvertTab;
use crate::tabs::crop::CropTab;
use crate::tabs::duotone::DuotoneTab;
use crate::tabs::fillbg::FillBgTab;
use crate::tabs::halftone::HalftoneTab;
use crate::tabs::outline::OutlineTab;
use crate::tabs::palette_container::PaletteContainer;
use crate::tabs::paperize::PaperizeTab;
use crate::tabs::pixelate::PixelateTab;
use crate::tabs::remove::RemoveTab;
use crate::tabs::watermark::WatermarkTab;
use crate::widgets::preview::Preview;

type TabMetaFn = Rc<dyn Fn() -> (PathBuf, String) + 'static>;
type InputListener = Rc<dyn Fn(Option<PathBuf>, Option<String>)>;

/// (tab_name, short_label, tooltip, group)
const RAIL_LAYOUT: &[(&str, &str, &str, &str)] = &[
    ("Remove", "RemBg", "Remove Background", "Isolate"),
    ("Adjust", "Adjst", "Adjust", "Adjust"),
    ("Crop", "Crop", "Crop & Transform", "Adjust"),
    ("Fill Background", "Fill", "Fill Background", "Color"),
    ("Palette", "Palet", "Palette", "Color"),
    ("Duotone", "Duo", "Duotone", "Effects"),
    ("Halftone", "Half", "Halftone", "Effects"),
    ("ASCII", "ASCII", "ASCII Art", "Effects"),
    ("Pixelate", "Pix", "Pixelate", "Effects"),
    ("Paperize", "Paper", "Paperize (paper cutout)", "Effects"),
    ("Outline", "Out", "Outline", "Style"),
    ("Watermark", "Wmark", "Watermark", "Style"),
    ("Convert", "Conv", "Convert Format", "Utility"),
];

const TAB_NAMES: &[&str] = &[
    "Remove",
    "Adjust",
    "Crop",
    "Convert",
    "Fill Background",
    "Palette",
    "Halftone",
    "Duotone",
    "ASCII",
    "Pixelate",
    "Paperize",
    "Outline",
    "Watermark",
];

const FADE_TABS: &[&str] = &["Remove", "Adjust", "Duotone", "Pixelate", "Paperize", "Watermark"];

#[derive(Clone)]
pub struct HistoryEntry {
    pub label: String,
    pub output_path: PathBuf,
    /// The tool's control values when this step was produced (same format as presets).
    pub params: serde_json::Value,
    /// Produced by a live-preview run. Consecutive live runs of the same tool collapse into one
    /// step; explicit "Process" clicks always start a new step.
    pub live: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub preview: Preview,
    pub info_lbl: Label,
    pub fname: Entry,
    pub initial_load: Rc<RefCell<Option<PathBuf>>>,
    pub history: Rc<RefCell<Vec<HistoryEntry>>>,
    pub current_result: Rc<RefCell<Option<PathBuf>>>,
    pub input_listeners: Rc<RefCell<Vec<(String, InputListener)>>>,
    pub tab_go_buttons: Rc<RefCell<HashMap<String, Button>>>,
    pub tab_meta: Rc<RefCell<HashMap<String, TabMetaFn>>>,
    pub active_tab_name: Rc<RefCell<String>>,
    pub history_list: Rc<RefCell<Option<ListBox>>>,
    pub fade_box: Rc<RefCell<Option<GtkBox>>>,
    pub fade_slider: Rc<RefCell<Option<Scale>>>,
    pub strip_box: Rc<RefCell<Option<GtkBox>>>,
    pub presets: Rc<RefCell<TabPresets>>,
    pub settings: Rc<RefCell<Settings>>,
    pub redo_stack: Rc<RefCell<Vec<HistoryEntry>>>,
    pub theme_provider: Rc<RefCell<Option<gtk::CssProvider>>>,
    pub rail_group_headers: Rc<RefCell<HashMap<String, Label>>>,
    pub rail_group_tabs: Rc<RefCell<HashMap<String, Vec<String>>>>,
    /// A live preview that has been shown but not yet committed at full resolution.
    pub pending_commit: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    /// Bumped whenever pending work is finalized or dropped, so late preview results are ignored.
    pub live_gen: Rc<std::cell::Cell<u64>>,
    /// True while a live-preview trigger is running (so commits can be tagged as live).
    pub live_run: Rc<std::cell::Cell<bool>>,
}

impl AppState {
    pub fn set_pending_commit(&self, f: Rc<dyn Fn()>) {
        *self.pending_commit.borrow_mut() = Some(f);
    }

    /// Forget any un-committed preview (something newer replaced it, or it was undone).
    pub fn drop_pending(&self) {
        *self.pending_commit.borrow_mut() = None;
        self.live_gen.set(self.live_gen.get() + 1);
    }

    /// Produce the full-resolution result of a pending live preview, if there is one.
    /// Call before anything that reads the current result (save, copy, switching tools).
    pub fn flush_pending(&self) {
        let f = self.pending_commit.borrow_mut().take();
        if let Some(f) = f {
            self.live_gen.set(self.live_gen.get() + 1);
            f();
        }
    }

    pub fn register_input_listener(&self, name: &str, cb: InputListener) {
        self.input_listeners
            .borrow_mut()
            .push((name.to_string(), cb));
    }

    fn friendly_name(&self, path: &Path) -> String {
        for entry in self.history.borrow().iter() {
            if entry.output_path == path {
                return format!("Output of {}", entry.label);
            }
        }
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn head_action(&self) -> String {
        self.history
            .borrow()
            .last()
            .map(|e| e.label.clone())
            .unwrap_or_else(|| "Loaded".to_string())
    }

    fn broadcast_input(&self, path: Option<PathBuf>, skip_active: bool) {
        let display = path.as_ref().map(|p| self.friendly_name(p));
        let skip_name = if skip_active {
            Some(self.active_tab_name.borrow().clone())
        } else {
            None
        };
        for (name, cb) in self.input_listeners.borrow().iter() {
            if let Some(s) = &skip_name {
                if name == s {
                    continue;
                }
            }
            cb(path.clone(), display.clone());
        }
    }

    fn current_head(&self) -> Option<PathBuf> {
        self.history
            .borrow()
            .last()
            .map(|e| e.output_path.clone())
            .or_else(|| self.initial_load.borrow().clone())
    }

    fn current_fade_original(&self) -> Option<PathBuf> {
        let h = self.history.borrow();
        if h.len() >= 2 {
            Some(h[h.len() - 2].output_path.clone())
        } else {
            self.initial_load.borrow().clone()
        }
    }

    fn refresh_strip(&self) {
        let strip_opt = self.strip_box.borrow().clone();
        let Some(strip) = strip_opt else { return };

        while let Some(c) = strip.first_child() {
            strip.remove(&c);
        }

        let initial = self.initial_load.borrow().clone();
        let Some(init_path) = initial else { return };

        let fname = init_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let load_btn = Button::with_label(&format!("📁 {}", fname));
        load_btn.add_css_class("flat");
        {
            let state = self.clone();
            load_btn.connect_clicked(move |_| state.rewind_to(-1));
        }
        strip.append(&load_btn);

        let snapshot: Vec<HistoryEntry> = self.history.borrow().clone();
        let len = snapshot.len();

        for (i, entry) in snapshot.into_iter().enumerate() {
            let arrow = Label::new(Some("→"));
            arrow.set_margin_start(6);
            arrow.set_margin_end(6);
            arrow.add_css_class("section-label");
            strip.append(&arrow);

            let chip = Button::with_label(&entry.label);
            chip.add_css_class("flat");
            {
                let state = self.clone();
                let idx = i as isize;
                chip.connect_clicked(move |_| state.rewind_to(idx));
            }
            strip.append(&chip);
        }

        if len > 0 {
            let clear = Button::with_label("✕");
            clear.set_tooltip_text(Some("Clear pipeline — back to initial file"));
            clear.set_margin_start(12);
            clear.add_css_class("flat");
            {
                let state = self.clone();
                clear.connect_clicked(move |_| state.clear_pipeline());
            }
            strip.append(&clear);
        }
    }

    /// Commit a step without recorded settings (explicit action).
    pub fn commit_history(&self, label: &str, output: PathBuf) {
        self.commit_step(label, output, serde_json::Value::Null, false);
    }

    /// Commit a step with the tool's settings. `live` = produced by a live-preview run: such a step
    /// is replaced by the next live/explicit run of the same tool; explicit steps are never replaced.
    pub fn commit_step(&self, label: &str, output: PathBuf, params: serde_json::Value, live: bool) {
        self.drop_pending();
        let replace = self
            .history
            .borrow()
            .last()
            .map(|e| e.label == label && e.live)
            .unwrap_or(false);

        if replace {
            if let Some(last) = self.history.borrow_mut().last_mut() {
                last.output_path = output.clone();
                last.params = params;
                last.live = live;
            }
        } else {
            self.history.borrow_mut().push(HistoryEntry {
                label: label.to_string(),
                output_path: output.clone(),
                params,
                live,
            });
        }
        self.redo_stack.borrow_mut().clear();
        *self.current_result.borrow_mut() = Some(output.clone());

        self.preview.set_crop_overlay(None);

        let prev = self.current_fade_original();
        if let Some(p) = prev {
            self.preview.set_original(&p);
        }
        self.preview.set_result(&output);

        if let Some(s) = self.fade_slider.borrow().as_ref() {
            s.set_value(100.0);
        }

        let stem = self
            .initial_load
            .borrow()
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        let ext = output
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "png".to_string());
        self.fname.set_text(&format!("{}_{}.{}", stem, label, ext));

        if let Ok((w, h)) = crate::util::image_dimensions(&output) {
            self.info_lbl
                .set_text(&format!("{}  •  {}×{}", output.display(), w, h));
        } else {
            self.info_lbl.set_text(&format!("{}", output.display()));
        }

        self.broadcast_input(Some(output), true);
        self.refresh_strip();
    }

    /// Ctrl+Z. A pending (not yet finalized) live preview is cancelled first; otherwise the
    /// last step is undone.
    pub fn undo(&self) {
        if self.pending_commit.borrow().is_some() {
            self.drop_pending();
            self.show_head();
            return;
        }
        let len = self.history.borrow().len() as isize;
        if len > 0 {
            self.rewind_to(len - 2);
        }
    }

    /// Show the committed state (last step, or the original) in the canvas.
    fn show_head(&self) {
        let head = self.history.borrow().last().map(|e| e.output_path.clone());
        match head {
            Some(h) => {
                if let Some(p) = self.current_fade_original() {
                    self.preview.set_original(&p);
                }
                self.preview.set_result(&h);
            }
            None => {
                if let Some(p) = self.initial_load.borrow().clone() {
                    self.preview.set_original(&p);
                }
                self.preview.clear_result();
            }
        }
    }

    pub fn rewind_to(&self, idx: isize) {
        self.drop_pending();
        let new_len = (idx + 1) as usize;
        {
            let mut h = self.history.borrow_mut();
            if new_len >= h.len() {
                return;
            }
            let removed: Vec<HistoryEntry> = h.drain(new_len..).collect();
            for entry in removed.into_iter().rev() {
                self.redo_stack.borrow_mut().push(entry);
            }
        }

        let stem = self
            .initial_load
            .borrow()
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());

        if new_len == 0 {
            let init = self.initial_load.borrow().clone();
            match init {
                Some(p) => {
                    self.preview.set_original(&p);
                    self.preview.clear_result();
                    *self.current_result.borrow_mut() = None;
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    self.fname.set_text(&name);
                    self.broadcast_input(Some(p), false);
                }
                None => {
                    self.preview.clear_result();
                    self.fname.set_text("");
                    self.broadcast_input(None, false);
                }
            }
            if let Some(s) = self.fade_slider.borrow().as_ref() {
                s.set_value(0.0);
            }
        } else {
            let head = self.current_head().unwrap();
            let prev = self.current_fade_original();
            if let Some(p) = prev {
                self.preview.set_original(&p);
            }
            self.preview.set_result(&head);
            *self.current_result.borrow_mut() = Some(head.clone());
            if let Some(s) = self.fade_slider.borrow().as_ref() {
                s.set_value(100.0);
            }
            let label = self
                .history
                .borrow()
                .last()
                .map(|e| e.label.clone())
                .unwrap_or_else(|| "Processed".to_string());
            let ext = head
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "png".to_string());
            self.fname.set_text(&format!("{}_{}.{}", stem, label, ext));
            self.broadcast_input(Some(head), false);
        }
        self.refresh_strip();
    }

    pub fn redo(&self) {
        self.drop_pending();
        let Some(entry) = self.redo_stack.borrow_mut().pop() else {
            return;
        };
        self.history.borrow_mut().push(entry);
        let head = self.current_head().unwrap();
        let prev = self.current_fade_original();
        if let Some(p) = prev {
            self.preview.set_original(&p);
        }
        self.preview.set_result(&head);
        *self.current_result.borrow_mut() = Some(head.clone());
        if let Some(s) = self.fade_slider.borrow().as_ref() {
            s.set_value(100.0);
        }
        self.broadcast_input(Some(head), false);
        self.refresh_strip();
    }

    pub fn clear_pipeline(&self) {
        self.redo_stack.borrow_mut().clear();
        self.rewind_to(-1);
    }

    pub fn set_universal_input(&self, path: Option<PathBuf>) {
        self.drop_pending();
        *self.initial_load.borrow_mut() = path.clone();
        self.history.borrow_mut().clear();
        *self.current_result.borrow_mut() = None;

        match &path {
            Some(p) => {
                self.preview.set_original(p);
                self.preview.clear_result();
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.fname.set_text(&name);
                if let Ok((w, h)) = crate::util::image_dimensions(p) {
                    self.info_lbl.set_text(&format!("{}  •  {}×{}", name, w, h));
                } else {
                    self.info_lbl.set_text(&name);
                }
            }
            None => {
                self.info_lbl.set_text("No image loaded");
                self.fname.set_text("");
            }
        }

        if let Some(s) = self.fade_slider.borrow().as_ref() {
            s.set_value(0.0);
        }

        self.broadcast_input(path, false);
        self.refresh_strip();
    }

    pub fn paste_from_clipboard(&self) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        let clipboard = display.clipboard();
        let app = self.clone();
        clipboard.read_texture_async(gtk::gio::Cancellable::NONE, move |res| {
            let Ok(maybe_tex) = res else { return };
            let Some(texture) = maybe_tex else { return };
            let path = config::tmp_file(".png");
            if texture.save_to_png(&path).is_ok() {
                app.set_universal_input(Some(path));
            }
        });
    }

    pub fn trigger_active_process(&self) {
        let name = self.active_tab_name.borrow().clone();
        let btns = self.tab_go_buttons.borrow();
        if let Some(btn) = btns.get(&name) {
            if btn.is_sensitive() {
                btn.emit_clicked();
            }
        }
    }

    pub fn copy_result_image(&self) {
        self.flush_pending();
        let src = self
            .current_result
            .borrow()
            .clone()
            .or_else(|| self.initial_load.borrow().clone());
        let Some(src) = src else {
            self.info_lbl
                .set_text("Nothing to copy — load or process an image first");
            return;
        };
        if crate::util::copy_image_to_clipboard(&src) {
            self.info_lbl.set_text("✓ Image copied to clipboard");
        } else {
            self.info_lbl
                .set_text("Copy failed — is wl-clipboard installed?");
        }
    }

    pub fn flush_preview(&self) {
        self.preview.clear_result();
        *self.current_result.borrow_mut() = None;
        if let Some(s) = self.fade_slider.borrow().as_ref() {
            s.set_value(0.0);
        }
    }

    pub fn show_result_preview(&self, path: &Path) {
        let prev = self.current_head();
        if let Some(p) = prev {
            self.preview.set_original(&p);
        }
        self.preview.set_result(path);
        if let Some(s) = self.fade_slider.borrow().as_ref() {
            s.set_value(100.0);
        }
    }

    fn current_meta(&self) -> (PathBuf, String) {
        let name = self.active_tab_name.borrow().clone();
        self.tab_meta
            .borrow()
            .get(&name)
            .map(|f| f())
            .unwrap_or_else(|| (config::save_dir(), "Processed".to_string()))
    }

    pub fn save_as(&self) {
        self.flush_pending();
        let Some(result) = self.current_result.borrow().clone() else {
            return;
        };
        let (_tab_dir, action) = self.current_meta();
        let settings = self.settings.borrow().clone();
        let save_dir = settings.effective_output_dir();

        let fname_text = self.fname.text().to_string();
        let name = if fname_text.trim().is_empty() {
            let stem = self
                .initial_load
                .borrow()
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            let ext = result
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "png".to_string());
            format!("{}_{}.{}", stem, action, ext)
        } else {
            fname_text.trim().to_string()
        };

        let _ = std::fs::create_dir_all(&save_dir);
        let dst = save_dir.join(&name);

        let data = match std::fs::read(&result) {
            Ok(d) => d,
            Err(e) => {
                self.info_lbl.set_text(&format!("Save failed: {}", e));
                return;
            }
        };
        if let Err(e) = std::fs::write(&dst, &data) {
            self.info_lbl.set_text(&format!("Save failed: {}", e));
            return;
        }

        if let Some(args) = settings.exiftool_args() {
            let mut cmd = std::process::Command::new("exiftool");
            cmd.args(&args);
            cmd.arg(dst.to_string_lossy().as_ref());
            let _ = cmd.output();
        }

        *self.current_result.borrow_mut() = Some(dst.clone());
        self.info_lbl
            .set_text(&format!("Saved → {}", dst.display()));
        self.refresh_history();
    }

    pub fn refresh_history(&self) {
        let Some(list) = self.history_list.borrow().as_ref().cloned() else {
            return;
        };
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        if let Ok(entries) = std::fs::read_dir(config::save_dir()) {
            let mut files: Vec<String> = entries
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect();
            files.sort();
            files.reverse();
            for name in files {
                let lbl = Label::new(Some(&name));
                lbl.set_xalign(0.0);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                list.append(&lbl);
            }
        }
    }

    pub fn load_from_history(&self, path: PathBuf) {
        self.drop_pending();
        self.set_universal_input(Some(path));
    }

    pub fn update_fade_visibility(&self) {
        let name = self.active_tab_name.borrow().clone();
        let show = FADE_TABS.contains(&name.as_str());
        if let Some(fb) = self.fade_box.borrow().as_ref() {
            fb.set_visible(show);
        }
    }

    pub fn refresh_rail_groups(&self) {
        let layout = Layout::load();
        let headers = self.rail_group_headers.borrow();
        let groups = self.rail_group_tabs.borrow();
        for (grp, tabs) in groups.iter() {
            let any = tabs.iter().any(|t| layout.is_enabled(t));
            if let Some(hdr) = headers.get(grp) {
                hdr.set_visible(any);
            }
        }
    }

    pub fn start_eyedropper(&self, cb: Rc<dyn Fn(String)>) {
        self.preview.set_eyedropper_mode(true, Some(cb));
        self.info_lbl
            .set_text("🎯 Eyedropper active — click anywhere on the preview to pick a color");
        let info = self.info_lbl.clone();
        self.preview.set_on_hover_hex(Rc::new(move |hex| {
            if let Some(h) = hex {
                info.set_text(&format!("🎯 Pick: {}  — click to accept", h));
            }
        }));
    }

    pub fn cancel_eyedropper(&self) {
        self.preview.set_eyedropper_mode(false, None);
        self.info_lbl.set_text("");
    }
}

/// Image passed on the command line (`file-tools photo.png`, or "Open With" from a file manager).
static INITIAL_FILE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub fn run() {
    if let Some(p) = std::env::args().skip(1).find(|a| !a.starts_with('-')) {
        if let Ok(path) = std::fs::canonicalize(&p) {
            if path.is_file() {
                let _ = INITIAL_FILE.set(path);
            }
        }
    }
    let app = Application::builder()
        .application_id("dev.local.filetools")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(build_ui);
    // We parse the file argument ourselves, so don't let GTK reject it.
    app.run_with_args::<&str>(&[]);
}

fn build_ui(app: &Application) {
    crate::icons::install();
    let settings_init = Settings::load();
    let initial_theme = settings_init.theme.clone();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("File Tools")
        .default_width(1500)
        .default_height(950)
        .build();

    let info = Label::new(Some("No image loaded"));
    info.set_xalign(0.0);
    info.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let preview = Preview::new();

    let fname = Entry::new();
    fname.set_placeholder_text(Some("filename.ext"));
    fname.set_hexpand(true);

    let app_state = AppState {
        preview: preview.clone(),
        info_lbl: info.clone(),
        fname: fname.clone(),
        initial_load: Rc::new(RefCell::new(None)),
        history: Rc::new(RefCell::new(Vec::new())),
        current_result: Rc::new(RefCell::new(None)),
        input_listeners: Rc::new(RefCell::new(Vec::new())),
        tab_go_buttons: Rc::new(RefCell::new(HashMap::new())),
        tab_meta: Rc::new(RefCell::new(HashMap::new())),
        active_tab_name: Rc::new(RefCell::new("Remove".to_string())),
        pending_commit: Rc::new(RefCell::new(None)),
        live_gen: Rc::new(std::cell::Cell::new(0)),
        live_run: Rc::new(std::cell::Cell::new(false)),
        history_list: Rc::new(RefCell::new(None)),
        fade_box: Rc::new(RefCell::new(None)),
        fade_slider: Rc::new(RefCell::new(None)),
        strip_box: Rc::new(RefCell::new(None)),
        presets: Rc::new(RefCell::new(TabPresets::load())),
        settings: Rc::new(RefCell::new(settings_init)),
        redo_stack: Rc::new(RefCell::new(Vec::new())),
        theme_provider: Rc::new(RefCell::new(None)),
        rail_group_headers: Rc::new(RefCell::new(HashMap::new())),
        rail_group_tabs: Rc::new(RefCell::new(HashMap::new())),
    };

    // Install the initial theme and keep the provider so we can swap it
    {
        let provider = gtk::CssProvider::new();
        crate::settings_panel::apply_color_scheme(&initial_theme);
        provider.load_from_string(&crate::settings_panel::build_theme_css(&initial_theme));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        *app_state.theme_provider.borrow_mut() = Some(provider);
    }

    // Set the initial canvas background based on theme
    {
        let canvas_bg = crate::settings_panel::palette(&initial_theme).canvas;
        preview.set_bg_color(canvas_bg);
    }

    // Page map
    let page_map: Rc<RefCell<HashMap<String, gtk::Widget>>> = Rc::new(RefCell::new(HashMap::new()));
    let tab_index: Rc<RefCell<HashMap<String, u32>>> = Rc::new(RefCell::new(HashMap::new()));
    let rail_buttons: Rc<RefCell<HashMap<String, gtk::ToggleButton>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let layout = Rc::new(RefCell::new(Layout::load()));

    // ---------- Inspector notebook ----------
    let inspector_nb = Notebook::new();
    inspector_nb.set_show_tabs(false);
    inspector_nb.set_scrollable(true);
    inspector_nb.set_hexpand(true);
    inspector_nb.set_vexpand(true);
    inspector_nb.set_size_request(320, -1);
    inspector_nb.add_css_class("inspector-pane");

    let mut next_page: u32 = 0;

    macro_rules! register_page {
        ($name:expr, $widget:expr) => {{
            let scroll = ScrolledWindow::builder()
                .vexpand(true)
                .hexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build();
            scroll.set_child(Some(&$widget));
            inspector_nb.append_page(&scroll, None::<&gtk::Widget>);
            page_map
                .borrow_mut()
                .insert($name.to_string(), scroll.clone().upcast());
            tab_index.borrow_mut().insert($name.to_string(), next_page);
            next_page += 1;
        }};
    }

    // Remove
    {
        let t = RemoveTab::new(app_state.clone());
        register_page!("Remove", t.base.inspector);
        let tc = t.clone();
        app_state.register_input_listener("Remove", Rc::new(move |p, d| tc.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Remove".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Remove".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Adjust
    {
        let t = AdjustTab::new(app_state.clone());
        register_page!("Adjust", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Adjust", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        t.base.go.set_visible(false);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run_live(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Adjust".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Adjust".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Crop
    {
        let t = CropTab::new(app_state.clone());
        register_page!("Crop", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Crop", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc, true));
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Crop".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Crop".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Convert
    {
        let t = Rc::new(ConvertTab::new(app_state.clone()));
        register_page!("Convert", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Convert", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Convert".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Convert".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Fill Background
    {
        let t = FillBgTab::new(app_state.clone());
        register_page!("Fill Background", t.base.inspector);
        let b = t.base.clone();
        app_state
            .register_input_listener("Fill Background", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run_live(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Fill Background".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Fill Background".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Palette
    {
        let container = PaletteContainer::new(app_state.clone());
        register_page!("Palette", container.inspector);
        let cc = container.clone();
        app_state.register_input_listener("Palette", Rc::new(move |p, d| cc.set_input(p, d)));
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Palette".to_string(), container.go_proxy.clone());
        let cc = container.clone();
        app_state
            .tab_meta
            .borrow_mut()
            .insert("Palette".to_string(), Rc::new(move || cc.current_meta()));
    }
    // Halftone
    {
        let t = HalftoneTab::new(app_state.clone());
        register_page!("Halftone", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Halftone", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 600);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Halftone".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Halftone".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Duotone
    {
        let t = DuotoneTab::new(app_state.clone());
        register_page!("Duotone", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Duotone", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Duotone".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Duotone".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // ASCII
    {
        let t = AsciiTab::new(app_state.clone());
        register_page!("ASCII", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("ASCII", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("ASCII".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "ASCII".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
        {
            let tc = t.clone();
            t.copy_btn.connect_clicked(move |_| tc.copy_text());
        }
        {
            let tc = t.clone();
            t.save_btn.connect_clicked(move |_| tc.save_txt());
        }
    }
    // Pixelate
    {
        let t = Rc::new(PixelateTab::new(app_state.clone()));
        register_page!("Pixelate", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Pixelate", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Pixelate".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Pixelate".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Paperize
    {
        let t = Rc::new(PaperizeTab::new(app_state.clone()));
        register_page!("Paperize", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Paperize", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 500);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run_live(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Paperize".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Paperize".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Outline
    {
        let t = OutlineTab::new(app_state.clone());
        register_page!("Outline", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Outline", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Outline".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Outline".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }
    // Watermark
    {
        let t = WatermarkTab::new(app_state.clone());
        register_page!("Watermark", t.base.inspector);
        let b = t.base.clone();
        app_state.register_input_listener("Watermark", Rc::new(move |p, d| b.set_input(p, d)));
        let tc = t.clone();
        let sc = app_state.clone();
        t.base.go.connect_clicked(move |_| tc.run(&sc));
        t.base.set_live_preview(true, 400);
        {
            let tc = t.clone();
            let sc = app_state.clone();
            t.base.set_live_trigger(Rc::new(move || tc.run(&sc)));
        }
        app_state
            .tab_go_buttons
            .borrow_mut()
            .insert("Watermark".to_string(), t.base.go.clone());
        let bc = t.base.clone();
        app_state.tab_meta.borrow_mut().insert(
            "Watermark".to_string(),
            Rc::new(move || (bc.save_dir(), bc.action_name.clone())),
        );
    }

    // ---------- Rail ----------
    let rail = GtkBox::new(Orientation::Vertical, 2);
    rail.set_margin_top(6);
    rail.set_margin_bottom(6);
    rail.set_margin_start(4);
    rail.set_margin_end(4);
    rail.set_size_request(84, -1);
    rail.add_css_class("rail-pane");

    let mut group: Option<gtk::ToggleButton> = None;
    let mut current_group = String::new();

    for (tab, short, tooltip, grp) in RAIL_LAYOUT {
        if *grp != current_group {
            current_group = grp.to_string();
            let hdr = Label::new(Some(&grp.to_uppercase()));
            hdr.set_xalign(0.0);
            hdr.add_css_class("section-label");
            hdr.set_margin_top(6);
            hdr.set_margin_start(4);
            rail.append(&hdr);
            app_state
                .rail_group_headers
                .borrow_mut()
                .insert(grp.to_string(), hdr);
        }
        app_state
            .rail_group_tabs
            .borrow_mut()
            .entry(grp.to_string())
            .or_default()
            .push(tab.to_string());

        let btn = gtk::ToggleButton::new();
        let inner = Label::new(Some(short));
        inner.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let content = GtkBox::new(gtk::Orientation::Vertical, 1);
        content.set_halign(gtk::Align::Center);
        if let Some(icon) = crate::icons::tab_icon(tab) {
            let img = gtk::Image::from_icon_name(icon);
            img.set_pixel_size(20);
            content.append(&img);
        }
        content.append(&inner);
        btn.set_child(Some(&content));
        btn.add_css_class("flat");
        btn.add_css_class("rail-btn");
        btn.set_tooltip_text(Some(tooltip));

        if let Some(g) = group.as_ref() {
            btn.set_group(Some(g));
        } else {
            group = Some(btn.clone());
        }

        let tab_name_s = tab.to_string();
        let ti = tab_index.clone();
        let atn = app_state.active_tab_name.clone();
        let state = app_state.clone();
        let nb = inspector_nb.clone();
        btn.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Some(idx) = ti.borrow().get(&tab_name_s) {
                nb.set_current_page(Some(*idx));
            }
            // Finish any pending live preview so the next tool receives the full-resolution result.
            state.flush_pending();
            *atn.borrow_mut() = tab_name_s.clone();
            state.update_fade_visibility();
            state.preview.set_crop_mode(tab_name_s == "Crop");
        });

        rail_buttons
            .borrow_mut()
            .insert(tab.to_string(), btn.clone());
        rail.append(&btn);
    }

    if let Some(first) = rail_buttons.borrow().get("Remove").cloned() {
        first.set_active(true);
    }

    let rail_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .build();
    rail_scroll.set_child(Some(&rail));
    rail_scroll.add_css_class("rail-pane");

    // ---------- Center canvas ----------
    let center = GtkBox::new(Orientation::Vertical, 4);
    center.set_margin_top(8);
    center.set_margin_bottom(4);
    center.set_margin_start(8);
    center.set_margin_end(4);
    center.set_vexpand(true);
    center.set_hexpand(true);

    center.append(&preview.root);

    let fade_box = GtkBox::new(Orientation::Vertical, 2);
    let fade_slider = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    fade_slider.set_value(0.0);
    {
        let state = app_state.clone();
        fade_slider.connect_value_changed(move |s| {
            state.preview.set_fade(s.value() / 100.0);
        });
    }
    fade_box.append(&fade_slider);
    fade_box.set_visible(false);
    center.append(&fade_box);
    *app_state.fade_box.borrow_mut() = Some(fade_box);
    *app_state.fade_slider.borrow_mut() = Some(fade_slider);

    // ---------- Layout ----------
    let main = GtkBox::new(Orientation::Vertical, 0);

    // Top bar
    let top_bar = GtkBox::new(Orientation::Horizontal, 6);
    top_bar.set_margin_top(8);
    top_bar.set_margin_bottom(8);
    top_bar.set_margin_start(8);
    top_bar.set_margin_end(8);
    top_bar.add_css_class("top-bar");

    let choose_btn = Button::with_label("Choose Image…");
    {
        let app_cb = app_state.clone();
        choose_btn.connect_clicked(move |b| {
            let dialog = gtk::FileDialog::builder().title("Choose an image").build();
            let app_inner = app_cb.clone();
            let parent_root = b.root();
            let parent_window: Option<gtk::Window> =
                parent_root.and_then(|r| r.downcast::<gtk::Window>().ok());
            dialog.open(
                parent_window.as_ref(),
                gtk::gio::Cancellable::NONE,
                move |res| {
                    if let Ok(file) = res {
                        if let Some(path) = file.path() {
                            app_inner.set_universal_input(Some(path));
                        }
                    }
                },
            );
        });
    }
    top_bar.append(&choose_btn);

    let paste_btn = Button::with_label("Paste");
    {
        let app_cb = app_state.clone();
        paste_btn.connect_clicked(move |_| app_cb.paste_from_clipboard());
    }
    top_bar.append(&paste_btn);

    top_bar.append(&fname);

    let save_btn = Button::with_label("Save As");
    {
        let state = app_state.clone();
        save_btn.connect_clicked(move |_| state.save_as());
    }
    top_bar.append(&save_btn);

    let copy_btn = Button::with_label("Copy Image");
    {
        let state = app_state.clone();
        copy_btn.connect_clicked(move |_| state.copy_result_image());
    }
    top_bar.append(&copy_btn);

    let flush_btn = Button::with_label("Flush");
    flush_btn.set_tooltip_text(Some("Reset pipeline back to the initial file"));
    {
        let state = app_state.clone();
        flush_btn.connect_clicked(move |_| state.clear_pipeline());
    }
    top_bar.append(&flush_btn);

    let tabs_btn = gtk::MenuButton::new();
    tabs_btn.set_label("Tabs ▾");
    tabs_btn.add_css_class("flat");
    tabs_btn.set_tooltip_text(Some("Show or hide tabs"));
    top_bar.append(&tabs_btn);

    // Tabs popover
    {
        let popover = gtk::Popover::new();
        let vbox = GtkBox::new(Orientation::Vertical, 4);
        vbox.set_margin_top(8);
        vbox.set_margin_bottom(8);
        vbox.set_margin_start(8);
        vbox.set_margin_end(8);
        vbox.set_size_request(240, -1);

        let hdr = Label::new(Some("Visible tabs"));
        hdr.set_xalign(0.0);
        hdr.add_css_class("section-label");
        vbox.append(&hdr);

        let bulk_row = GtkBox::new(Orientation::Horizontal, 4);
        let all_btn = Button::with_label("Select All");
        let none_btn = Button::with_label("Unselect All");
        all_btn.set_hexpand(true);
        none_btn.set_hexpand(true);
        bulk_row.append(&all_btn);
        bulk_row.append(&none_btn);
        vbox.append(&bulk_row);
        vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        let check_buttons: Rc<RefCell<Vec<(String, gtk::CheckButton)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let state_c = app_state.clone();
        for name in TAB_NAMES {
            let cb = gtk::CheckButton::with_label(name);
            cb.set_active(layout.borrow().is_enabled(name));
            let name_s = name.to_string();
            let layout_c = layout.clone();
            let pm_c = page_map.clone();
            let rail_c = rail_buttons.clone();
            let state_cb = state_c.clone();
            cb.connect_toggled(move |c| {
                let active = c.is_active();
                layout_c.borrow_mut().set_enabled(&name_s, active);
                layout_c.borrow().save();
                if let Some(page) = pm_c.borrow().get(&name_s) {
                    page.set_visible(active);
                }
                if let Some(btn) = rail_c.borrow().get(&name_s) {
                    btn.set_visible(active);
                }
                state_cb.refresh_rail_groups();
            });
            vbox.append(&cb);
            check_buttons.borrow_mut().push((name.to_string(), cb));
        }

        {
            let cbs = check_buttons.clone();
            all_btn.connect_clicked(move |_| {
                for (_, cb) in cbs.borrow().iter() {
                    cb.set_active(true);
                }
            });
        }
        {
            let cbs = check_buttons.clone();
            none_btn.connect_clicked(move |_| {
                for (_, cb) in cbs.borrow().iter() {
                    cb.set_active(false);
                }
            });
        }

        popover.set_child(Some(&vbox));
        tabs_btn.set_popover(Some(&popover));
    }

    let gear_btn = gtk::MenuButton::new();
    gear_btn.set_label("⚙");
    gear_btn.add_css_class("flat");
    gear_btn.set_tooltip_text(Some("Settings"));
    top_bar.append(&gear_btn);
    show_settings_popover(&gear_btn, app_state.clone());

    main.append(&top_bar);

    // Middle
    let middle = Paned::new(Orientation::Horizontal);
    middle.set_wide_handle(true);
    middle.set_vexpand(true);
    middle.set_hexpand(true);
    middle.set_start_child(Some(&rail_scroll));

    let right_paned = Paned::new(Orientation::Horizontal);
    right_paned.set_wide_handle(true);
    right_paned.set_vexpand(true);
    right_paned.set_hexpand(true);
    right_paned.set_start_child(Some(&center));
    right_paned.set_end_child(Some(&inspector_nb));

    middle.set_position(160);
    right_paned.set_position(800);

    middle.set_end_child(Some(&right_paned));
    main.append(&middle);

    {
        let m = middle.clone();
        let rp = right_paned.clone();
        let w = window.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            let width = w.width();
            if width < 300 {
                return;
            }
            let rail_w = (width as f64 * 0.13).max(90.0) as i32;
            m.set_position(rail_w);
            let remaining = width - rail_w;
            if remaining > 200 {
                let canvas_w = (remaining as f64 * 0.72).max(300.0) as i32;
                rp.set_position(canvas_w);
            }
        });
    }

    // Bottom strip
    let bottom = GtkBox::new(Orientation::Horizontal, 8);
    bottom.set_margin_top(4);
    bottom.set_margin_bottom(6);
    bottom.set_margin_start(8);
    bottom.set_margin_end(8);
    bottom.set_vexpand(false);
    bottom.set_valign(gtk::Align::End);
    bottom.set_size_request(-1, 46);

    let strip = GtkBox::new(Orientation::Horizontal, 4);
    let strip_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(40)
        .max_content_height(48)
        .vexpand(false)
        .hexpand(true)
        .build();
    strip_scroll.set_child(Some(&strip));
    bottom.append(&strip_scroll);
    *app_state.strip_box.borrow_mut() = Some(strip);
    app_state.refresh_strip();

    // History — compact
    let hist_col = GtkBox::new(Orientation::Vertical, 2);
    hist_col.set_size_request(220, -1);
    hist_col.set_vexpand(false);
    hist_col.set_valign(gtk::Align::End);

    let hist_hdr = Label::new(Some("<b>History</b>"));
    hist_hdr.set_use_markup(true);
    hist_hdr.set_xalign(0.0);
    hist_col.append(&hist_hdr);

    let hist_list = ListBox::new();
    hist_list.set_selection_mode(gtk::SelectionMode::None);
    hist_list.add_css_class("hist-pane");
    let hist_scroll = ScrolledWindow::builder()
        .vexpand(false)
        .min_content_height(60)
        .max_content_height(72)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    hist_scroll.set_child(Some(&hist_list));
    hist_col.append(&hist_scroll);
    bottom.append(&hist_col);

    *app_state.history_list.borrow_mut() = Some(hist_list.clone());
    app_state.refresh_history();

    {
        let state = app_state.clone();
        hist_list.connect_row_activated(move |_, row| {
            let Some(lbl) = row.child().and_then(|c| c.downcast::<Label>().ok()) else {
                return;
            };
            let name = lbl.text().to_string();
            let p = config::save_dir().join(name);
            if p.exists() {
                state.load_from_history(p);
            }
        });
    }

    main.append(&bottom);
    window.set_child(Some(&main));

    // Apply initial layout — hide disabled tabs
    {
        let pm = page_map.borrow();
        let rb = rail_buttons.borrow();
        let l = layout.borrow();
        for name in TAB_NAMES {
            if !l.is_enabled(name) {
                if let Some(page) = pm.get(*name) {
                    page.set_visible(false);
                }
                if let Some(btn) = rb.get(*name) {
                    btn.set_visible(false);
                }
            }
        }
    }
    app_state.refresh_rail_groups();

    // Keyboard shortcuts
    let key_ctrl = gtk::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let state = app_state.clone();
        key_ctrl.connect_key_pressed(move |kc, keyval, _, modifiers| {
            use gtk::gdk::ModifierType;
            let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(ModifierType::SHIFT_MASK);
            if keyval == gtk::gdk::Key::Escape && state.preview.is_eyedropper_active() {
                state.cancel_eyedropper();
                return gtk::glib::Propagation::Stop;
            }
            if ctrl && keyval == gtk::gdk::Key::v {
                state.paste_from_clipboard();
                return gtk::glib::Propagation::Stop;
            }
            if ctrl && keyval == gtk::gdk::Key::c {
                state.copy_result_image();
                return gtk::glib::Propagation::Stop;
            }
            if ctrl && !state.preview.is_eyedropper_active() {
                let is_z = keyval == gtk::gdk::Key::z || keyval == gtk::gdk::Key::Z;
                let is_y = keyval == gtk::gdk::Key::y || keyval == gtk::gdk::Key::Y;
                // While typing in a text field, Ctrl+Z/Y belong to the text field.
                let typing = kc
                    .widget()
                    .and_then(|w| w.root())
                    .and_then(|r| r.focus())
                    .map(|f| f.is::<gtk::Text>() || f.is::<gtk::TextView>())
                    .unwrap_or(false);
                if (is_z || is_y) && typing {
                    return gtk::glib::Propagation::Proceed;
                }
                if is_z {
                    if shift {
                        state.redo();
                    } else {
                        state.undo();
                    }
                    return gtk::glib::Propagation::Stop;
                }
                if is_y {
                    state.redo();
                    return gtk::glib::Propagation::Stop;
                }
            }
            if ctrl && (keyval == gtk::gdk::Key::Return || keyval == gtk::gdk::Key::KP_Enter) {
                state.trigger_active_process();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
    }
    window.add_controller(key_ctrl);

    window.present();

    if let Some(path) = INITIAL_FILE.get() {
        app_state.set_universal_input(Some(path.clone()));
    }
}
