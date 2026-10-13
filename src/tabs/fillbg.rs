use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct FillBgTab {
    pub base: BaseTab,
    pub mode: gtk::DropDown,
    pub color_btn: gtk::ColorDialogButton,
    pub blur_amt: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl FillBgTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("FillBg", Some("FilledBg"), "Fill Background", app.clone());

        let mode_lbl = gtk::Label::new(Some("Mode"));
        mode_lbl.set_xalign(0.0);
        base.controls.append(&mode_lbl);
        let mode = gtk::DropDown::from_strings(&["Solid Color", "Blur Original"]);
        base.controls.append(&mode);

        let color_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        color_row.append(&gtk::Label::new(Some("Color")));
        let dialog = gtk::ColorDialog::new();
        let color_btn = gtk::ColorDialogButton::new(Some(dialog));
        color_btn.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
        color_row.append(&color_btn);
        color_row.append(&crate::util::make_eyedropper_button(
            color_btn.clone(),
            app.clone(),
        ));
        base.controls.append(&color_row);
        {
            let b = base.clone();
            color_btn.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let blur_amt = base.tracked_slider("Blur radius", 0.0, 100.0, 40.0);

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["jpg", "png", "webp"]);
        base.controls.append(&fmt);

        base.register_dropdown("mode", &mode);
        base.register_color("color", &color_btn);
        base.register_slider("blur_amt", &blur_amt);
        base.register_dropdown("fmt", &fmt);

        Rc::new(Self {
            base,
            mode,
            color_btn,
            blur_amt,
            fmt,
        })
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let fmts = ["jpg", "png", "webp"];
        let fmt = fmts
            .get(self.fmt.selected() as usize)
            .copied()
            .unwrap_or("jpg");
        let out = config::tmp_file(&format!(".{}", fmt));
        let cmds = vec![self.build_cmd(&input, &out, fmt, 1.0)];
        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, cmds);
    }

    /// Live preview (control changes). Big images are previewed on a smaller copy; see
    /// `BaseTab::run_live_proxy`.
    pub fn run_live(self: &Rc<Self>, app: &AppState) {
        let tc = self.clone();
        self.base.run_live_proxy(
            app,
            Rc::new(move |input: &std::path::Path, scale: f64, full: bool| {
                let fmts = ["jpg", "png", "webp"];
                let fmt = if full {
                    fmts.get(tc.fmt.selected() as usize).copied().unwrap_or("jpg")
                } else {
                    "png"
                };
                let out = config::tmp_file(&format!(".{}", fmt));
                (vec![tc.build_cmd(input, &out, fmt, scale)], out)
            }),
        );
    }

    /// `scale` = size of `input` relative to the real image (blur radius is in pixels).
    fn build_cmd(
        &self,
        input: &std::path::Path,
        out: &std::path::Path,
        fmt: &str,
        scale: f64,
    ) -> Vec<String> {
        let rgba = self.color_btn.rgba();
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            (rgba.red() * 255.0) as u8,
            (rgba.green() * 255.0) as u8,
            (rgba.blue() * 255.0) as u8,
        );

        let mut cmd: Vec<String> = vec!["magick".into(), input.to_string_lossy().into_owned()];

        if self.mode.selected() == 0 {
            cmd.extend([
                "-background".into(),
                hex,
                "-alpha".into(),
                "remove".into(),
                "-alpha".into(),
                "off".into(),
            ]);
        } else {
            let bl = self.blur_amt.value() as i32;
            cmd.extend([
                "(".into(),
                "+clone".into(),
                "-background".into(),
                "white".into(),
                "-alpha".into(),
                "remove".into(),
                "-alpha".into(),
                "off".into(),
                "-blur".into(),
                format!("0x{}", bl as f64 * scale),
                ")".into(),
                "+swap".into(),
                "-compose".into(),
                "Over".into(),
                "-composite".into(),
                "-background".into(),
                hex,
                "-alpha".into(),
                "remove".into(),
                "-alpha".into(),
                "off".into(),
            ]);
        }

        if fmt == "jpg" {
            cmd.extend(["-quality".into(), "92".into()]);
        }
        cmd.push(out.to_string_lossy().into_owned());
        cmd
    }
}
