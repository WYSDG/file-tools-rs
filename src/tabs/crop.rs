//! Crop & Transform — direct, single-purpose actions (like paint.net):
//! rotate/flip buttons apply immediately, and the crop box, angle, resize and trim each have
//! their own button. Every action becomes one History step. (There is no generic "Process" button.)

use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

const OP_ROTFLIP: u8 = 1;
const OP_ANGLE: u8 = 2;
const OP_CROP: u8 = 4;
const OP_RESIZE: u8 = 8;
const OP_TRIM: u8 = 16;

const CROP_HINT: &str = "Drag on the image to draw a crop box";

pub struct CropTab {
    pub base: BaseTab,
    pub rotate_cw: gtk::Button,
    pub rotate_ccw: gtk::Button,
    pub rotate_180: gtk::Button,
    pub flip_h: gtk::Button,
    pub flip_v: gtk::Button,
    pub angle: gtk::Scale,
    pub apply_angle: gtk::Button,
    pub w_entry: gtk::Entry,
    pub h_entry: gtk::Entry,
    pub apply_resize: gtk::Button,
    pub fuzz: gtk::Scale,
    pub apply_trim: gtk::Button,
    pub fmt: gtk::DropDown,
    pub apply_crop: gtk::Button,
    pub clear_crop: gtk::Button,
    pub crop_info: gtk::Label,
    rotate_quarters: Rc<RefCell<i32>>,
    flip_h_state: Rc<RefCell<bool>>,
    flip_v_state: Rc<RefCell<bool>>,
    app: AppState,
}

