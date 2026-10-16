use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;
use crate::util;
use crate::widgets::mask_canvas::MaskCanvas;

const MODELS: &[(&str, &str)] = &[
    ("BiRefNet General", "birefnet-general"),
    ("BiRefNet Portrait", "birefnet-portrait"),
    ("BiRefNet General Lite", "birefnet-general-lite"),
    ("BiRefNet Massive", "birefnet-massive"),
    ("ISNet General", "isnet-general-use"),
    ("ISNet Anime", "isnet-anime"),
    ("U2Net", "u2net"),
    ("U2Net Human Seg", "u2net_human_seg"),
    ("U2Net Cloth Seg", "u2net_cloth_seg"),
    ("U2Netp (fastest)", "u2netp"),
    ("Silueta (light)", "silueta"),
    ("SAM", "sam"),
    ("BRIA RMBG", "bria-rmbg"),
];

pub struct RemoveTab {
    pub base: BaseTab,
    pub canvas: MaskCanvas,
    pub mode: gtk::DropDown,
    pub model: gtk::DropDown,
    pub alpha: gtk::Switch,
    pub af: gtk::Scale,
    pub ab: gtk::Scale,
    pub ae: gtk::Scale,
    pub brush: gtk::Scale,
    pub blur: gtk::Scale,
    pub fmt: gtk::DropDown,
    pub rembg_path: std::path::PathBuf,
    mask_widgets: Vec<gtk::Widget>,
    ai_widgets: Vec<gtk::Widget>,
}

