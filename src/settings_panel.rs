use gtk::prelude::*;
use gtk4 as gtk;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_dir: Option<PathBuf>,
    pub strip_metadata: bool,
    pub keep_date: bool,
    pub keep_camera: bool,
    pub keep_gps: bool,
    pub sound_on: bool,
    pub sound_volume: f64,
    pub default_model: String,
    pub live_preview_delay_ms: u64,
    pub confirm_overwrite: bool,
    pub show_tooltips: bool,
    pub theme: String,
    /// Settings schema version for the theme names (2 = "dark"/"black" naming).
    #[serde(default)]
    pub theme_v: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: None,
            strip_metadata: false,
            keep_date: true,
            keep_camera: true,
            keep_gps: false,
            sound_on: true,
            sound_volume: 0.5,
            default_model: "silueta".to_string(),
            live_preview_delay_ms: 400,
            confirm_overwrite: true,
            show_tooltips: true,
            theme: "blue".to_string(),
            theme_v: 2,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = crate::config::config_dir().join("settings.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(mut s) = serde_json::from_str::<Settings>(&data) {
                // One-time rename: old "black" (grey) -> "dark", old "oled" -> "black".
                if s.theme_v < 2 {
                    s.theme = match s.theme.as_str() {
                        "black" => "dark".to_string(),
                        "oled" => "black".to_string(),
                        t => t.to_string(),
                    };
                    s.theme_v = 2;
                    s.save();
                }
                return s;
            }
        }
        Settings::default()
    }

    pub fn save(&self) {
        let path = crate::config::config_dir().join("settings.json");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn effective_output_dir(&self) -> PathBuf {
        match &self.output_dir {
            Some(p) if p.exists() => p.clone(),
            _ => crate::config::save_dir(),
        }
    }

    pub fn exiftool_args(&self) -> Option<Vec<String>> {
        if !self.strip_metadata {
            return None;
        }
        let mut args: Vec<String> = vec!["-all=".into(), "-overwrite_original".into()];
        let mut keep: Vec<String> = Vec::new();
        if self.keep_date {
            keep.extend([
                "-DateTimeOriginal".into(),
                "-CreateDate".into(),
                "-ModifyDate".into(),
            ]);
        }
        if self.keep_camera {
            keep.extend([
                "-Make".into(),
                "-Model".into(),
                "-LensModel".into(),
                "-FNumber".into(),
                "-ExposureTime".into(),
                "-ISO".into(),
                "-FocalLength".into(),
            ]);
        }
        if self.keep_gps {
            keep.push("-GPS:all".into());
        }
        args.push("-tagsFromFile".into());
        args.push("@".into());
        args.extend(keep);
        Some(args)
    }
}

pub const THEMES: [&str; 5] = ["dark", "black", "white", "blue", "red"];

pub struct Palette {
    pub bg: &'static str,
    pub bg_soft: &'static str,
    pub fg: &'static str,
    pub accent: &'static str,
    pub accent_dim: &'static str,
    /// Colour for controls changed from their default (sliders, labels): must stand out from `accent`.
    pub modified: &'static str,
    /// Text/check-mark colour drawn on top of a solid accent.
    pub on_accent: &'static str,
    pub error: &'static str,
    pub ok: &'static str,
    pub border: &'static str,
    /// Canvas (preview) background as 0..1 RGB.
    pub canvas: [f64; 3],
    /// Whether GTK should use its dark variant for built-in widgets.
    pub dark: bool,
}