impl CropTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Cropped", Some("Cropped"), "Crop & Transform", app.clone());
        // No generic Process button: every action below applies by itself.
        base.go.set_visible(false);

        let section = |text: &str| {
            let l = gtk::Label::new(Some(text));
            l.set_xalign(0.0);
            l.add_css_class("section-label");
            l
        };

        // ---- Rotate & flip (apply immediately) ----
        base.controls.append(&section("ROTATE & FLIP"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let rotate_ccw = gtk::Button::with_label("⟲ 90°");
        let rotate_180 = gtk::Button::with_label("180°");
        let rotate_cw = gtk::Button::with_label("⟳ 90°");
        let flip_h = gtk::Button::with_label("Flip H");
        let flip_v = gtk::Button::with_label("Flip V");
        for b in [&rotate_ccw, &rotate_180, &rotate_cw, &flip_h, &flip_v] {
            row.append(b);
        }
        base.controls.append(&row);

        let angle = base.tracked_slider("Custom Angle", -180.0, 180.0, 0.0);
        let apply_angle = gtk::Button::with_label("Rotate by angle");
        base.controls.append(&apply_angle);

        // ---- Crop region ----
        base.controls.append(&section("CROP"));
        let crop_info = gtk::Label::new(Some(CROP_HINT));
        crop_info.set_xalign(0.0);
        crop_info.set_wrap(true);
        base.controls.append(&crop_info);
        let crop_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let apply_crop = gtk::Button::with_label("Crop to selection");
        apply_crop.add_css_class("suggested-action");
        let clear_crop = gtk::Button::with_label("Clear");
        crop_row.append(&apply_crop);
        crop_row.append(&clear_crop);
        base.controls.append(&crop_row);

        // ---- Resize ----
        base.controls.append(&section("RESIZE (blank = keep)"));
        let rr = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        rr.append(&gtk::Label::new(Some("W")));
        let w_entry = gtk::Entry::new();
        w_entry.set_placeholder_text(Some("auto"));
        w_entry.set_width_chars(5);
        w_entry.set_max_width_chars(5);
        rr.append(&w_entry);
        rr.append(&gtk::Label::new(Some("H")));
        let h_entry = gtk::Entry::new();
        h_entry.set_placeholder_text(Some("auto"));
        h_entry.set_width_chars(5);
        h_entry.set_max_width_chars(5);
        rr.append(&h_entry);
        let apply_resize = gtk::Button::with_label("Resize");
        rr.append(&apply_resize);
        base.controls.append(&rr);

        // ---- Trim ----
        base.controls.append(&section("TRIM"));
        let fuzz = base.tracked_slider("Trim fuzz", 0.0, 20.0, 5.0);
        let apply_trim = gtk::Button::with_label("Trim empty borders");
        base.controls.append(&apply_trim);

        // ---- Format ----
        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_slider("angle", &angle);
        base.register_entry("w", &w_entry);
        base.register_entry("h", &h_entry);
        base.register_slider("fuzz", &fuzz);
        base.register_dropdown("fmt", &fmt);

        let this = Rc::new(Self {
            base: base.clone(),
            rotate_cw,
            rotate_ccw,
            rotate_180,
            flip_h,
            flip_v,
            angle,
            apply_angle,
            w_entry,
            h_entry,
            apply_resize,
            fuzz,
            apply_trim,
            fmt,
            apply_crop,
            clear_crop,
            crop_info,
            rotate_quarters: Rc::new(RefCell::new(0)),
            flip_h_state: Rc::new(RefCell::new(false)),
            flip_v_state: Rc::new(RefCell::new(false)),
            app: app.clone(),
        });

        // ---- rotate / flip: one click = one step ----
        let wire_rotflip = |btn: &gtk::Button, quarters: i32, fh: bool, fv: bool| {
            let t = this.clone();
            let a = app.clone();
            btn.connect_clicked(move |_| {
                if t.base.is_busy() {
                    return;
                }
                *t.rotate_quarters.borrow_mut() = quarters;
                *t.flip_h_state.borrow_mut() = fh;
                *t.flip_v_state.borrow_mut() = fv;
                t.run_ops(&a, OP_ROTFLIP);
            });
        };
        wire_rotflip(&this.rotate_cw, 1, false, false);
        wire_rotflip(&this.rotate_ccw, 3, false, false);
        wire_rotflip(&this.rotate_180, 2, false, false);
        wire_rotflip(&this.flip_h, 0, true, false);
        wire_rotflip(&this.flip_v, 0, false, true);

        // ---- the other actions ----
        for (btn, ops) in [
            (&this.apply_angle, OP_ANGLE),
            (&this.apply_crop, OP_CROP),
            (&this.apply_resize, OP_RESIZE),
            (&this.apply_trim, OP_TRIM),
        ] {
            let t = this.clone();
            let a = app.clone();
            btn.connect_clicked(move |_| t.run_ops(&a, ops));
        }
        {
            let t = this.clone();
            this.clear_crop.connect_clicked(move |_| {
                t.app.preview.set_crop_overlay(None);
                t.crop_info.set_text(CROP_HINT);
            });
        }

        // The crop box itself is just an overlay; applying is an explicit action.
        {
            let t = this.clone();
            app.preview.set_on_crop_changed(Rc::new(move |rect| match rect {
                Some((x, y, w, h)) => t.crop_info.set_text(&format!("Crop: {}×{} at ({}, {})", w, h, x, y)),
                None => t.crop_info.set_text(CROP_HINT),
            }));
        }

        this
    }

    /// Ctrl+Enter: crop to the current box (if there is one).
    pub fn run(self: &Rc<Self>, app: &AppState, _commit: bool) {
        self.run_ops(app, OP_CROP);
    }

    /// Apply only the operations in `ops` to the tool's input; result = one new History step.
    fn run_ops(self: &Rc<Self>, app: &AppState, ops: u8) {
        if self.base.is_busy() {
            self.base.set_status("Still working on the previous action…", true);
            return;
        }
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };

        let use_rot = ops & OP_ROTFLIP != 0;
        let quarters = if use_rot { *self.rotate_quarters.borrow() } else { 0 };
        let flip_h = use_rot && *self.flip_h_state.borrow();
        let flip_v = use_rot && *self.flip_v_state.borrow();
        let ang = if ops & OP_ANGLE != 0 { self.angle.value().round() as i32 } else { 0 };
        let crop = if ops & OP_CROP != 0 {
            app.preview.get_crop_overlay().filter(|(_, _, w, h)| *w > 0 && *h > 0)
        } else {
            None
        };
        let (w_txt, h_txt) = if ops & OP_RESIZE != 0 {
            (self.w_entry.text().trim().to_string(), self.h_entry.text().trim().to_string())
        } else {
            (String::new(), String::new())
        };
        let trim = ops & OP_TRIM != 0;

        // Nothing to do? Say what's missing instead of silently doing nothing.
        let missing = match ops {
            OP_ANGLE if ang == 0 => Some("Move the angle slider first."),
            OP_CROP if crop.is_none() => Some("Drag a crop box on the image first."),
            OP_RESIZE if w_txt.is_empty() && h_txt.is_empty() => Some("Enter a width and/or a height first."),
            _ => None,
        };
        if let Some(msg) = missing {
            self.base.set_status(msg, true);
            return;
        }

        let fmts = ["png", "jpg", "webp"];
        let fmt = fmts.get(self.fmt.selected() as usize).copied().unwrap_or("png");
        let out = config::tmp_file(&format!(".{}", fmt));

        let mut cmd: Vec<String> = vec!["magick".into(), input.to_string_lossy().into_owned()];
        if quarters != 0 {
            cmd.extend(["-rotate".into(), (quarters * 90).to_string()]);
        }
        if flip_h {
            cmd.push("-flop".into());
        }
        if flip_v {
            cmd.push("-flip".into());
        }
        if ang != 0 {
            cmd.extend(["-background".into(), "none".into(), "-rotate".into(), ang.to_string()]);
        }
        if let Some((x, y, w, h)) = crop {
            cmd.extend(["-crop".into(), format!("{}x{}+{}+{}", w, h, x, y), "+repage".into()]);
        }
        if !w_txt.is_empty() || !h_txt.is_empty() {
            let size = if w_txt.is_empty() {
                format!("x{}", h_txt)
            } else if h_txt.is_empty() {
                format!("{}x", w_txt)
            } else {
                format!("{}x{}", w_txt, h_txt)
            };
            cmd.extend(["-resize".into(), size]);
        }
        if trim {
            cmd.extend([
                "-fuzz".into(),
                format!("{}%", self.fuzz.value().round() as i32),
                "-trim".into(),
                "+repage".into(),
            ]);
        }
        if matches!(fmt, "jpg" | "webp") {
            cmd.extend(["-quality".into(), "92".into()]);
        }
        cmd.push(out.to_string_lossy().into_owned());

        // After success the result becomes this tool's input, so everything that referred to the old
        // image's coordinates (the crop box) must be cleared, and the consumed settings reset.
        let base_c = self.base.clone();
        let tc = self.clone();
        let app_c = app.clone();
        let out_cb = out.clone();
        let on_complete: Option<Rc<dyn Fn()>> = Some(Rc::new(move || {
            base_c.set_input(Some(out_cb.clone()), None);
            if ops & OP_ROTFLIP != 0 {
                *tc.rotate_quarters.borrow_mut() = 0;
                *tc.flip_h_state.borrow_mut() = false;
                *tc.flip_v_state.borrow_mut() = false;
            }
            if ops & OP_ANGLE != 0 {
                tc.angle.set_value(0.0);
            }
            if ops & OP_RESIZE != 0 {
                tc.w_entry.set_text("");
                tc.h_entry.set_text("");
            }
            app_c.preview.set_crop_overlay(None);
            tc.crop_info.set_text(CROP_HINT);
        }));
        self.base
            .clone()
            .spawn_processing_with_callback(app.clone(), input, out, vec![cmd], on_complete);
    }
}
