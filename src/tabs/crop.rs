use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct CropTab {
    pub base: BaseTab,
    pub rotate_cw: gtk::Button,
    pub rotate_ccw: gtk::Button,
    pub rotate_180: gtk::Button,
    pub flip_h: gtk::Button,
    pub flip_v: gtk::Button,
    pub angle: gtk::Scale,
    pub w_entry: gtk::Entry,
    pub h_entry: gtk::Entry,
    pub fuzz: gtk::Scale,
    pub do_trim: gtk::Switch,
    pub fmt: gtk::DropDown,
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

        // ---- Rotate & flip ----
        let hdr = gtk::Label::new(Some("ROTATE & FLIP"));
        hdr.set_xalign(0.0);
        hdr.add_css_class("section-label");
        base.controls.append(&hdr);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let rotate_ccw = gtk::Button::with_label("⟲ 90°");
        let rotate_180 = gtk::Button::with_label("180°");
        let rotate_cw = gtk::Button::with_label("⟳ 90°");
        let flip_h = gtk::Button::with_label("Flip H");
        let flip_v = gtk::Button::with_label("Flip V");
        row.append(&rotate_ccw);
        row.append(&rotate_180);
        row.append(&rotate_cw);
        row.append(&flip_h);
        row.append(&flip_v);
        base.controls.append(&row);

        let angle = base.tracked_slider("Custom Angle", -180.0, 180.0, 0.0);

        // ---- Crop region ----
        let hdr2 = gtk::Label::new(Some("CROP REGION"));
        hdr2.set_xalign(0.0);
        hdr2.add_css_class("section-label");
        base.controls.append(&hdr2);

        let crop_info = gtk::Label::new(Some("Drag on the preview to define a crop rectangle"));
        crop_info.set_xalign(0.0);
        crop_info.set_wrap(true);
        crop_info.add_css_class("section-label");
        base.controls.append(&crop_info);

        let clear_crop = gtk::Button::with_label("Clear Crop");
        base.controls.append(&clear_crop);

        // ---- Resize ----
        let hdr3 = gtk::Label::new(Some("RESIZE (blank = keep)"));
        hdr3.set_xalign(0.0);
        hdr3.add_css_class("section-label");
        base.controls.append(&hdr3);

        let rr = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        rr.append(&gtk::Label::new(Some("W")));
        let w_entry = gtk::Entry::new();
        w_entry.set_placeholder_text(Some("auto"));
        rr.append(&w_entry);
        rr.append(&gtk::Label::new(Some("H")));
        let h_entry = gtk::Entry::new();
        h_entry.set_placeholder_text(Some("auto"));
        rr.append(&h_entry);
        base.controls.append(&rr);

        // ---- Trim ----
        let hdr4 = gtk::Label::new(Some("TRIM"));
        hdr4.set_xalign(0.0);
        hdr4.add_css_class("section-label");
        base.controls.append(&hdr4);

        let fuzz = base.tracked_slider("Trim Fuzz", 0.0, 20.0, 5.0);

        let trim_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        trim_row.append(&gtk::Label::new(Some("Trim transparent borders")));
        let do_trim = gtk::Switch::new();
        trim_row.append(&do_trim);
        base.controls.append(&trim_row);

        // ---- Format ----
        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        let rotate_quarters: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));
        let flip_h_state: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let flip_v_state: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        base.register_slider("angle", &angle);
        base.register_entry("w", &w_entry);
        base.register_entry("h", &h_entry);
        base.register_slider("fuzz", &fuzz);
        base.register_switch("do_trim", &do_trim);
        base.register_dropdown("fmt", &fmt);

        let this = Rc::new(Self {
            base: base.clone(),
            rotate_cw,
            rotate_ccw,
            rotate_180,
            flip_h,
            flip_v,
            angle,
            w_entry,
            h_entry,
            fuzz,
            do_trim,
            fmt,
            clear_crop,
            crop_info,
            rotate_quarters: rotate_quarters.clone(),
            flip_h_state: flip_h_state.clone(),
            flip_v_state: flip_v_state.clone(),
            app: app.clone(),
        });

        // ---- Wire rotate/flip ----
        {
            let t = this.clone();
            this.rotate_cw.connect_clicked(move |_| {
                *t.rotate_quarters.borrow_mut() = (*t.rotate_quarters.borrow() + 1) % 4;
                t.base.schedule_live_process();
            });
        }
        {
            let t = this.clone();
            this.rotate_ccw.connect_clicked(move |_| {
                *t.rotate_quarters.borrow_mut() = (*t.rotate_quarters.borrow() + 3) % 4;
                t.base.schedule_live_process();
            });
        }
        {
            let t = this.clone();
            this.rotate_180.connect_clicked(move |_| {
                *t.rotate_quarters.borrow_mut() = (*t.rotate_quarters.borrow() + 2) % 4;
                t.base.schedule_live_process();
            });
        }
        {
            let t = this.clone();
            this.flip_h.connect_clicked(move |_| {
                let cur = *t.flip_h_state.borrow();
                *t.flip_h_state.borrow_mut() = !cur;
                t.base.schedule_live_process();
            });
        }
        {
            let t = this.clone();
            this.flip_v.connect_clicked(move |_| {
                let cur = *t.flip_v_state.borrow();
                *t.flip_v_state.borrow_mut() = !cur;
                t.base.schedule_live_process();
            });
        }

        // ---- Wire resize fields ----
        {
            let t = this.clone();
            this.w_entry
                .connect_changed(move |_| t.base.schedule_live_process());
        }
        {
            let t = this.clone();
            this.h_entry
                .connect_changed(move |_| t.base.schedule_live_process());
        }

        // ---- Wire trim toggle ----
        {
            let t = this.clone();
            this.do_trim.connect_state_set(move |_, _| {
                t.base.schedule_live_process();
                gtk::glib::Propagation::Proceed
            });
        }

        // ---- Clear Crop ----
        {
            let t = this.clone();
            this.clear_crop.connect_clicked(move |_| {
                t.app.preview.set_crop_overlay(None);
                t.crop_info
                    .set_text("Drag on the preview to define a crop rectangle");
                t.base.schedule_live_process();
            });
        }

        // ---- Crop rect callback ----
        {
            let t = this.clone();
            app.preview.set_on_crop_changed(Rc::new(move |rect| {
                match rect {
                    Some((x, y, w, h)) => {
                        t.crop_info
                            .set_text(&format!("Crop: {}×{} at ({}, {})", w, h, x, y));
                    }
                    None => {
                        t.crop_info
                            .set_text("Drag on the preview to define a crop rectangle");
                    }
                }
                t.base.schedule_live_process();
            }));
        }

        // ---- Live preview: PREVIEW-ONLY (does not commit) ----
        this.base.set_live_preview(true, 500);
        {
            let t = this.clone();
            let ac = app.clone();
            this.base
                .set_live_trigger(Rc::new(move || t.run(&ac, true)));
        }

        this
    }

    /// `commit = true` → full pipeline commit (Apply Now).
    /// `commit = false` → preview only (live trigger).
    pub fn run(self: &Rc<Self>, app: &AppState, commit: bool) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let fmts = ["png", "jpg", "webp"];
        let fmt = fmts
            .get(self.fmt.selected() as usize)
            .copied()
            .unwrap_or("png");
        let out = config::tmp_file(&format!(".{}", fmt));

        let mut cmd: Vec<String> = vec!["magick".into(), input.to_string_lossy().into_owned()];

        let quarters = *self.rotate_quarters.borrow();
        if quarters != 0 {
            cmd.extend(["-rotate".into(), (quarters * 90).to_string()]);
        }
        if *self.flip_h_state.borrow() {
            cmd.push("-flop".into());
        }
        if *self.flip_v_state.borrow() {
            cmd.push("-flip".into());
        }

        let ang = self.angle.value().round() as i32;
        if ang != 0 {
            cmd.extend([
                "-background".into(),
                "none".into(),
                "-rotate".into(),
                ang.to_string(),
            ]);
        }

        if let Some((x, y, w, h)) = app.preview.get_crop_overlay() {
            if w > 0 && h > 0 {
                cmd.extend([
                    "-crop".into(),
                    format!("{}x{}+{}+{}", w, h, x, y),
                    "+repage".into(),
                ]);
            }
        }

        let w = self.w_entry.text().to_string();
        let h = self.h_entry.text().to_string();
        let w = w.trim();
        let h = h.trim();
        if !w.is_empty() || !h.is_empty() {
            let size = if w.is_empty() {
                format!("x{}", h)
            } else if h.is_empty() {
                format!("{}x", w)
            } else {
                format!("{}x{}", w, h)
            };
            cmd.extend(["-resize".into(), size]);
        }

        if self.do_trim.is_active() {
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

        if commit {
            // After a successful commit, update crop's own input to the
            // result so the next drag's coordinates are in the right space.
            let base_c = self.base.clone();
            let out_for_cb = out.clone();
            let on_complete: Option<Rc<dyn Fn()>> = Some(Rc::new(move || {
                base_c.set_input(Some(out_for_cb.clone()), None);
            }));
            self.base.clone().spawn_processing_with_callback(
                app.clone(),
                input,
                out,
                vec![cmd],
                on_complete,
            );
        } else {
            self.base
                .clone()
                .spawn_processing_preview(app.clone(), input, out, vec![cmd]);
        }
    }
}