pub fn palette(theme: &str) -> Palette {
    match theme {
        // "black": pure #000 panels (saves power on OLED screens), black & white only.
        "black" => Palette {
            bg: "#000000",
            bg_soft: "#0c0c0c",
            fg: "#ececec",
            accent: "#ffffff",
            accent_dim: "rgba(255,255,255,0.14)",
            modified: "#ff9f43",
            on_accent: "#000000",
            error: "#ff5c6c",
            ok: "#4ade80",
            border: "#1f1f1f",
            canvas: [0.0, 0.0, 0.0],
            dark: true,
        },
        // "dark": neutral dark grey, easier on the eyes than pure black. Black & white only.
        "dark" => Palette {
            bg: "#161616",
            bg_soft: "#212121",
            fg: "#e2e2e2",
            accent: "#e8e8e8",
            accent_dim: "rgba(255,255,255,0.12)",
            modified: "#ff9f43",
            on_accent: "#141414",
            error: "#f87171",
            ok: "#4ade80",
            border: "#333333",
            canvas: [0.075, 0.075, 0.075],
            dark: true,
        },
        "white" => Palette {
            bg: "#f1f2f5",
            bg_soft: "#ffffff",
            fg: "#1c1d26",
            accent: "#1a1a1a",
            accent_dim: "rgba(0,0,0,0.09)",
            modified: "#d9730d",
            on_accent: "#ffffff",
            error: "#c0392b",
            ok: "#2f7d4f",
            border: "#cfd2da",
            canvas: [0.89, 0.90, 0.93],
            dark: false,
        },
        "red" => Palette {
            bg: "#12080a",
            bg_soft: "#1f1014",
            fg: "#f2cdd3",
            accent: "#ff6b81",
            accent_dim: "rgba(255,107,129,0.22)",
            modified: "#ffd166",
            on_accent: "#12080a",
            error: "#ffb454",
            ok: "#7ee787",
            border: "#3d2127",
            canvas: [0.07, 0.03, 0.04],
            dark: true,
        },
        // "blue" (default): deep navy with a clear blue accent.
        _ => Palette {
            bg: "#0b0d18",
            bg_soft: "#151827",
            fg: "#cdd6f4",
            accent: "#7aa2f7",
            accent_dim: "rgba(122,162,247,0.24)",
            modified: "#ff9e64",
            on_accent: "#0b0d18",
            error: "#f7768e",
            ok: "#9ece6a",
            border: "#262a3f",
            canvas: [0.040, 0.046, 0.085],
            dark: true,
        },
    }
}

/// Make GTK's own widgets (buttons, entries, dropdowns, scrollbars…) use the
/// light or dark variant that matches the chosen theme, regardless of the system theme.
pub fn apply_color_scheme(theme: &str) {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property("gtk-application-prefer-dark-theme", palette(theme).dark);
    }
}

const THEME_CSS: &str = r#"
@define-color theme_bg_color $bg;
@define-color theme_fg_color $fg;
@define-color theme_base_color $bg_soft;
@define-color theme_text_color $fg;
@define-color theme_selected_bg_color $accent;
@define-color theme_selected_fg_color $on_accent;
@define-color theme_unfocused_bg_color $bg;
@define-color theme_unfocused_fg_color $fg;
@define-color theme_unfocused_base_color $bg_soft;
@define-color theme_unfocused_text_color $fg;
@define-color theme_unfocused_selected_bg_color $accent;
@define-color theme_unfocused_selected_fg_color $on_accent;
@define-color borders $border;
@define-color unfocused_borders $border;
@define-color content_view_bg $bg_soft;
@define-color text_view_bg $bg_soft;
@define-color error_color $error;
@define-color success_color $ok;
@define-color window_bg_color $bg;
@define-color window_fg_color $fg;
@define-color view_bg_color $bg_soft;
@define-color view_fg_color $fg;
@define-color accent_color $accent;
@define-color accent_bg_color $accent;
@define-color accent_fg_color $on_accent;
@define-color popover_bg_color $bg_soft;
@define-color popover_fg_color $fg;
@define-color card_bg_color $bg_soft;
@define-color card_fg_color $fg;

window, window.background, .background { background-color: $bg; color: $fg; }
popover > contents { background-color: $bg_soft; color: $fg; }

/* GTK hard-codes a blue border on these, so recolour border + fill explicitly. */
scale > trough > highlight, progressbar > trough > progress {
    background-image: none;
    background-color: $accent;
    border-color: $accent;
}
button.suggested-action, button.suggested-action:hover, button.suggested-action:active {
    background-image: none;
    background-color: $accent;
    border-color: $accent;
    color: $on_accent;
}
/* (the generic ".inspector-pane label" rule below must not recolour text on the accent button) */
button.suggested-action label, .inspector-pane button.suggested-action label { color: $on_accent; }
check:checked, radio:checked {
    background-image: none;
    background-color: $accent;
    border-color: $accent;
    color: $on_accent;
}
switch:checked { background-image: none; background-color: $accent; border-color: $accent; }
/* Knob contrasts with the track (white-on-white was unreadable on the black/dark themes). */
switch:checked slider { background-image: none; background-color: $on_accent; border-color: $on_accent; }