impl RemoveTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Remove", Some("BgRem"), "Remove Background", app);

        let canvas = MaskCanvas::new();
        base.controls.append(&canvas.widget);

        // --- Mode ---
        let mode_lbl = gtk::Label::new(Some("Mode"));
        mode_lbl.set_xalign(0.0);
        base.controls.append(&mode_lbl);
        let mode = gtk::DropDown::from_strings(&[
            "AI only (whole image)",
            "Mask + AI (paint areas to remove, AI does the rest)",
            "Mask + Erase (paint areas to remove, no AI — fast)",
        ]);
        mode.set_selected(0);
        base.controls.append(&mode);

        // --- Brush ---
        let brush_lbl = gtk::Label::new(Some("Brush size: 40 px"));
        brush_lbl.set_xalign(0.0);
        let brush = gtk::Scale::with_range(gtk::Orientation::Horizontal, 5.0, 200.0, 1.0);
        brush.set_value(40.0);
        brush.set_round_digits(0);
        {
            let c = canvas.clone();
            let l = brush_lbl.clone();
            brush.connect_value_changed(move |s| {
                let v = s.value().round() as i32;
                l.set_text(&format!("Brush size: {} px", v));
                c.set_brush(v);
            });
        }
        base.controls.append(&brush_lbl);
        base.controls.append(&brush);

        // Undo / Redo / Remove Mask
        let undo_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let undo = gtk::Button::with_label("Undo");
        let redo = gtk::Button::with_label("Redo");
        let remove_mask = gtk::Button::with_label("Remove Mask");
        remove_mask.add_css_class("destructive-action");
        remove_mask.set_tooltip_text(Some("Clear all painted mask strokes"));
        undo_row.append(&undo);
        undo_row.append(&redo);
        undo_row.append(&remove_mask);
        base.controls.append(&undo_row);
        {
            let c = canvas.clone();
            undo.connect_clicked(move |_| c.undo());
        }
        {
            let c = canvas.clone();
            redo.connect_clicked(move |_| c.redo());
        }
        {
            let c = canvas.clone();
            remove_mask.connect_clicked(move |_| c.clear());
        }

        let blur = base.tracked_slider("Mask blur / Eraser softness", 0.0, 30.0, 5.0);

        // --- AI controls ---
        let model_lbl = gtk::Label::new(Some("AI model"));
        model_lbl.set_xalign(0.0);
        base.controls.append(&model_lbl);
        let labels: Vec<&str> = MODELS.iter().map(|(l, _)| *l).collect();
        let model = gtk::DropDown::from_strings(&labels);
        // Default to Silueta — much faster on CPU with comparable quality
        // for most images. Users can still pick BiRefNet for tricky edges.
        model.set_selected(10); // Silueta (light)
        base.controls.append(&model);

        let alpha_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        alpha_row.append(&gtk::Label::new(Some("Alpha matting (better edges)")));
        let alpha = gtk::Switch::new();
        alpha_row.append(&alpha);
        base.controls.append(&alpha_row);

        let af = base.tracked_slider("Foreground threshold", 0.0, 255.0, 240.0);
        let ab = base.tracked_slider("Background threshold", 0.0, 255.0, 10.0);
        let ae = base.tracked_slider("Erode size", 0.0, 20.0, 10.0);
        af.set_sensitive(false);
        ab.set_sensitive(false);
        ae.set_sensitive(false);
        {
            let a = af.clone();
            let b = ab.clone();
            let c = ae.clone();
            alpha.connect_state_set(move |_, on| {
                a.set_sensitive(on);
                b.set_sensitive(on);
                c.set_sensitive(on);
                gtk::glib::Propagation::Proceed
            });
        }

        // --- Common: output format + notify + volume ---
        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "webp"]);
        base.controls.append(&fmt);

        let mut mask_widgets: Vec<gtk::Widget> = vec![
            brush_lbl.clone().upcast(),
            brush.clone().upcast(),
            undo_row.clone().upcast(),
        ];
        for w in base.slider_widgets(&blur) {
            mask_widgets.push(w);
        }

        let mut ai_widgets: Vec<gtk::Widget> = vec![
            model_lbl.clone().upcast(),
            model.clone().upcast(),
            alpha_row.clone().upcast(),
        ];
        for w in base.slider_widgets(&af) {
            ai_widgets.push(w);
        }
        for w in base.slider_widgets(&ab) {
            ai_widgets.push(w);
        }
        for w in base.slider_widgets(&ae) {
            ai_widgets.push(w);
        }

        base.register_dropdown("mode", &mode);
        base.register_dropdown("model", &model);
        base.register_switch("alpha", &alpha);
        base.register_slider("af", &af);
        base.register_slider("ab", &ab);
        base.register_slider("ae", &ae);
        base.register_slider("blur", &blur);
        base.register_dropdown("fmt", &fmt);

        let this = Rc::new(Self {
            base,
            canvas,
            mode,
            model,
            alpha,
            af,
            ab,
            ae,
            brush,
            blur,
            fmt,
            rembg_path: config::home()
                .join("bg-remover-env")
                .join("bin")
                .join("rembg"),
            mask_widgets,
            ai_widgets,
        });

        {
            let this_c = this.clone();
            let mode_c = this.mode.clone();
            mode_c.connect_selected_notify(move |_| {
                this_c.update_mode_visibility();
            });
        }
        this.update_mode_visibility();

        this
    }

    fn update_mode_visibility(&self) {
        let m = self.mode.selected();
        let show_mask = m == 1 || m == 2;
        let show_ai = m == 0 || m == 1;
        for w in &self.mask_widgets {
            w.set_visible(show_mask);
        }
        for w in &self.ai_widgets {
            w.set_visible(show_ai);
        }
        // Only allow painting when a mask mode is active
        self.canvas.set_painting_enabled(show_mask);
    }

    pub fn set_input(&self, path: Option<std::path::PathBuf>, display: Option<String>) {
        self.base.set_input(path.clone(), display);
        if let Some(p) = path {
            self.canvas.set_image(&p);
        }
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let mode = self.mode.selected();
        let fmts = ["png", "webp"];
        let fmt = fmts
            .get(self.fmt.selected() as usize)
            .copied()
            .unwrap_or("png");
        let out = config::tmp_file(&format!(".{}", fmt));

        // Sound is a global setting (Settings -> Sound), not a per-tab option.
        let (snd_on, vol) = {
            let s = app.settings.borrow();
            (s.sound_on, s.sound_volume)
        };
        let on_complete: Option<Rc<dyn Fn()>> = if snd_on {
            Some(Rc::new(move || util::play_done_sound(vol)))
        } else {
            None
        };

        let cmds: Vec<Vec<String>> = match mode {
            0 => {
                let model = MODELS
                    .get(self.model.selected() as usize)
                    .map(|(_, id)| *id)
                    .unwrap_or("birefnet-general");
                if !self.rembg_path.exists() {
                    self.base.set_status(
                        &format!("rembg not found at {}", self.rembg_path.display()),
                        true,
                    );
                    return;
                }
                let mut cmd = vec![
                    self.rembg_path.to_string_lossy().into_owned(),
                    "i".into(),
                    "-m".into(),
                    model.into(),
                ];
                if self.alpha.is_active() {
                    cmd.extend([
                        "-a".into(),
                        "-af".into(),
                        (self.af.value().round() as i32).to_string(),
                        "-ab".into(),
                        (self.ab.value().round() as i32).to_string(),
                        "-ae".into(),
                        (self.ae.value().round() as i32).to_string(),
                    ]);
                }
                cmd.push(input.to_string_lossy().into_owned());
                cmd.push(out.to_string_lossy().into_owned());
                vec![cmd]
            }
            1 => {
                if !self.canvas.has_image() {
                    self.base.set_status("Canvas has no image", true);
                    return;
                }
                if !self.canvas.has_strokes() {
                    self.base
                        .set_status("Paint over the parts you want to remove first", true);
                    return;
                }
                if !self.rembg_path.exists() {
                    self.base.set_status(
                        &format!("rembg not found at {}", self.rembg_path.display()),
                        true,
                    );
                    return;
                }
                let model = MODELS
                    .get(self.model.selected() as usize)
                    .map(|(_, id)| *id)
                    .unwrap_or("birefnet-general");
                let mask_path = config::tmp_file(".png");
                if let Err(e) = self.canvas.build_mask(&mask_path) {
                    self.base
                        .set_status(&format!("Mask build failed: {}", e), true);
                    return;
                }
                let mask_final = self.maybe_blur_mask(&mask_path);
                let step1 = config::tmp_file(".png");
                vec![
                    vec![
                        self.rembg_path.to_string_lossy().into_owned(),
                        "i".into(),
                        "-m".into(),
                        model.into(),
                        input.to_string_lossy().into_owned(),
                        step1.to_string_lossy().into_owned(),
                    ],
                    vec![
                        "magick".into(),
                        step1.to_string_lossy().into_owned(),
                        "(".into(),
                        mask_final.to_string_lossy().into_owned(),
                        "-alpha".into(),
                        "copy".into(),
                        ")".into(),
                        "-compose".into(),
                        "DstOut".into(),
                        "-composite".into(),
                        out.to_string_lossy().into_owned(),
                    ],
                ]
            }
            _ => {
                if !self.canvas.has_image() {
                    self.base.set_status("Canvas has no image", true);
                    return;
                }
                if !self.canvas.has_strokes() {
                    self.base
                        .set_status("Paint over the parts you want to remove first", true);
                    return;
                }
                let mask_path = config::tmp_file(".png");
                if let Err(e) = self.canvas.build_mask(&mask_path) {
                    self.base
                        .set_status(&format!("Mask build failed: {}", e), true);
                    return;
                }
                let mask_final = self.maybe_blur_mask(&mask_path);
                vec![vec![
                    "magick".into(),
                    input.to_string_lossy().into_owned(),
                    "(".into(),
                    mask_final.to_string_lossy().into_owned(),
                    "-alpha".into(),
                    "copy".into(),
                    ")".into(),
                    "-compose".into(),
                    "DstOut".into(),
                    "-composite".into(),
                    out.to_string_lossy().into_owned(),
                ]]
            }
        };

        // RemBg leaves the original colours under the transparent pixels; clear them so later tools
        // (halftone, palette, pixelate...) can't "see" the removed background.
        let mut cmds = cmds;
        if matches!(out.extension().and_then(|e| e.to_str()), Some("png") | Some("webp")) {
            cmds.push(vec!["@clean-alpha".into(), out.to_string_lossy().into_owned()]);
        }

        self.base.clone().spawn_processing_with_callback(
            app.clone(),
            input,
            out,
            cmds,
            on_complete,
        );
    }

    fn maybe_blur_mask(&self, mask_path: &std::path::Path) -> std::path::PathBuf {
        let blur = self.blur.value().round() as i32;
        if blur <= 0 {
            return mask_path.to_path_buf();
        }
        let blurred = config::tmp_file(".png");
        let r = std::process::Command::new("magick")
            .args([
                mask_path.to_string_lossy().as_ref(),
                "-blur",
                &format!("0x{}", blur),
                blurred.to_string_lossy().as_ref(),
            ])
            .output();
        match r {
            Ok(o) if o.status.success() => blurred,
            _ => mask_path.to_path_buf(),
        }
    }
}