.status-ok  { color: $ok; }
.status-err { color: $error; }
scale.modified > trough > highlight { background-color: $modified; border-color: $modified; }
scale.modified slider    { background-image: none; background-color: $modified; border-color: $modified; }
label.modified           { color: $modified; }
.section-label {
    color: $accent;
    font-size: 0.72em;
    font-weight: 700;
    letter-spacing: 0.6px;
    margin-top: 10px;
}
.rail-pane { background-color: $bg; border-right: 1px solid $border; }
.inspector-pane { background-color: $bg; border-left: 1px solid $border; }
.top-bar { background-color: $bg_soft; border-bottom: 1px solid $border; }
.rail-btn {
    min-width: 56px;
    min-height: 44px;
    padding: 2px 4px;
    font-size: 0.95em;
    color: $fg;
    border-radius: 6px;
}
.rail-btn:checked, .rail-btn:active {
    background-color: $accent_dim;
    color: $accent;
    box-shadow: inset 3px 0 0 0 $accent;
}
.tool-btn { border-radius: 6px; padding: 3px 8px; min-width: 52px; }
.status-bar { background-color: $bg_soft; border-top: 1px solid $border; padding: 3px 4px; font-size: 0.9em; }
menubar { background-color: $bg_soft; border-bottom: 1px solid $border; }
.history-panel { background-color: $bg; border-top: 1px solid $border; }
.step-current { background-color: $accent_dim; font-weight: 700; }
.step-future { opacity: 0.45; font-style: italic; }
.hist-pane { background-color: $bg_soft; }
.hist-pane label { color: $fg; padding: 2px 6px; }
.hist-pane row:selected { background-color: $accent_dim; }
window, box, notebook, paned, listbox { color: $fg; }
.inspector-pane label { color: $fg; }
"#;

pub fn build_theme_css(theme: &str) -> String {
    let p = palette(theme);
    THEME_CSS
        .replace("$bg_soft", p.bg_soft)
        .replace("$bg", p.bg)
        .replace("$fg", p.fg)
        .replace("$accent_dim", p.accent_dim)
        .replace("$modified", p.modified)
        .replace("$on_accent", p.on_accent)
        .replace("$accent", p.accent)
        .replace("$error", p.error)
        .replace("$ok", p.ok)
        .replace("$border", p.border)
}

pub fn show_settings_popover(parent: &gtk::MenuButton, app: crate::app::AppState) {
    let popover = gtk::Popover::new();
    // Don't rely on the compositor to dismiss it (some Wayland setups never do, and
    // switching windows would close it unexpectedly). We handle closing ourselves.
    popover.set_autohide(false);
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_size_request(360, -1);

    // ---- Header with an explicit close button ----
    {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = gtk::Label::new(Some("SETTINGS"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("section-label");
        header.append(&title);
        let close = gtk::Button::with_label("\u{2715}");
        close.add_css_class("flat");
        close.set_tooltip_text(Some("Close (Esc)"));
        let pop_c = popover.clone();
        close.connect_clicked(move |_| pop_c.popdown());
        header.append(&close);
        vbox.append(&header);
    }

    // ---- Theme ----
    let theme_hdr = gtk::Label::new(Some("THEME"));
    theme_hdr.set_xalign(0.0);
    theme_hdr.add_css_class("section-label");
    vbox.append(&theme_hdr);

    let themes = THEMES;
    let theme_dd = gtk::DropDown::from_strings(&themes);
    let cur_theme = app.settings.borrow().theme.clone();
    if let Some(i) = themes.iter().position(|t| *t == cur_theme) {
        theme_dd.set_selected(i as u32);
    }
    vbox.append(&theme_dd);
    {
        let app_c = app.clone();
        theme_dd.connect_selected_notify(move |dd| {
            let new_theme = themes
                .get(dd.selected() as usize)
                .copied()
                .unwrap_or("blue");
            app_c.settings.borrow_mut().theme = new_theme.to_string();
            app_c.settings.borrow().save();
            apply_color_scheme(new_theme);

            if let Some(old) = app_c.theme_provider.borrow_mut().take() {
                if let Some(display) = gtk::gdk::Display::default() {
                    gtk::style_context_remove_provider_for_display(&display, &old);
                }
            }
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&build_theme_css(new_theme));
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            *app_c.theme_provider.borrow_mut() = Some(provider);

            let canvas_bg = palette(new_theme).canvas;
            app_c.preview.set_bg_color(canvas_bg);
        });
    }

    vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    // ---- Output folder ----
    let out_hdr = gtk::Label::new(Some("OUTPUT FOLDER"));
    out_hdr.set_xalign(0.0);
    out_hdr.add_css_class("section-label");
    vbox.append(&out_hdr);

    let out_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let out_lbl = gtk::Label::new(None);
    out_lbl.set_xalign(0.0);
    out_lbl.set_hexpand(true);
    out_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    let cur_dir = app.settings.borrow().effective_output_dir();
    out_lbl.set_text(&cur_dir.to_string_lossy());
    out_row.append(&out_lbl);

    let pick_dir_btn = gtk::Button::with_label("Change…");
    out_row.append(&pick_dir_btn);
    vbox.append(&out_row);

    {
        let app_c = app.clone();
        let out_lbl_c = out_lbl.clone();
        let parent_c = parent.clone();
        pick_dir_btn.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Choose output folder")
                .build();
            let parent_root = parent_c.root();
            let parent_win: Option<gtk::Window> =
                parent_root.and_then(|r| r.downcast::<gtk::Window>().ok());
            let app_cb = app_c.clone();
            let out_lbl_cb = out_lbl_c.clone();
            dialog.select_folder(
                parent_win.as_ref(),
                gtk::gio::Cancellable::NONE,
                move |res| {
                    if let Ok(file) = res {
                        if let Some(path) = file.path() {
                            app_cb.settings.borrow_mut().output_dir = Some(path.clone());
                            app_cb.settings.borrow().save();
                            out_lbl_cb.set_text(&path.to_string_lossy());
                        }
                    }
                },
            );
        });
    }

    vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    // ---- Metadata ----
    let meta_hdr = gtk::Label::new(Some("METADATA ON SAVE"));
    meta_hdr.set_xalign(0.0);
    meta_hdr.add_css_class("section-label");
    vbox.append(&meta_hdr);

    let strip_cb = gtk::CheckButton::with_label("Strip metadata when saving");
    strip_cb.set_active(app.settings.borrow().strip_metadata);
    vbox.append(&strip_cb);

    let keep_date_cb = gtk::CheckButton::with_label("  Keep date/time");
    keep_date_cb.set_active(app.settings.borrow().keep_date);
    keep_date_cb.set_margin_start(18);
    vbox.append(&keep_date_cb);

    let keep_camera_cb = gtk::CheckButton::with_label("  Keep camera info");
    keep_camera_cb.set_active(app.settings.borrow().keep_camera);
    keep_camera_cb.set_margin_start(18);
    vbox.append(&keep_camera_cb);

    let keep_gps_cb = gtk::CheckButton::with_label("  Keep GPS location");
    keep_gps_cb.set_active(app.settings.borrow().keep_gps);
    keep_gps_cb.set_margin_start(18);
    vbox.append(&keep_gps_cb);

    let update_sub = {
        let kd = keep_date_cb.clone();
        let kc = keep_camera_cb.clone();
        let kg = keep_gps_cb.clone();
        move |on: bool| {
            kd.set_sensitive(on);
            kc.set_sensitive(on);
            kg.set_sensitive(on);
        }
    };
    update_sub(strip_cb.is_active());
    {
        let app_c = app.clone();
        let update_sub = update_sub.clone();
        strip_cb.connect_toggled(move |c| {
            let on = c.is_active();
            app_c.settings.borrow_mut().strip_metadata = on;
            app_c.settings.borrow().save();
            update_sub(on);
        });
    }
    {
        let app_c = app.clone();
        keep_date_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().keep_date = c.is_active();
            app_c.settings.borrow().save();
        });
    }
    {
        let app_c = app.clone();
        keep_camera_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().keep_camera = c.is_active();
            app_c.settings.borrow().save();
        });
    }
    {
        let app_c = app.clone();
        keep_gps_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().keep_gps = c.is_active();
            app_c.settings.borrow().save();
        });
    }

    vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    // ---- Sound ----
    let snd_hdr = gtk::Label::new(Some("SOUND"));
    snd_hdr.set_xalign(0.0);
    snd_hdr.add_css_class("section-label");
    vbox.append(&snd_hdr);

    let sound_cb = gtk::CheckButton::with_label("Play sound when done");
    sound_cb.set_active(app.settings.borrow().sound_on);
    vbox.append(&sound_cb);

    let vol_lbl = gtk::Label::new(None);
    vol_lbl.set_xalign(0.0);
    let vol = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    vol.set_value(app.settings.borrow().sound_volume * 100.0);
    vol.set_round_digits(0);
    vol_lbl.set_text(&format!(
        "Volume: {}%",
        (app.settings.borrow().sound_volume * 100.0) as i32
    ));
    vbox.append(&vol_lbl);
    vbox.append(&vol);
    vol.set_sensitive(sound_cb.is_active());

    {
        let app_c = app.clone();
        let vol_c = vol.clone();
        sound_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().sound_on = c.is_active();
            app_c.settings.borrow().save();
            vol_c.set_sensitive(c.is_active());
        });
    }
    {
        let app_c = app.clone();
        let lbl_c = vol_lbl.clone();
        vol.connect_value_changed(move |s| {
            let v = s.value() / 100.0;
            app_c.settings.borrow_mut().sound_volume = v;
            app_c.settings.borrow().save();
            lbl_c.set_text(&format!("Volume: {}%", (v * 100.0) as i32));
        });
    }

    vbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    // ---- Defaults ----
    let def_hdr = gtk::Label::new(Some("DEFAULTS"));
    def_hdr.set_xalign(0.0);
    def_hdr.add_css_class("section-label");
    vbox.append(&def_hdr);

    let model_lbl = gtk::Label::new(Some("Default AI model"));
    model_lbl.set_xalign(0.0);
    vbox.append(&model_lbl);
    let models = [
        "silueta",
        "u2netp",
        "u2net",
        "u2net_human_seg",
        "isnet-general-use",
        "isnet-anime",
        "birefnet-general",
        "birefnet-portrait",
    ];
    let model_dd = gtk::DropDown::from_strings(&models);
    let cur_model = app.settings.borrow().default_model.clone();
    if let Some(i) = models.iter().position(|m| *m == cur_model) {
        model_dd.set_selected(i as u32);
    }
    vbox.append(&model_dd);
    {
        let app_c = app.clone();
        model_dd.connect_selected_notify(move |dd| {
            if let Some(m) = models.get(dd.selected() as usize) {
                app_c.settings.borrow_mut().default_model = m.to_string();
                app_c.settings.borrow().save();
            }
        });
    }

    // ---- Behavior ----
    let beh_hdr = gtk::Label::new(Some("BEHAVIOR"));
    beh_hdr.set_xalign(0.0);
    beh_hdr.add_css_class("section-label");
    vbox.append(&beh_hdr);

    let confirm_cb = gtk::CheckButton::with_label("Confirm before overwrite");
    confirm_cb.set_active(app.settings.borrow().confirm_overwrite);
    vbox.append(&confirm_cb);
    {
        let app_c = app.clone();
        confirm_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().confirm_overwrite = c.is_active();
            app_c.settings.borrow().save();
        });
    }

    let tip_cb = gtk::CheckButton::with_label("Show tool tooltips");
    tip_cb.set_active(app.settings.borrow().show_tooltips);
    vbox.append(&tip_cb);
    {
        let app_c = app.clone();
        tip_cb.connect_toggled(move |c| {
            app_c.settings.borrow_mut().show_tooltips = c.is_active();
            app_c.settings.borrow().save();
        });
    }

    let delay_lbl = gtk::Label::new(None);
    delay_lbl.set_xalign(0.0);
    let delay = gtk::Scale::with_range(gtk::Orientation::Horizontal, 100.0, 1500.0, 50.0);
    delay.set_value(app.settings.borrow().live_preview_delay_ms as f64);
    delay.set_round_digits(0);
    delay_lbl.set_text(&format!(
        "Live preview delay: {} ms",
        app.settings.borrow().live_preview_delay_ms
    ));
    vbox.append(&delay_lbl);
    vbox.append(&delay);
    {
        let app_c = app.clone();
        let lbl_c = delay_lbl.clone();
        delay.connect_value_changed(move |s| {
            let v = s.value() as u64;
            app_c.settings.borrow_mut().live_preview_delay_ms = v;
            app_c.settings.borrow().save();
            lbl_c.set_text(&format!("Live preview delay: {} ms", v));
        });
    }

    // Close on click outside the popover, or on Escape (the gear button itself still toggles).
    {
        let pop = popover.clone();
        let btn = parent.clone();
        let attached = std::rc::Rc::new(std::cell::Cell::new(false));
        popover.connect_map(move |_| {
            if attached.get() {
                return;
            }
            let Some(root) = btn.root() else { return };
            attached.set(true);
            let win: gtk::Widget = root.upcast();

            let click = gtk::GestureClick::new();
            click.set_button(0);
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            {
                let (pop_c, btn_c, win_c) = (pop.clone(), btn.clone(), win.clone());
                click.connect_pressed(move |_, _, x, y| {
                    if !pop_c.is_visible() {
                        return;
                    }
                    let on_btn = btn_c
                        .compute_bounds(&win_c)
                        .map(|r| r.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)))
                        .unwrap_or(false);
                    if !on_btn {
                        pop_c.popdown();
                    }
                });
            }
            win.add_controller(click);

            let key = gtk::EventControllerKey::new();
            key.set_propagation_phase(gtk::PropagationPhase::Capture);
            {
                let pop_k = pop.clone();
                key.connect_key_pressed(move |_, k, _, _| {
                    if k == gtk::gdk::Key::Escape && pop_k.is_visible() {
                        pop_k.popdown();
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::glib::Propagation::Proceed
                });
            }
            win.add_controller(key);
        });
    }

    popover.set_child(Some(&vbox));
    parent.set_popover(Some(&popover));
}
