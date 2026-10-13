use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct AdjustTab {
    pub base: BaseTab,
    pub bright: gtk::Scale,
    pub contrast: gtk::Scale,
    pub gamma: gtk::Scale,
    pub sat: gtk::Scale,
    pub hue: gtk::Scale,
    pub temp: gtk::Scale,
    pub sharp: gtk::Scale,
    pub blur: gtk::Scale,
    pub vignette: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl AdjustTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Adjusted", Some("Adjusted"), "Adjust", app);

        let bright = base.tracked_slider("Brightness", -100.0, 100.0, 0.0);
        let contrast = base.tracked_slider("Contrast", -100.0, 100.0, 0.0);
        let gamma = base.tracked_slider("Gamma x100", 10.0, 300.0, 100.0);
        let sat = base.tracked_slider("Saturation", 0.0, 300.0, 100.0);
        let hue = base.tracked_slider("Hue Shift", 0.0, 360.0, 0.0);
        let temp = base.tracked_slider("Warmth", -100.0, 100.0, 0.0);
        let sharp = base.tracked_slider("Sharpen", 0.0, 10.0, 0.0);
        let blur = base.tracked_slider("Blur", 0.0, 20.0, 0.0);
        let vignette = base.tracked_slider("Vignette", 0.0, 100.0, 0.0);

        use crate::tabs::base::PresetWidget;
        base.register_preset("bright", PresetWidget::Slider(bright.clone()));
        base.register_preset("contrast", PresetWidget::Slider(contrast.clone()));
        base.register_preset("gamma", PresetWidget::Slider(gamma.clone()));
        base.register_preset("sat", PresetWidget::Slider(sat.clone()));
        base.register_preset("hue", PresetWidget::Slider(hue.clone()));
        base.register_preset("temp", PresetWidget::Slider(temp.clone()));
        base.register_preset("sharp", PresetWidget::Slider(sharp.clone()));
        base.register_preset("blur", PresetWidget::Slider(blur.clone()));
        base.register_preset("vignette", PresetWidget::Slider(vignette.clone()));

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_preset("fmt", PresetWidget::DropDown(fmt.clone()));

        Rc::new(Self {
            base,
            bright,
            contrast,
            gamma,
            sat,
            hue,
            temp,
            sharp,
            blur,
            vignette,
            fmt,
        })
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
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
        let cmds = vec![self.build_cmd(&input, &out, fmt, 1.0)];
        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, cmds);
    }

    /// Live preview (slider moves). Big images are previewed on a smaller copy; see
    /// `BaseTab::run_live_proxy`.
    pub fn run_live(self: &Rc<Self>, app: &AppState) {
        let tc = self.clone();
        self.base.run_live_proxy(
            app,
            Rc::new(move |input: &std::path::Path, scale: f64, full: bool| {
                let fmts = ["png", "jpg", "webp"];
                let fmt = if full {
                    fmts.get(tc.fmt.selected() as usize).copied().unwrap_or("png")
                } else {
                    "png"
                };
                let out = config::tmp_file(&format!(".{}", fmt));
                (vec![tc.build_cmd(input, &out, fmt, scale)], out)
            }),
        );
    }

    /// `scale` = size of `input` relative to the real image; pixel-valued effects are scaled by it
    /// so a small preview looks like the full-size result.
    fn build_cmd(
        &self,
        input: &std::path::Path,
        out: &std::path::Path,
        fmt: &str,
        scale: f64,
    ) -> Vec<String> {
        let mut cmd: Vec<String> = vec!["magick".into(), input.to_string_lossy().into_owned()];

        let b = self.bright.value() as i32;
        let c = self.contrast.value() as i32;
        if b != 0 || c != 0 {
            cmd.push("-brightness-contrast".into());
            cmd.push(format!("{}x{}", b, c));
        }

        let g = self.gamma.value() / 100.0;
        if (g - 1.0).abs() > 0.01 {
            cmd.push("-gamma".into());
            cmd.push(format!("{:.3}", g));
        }

        let sat = self.sat.value() as i32;
        let hue = self.hue.value() as i32;
        if sat != 100 || hue != 0 {
            cmd.push("-modulate".into());
            cmd.push(format!("100,{},{:.3}", sat, hue as f64 / 1.8));
        }

        let temp = self.temp.value() as i32;
        if temp != 0 {
            let rs = temp as f64 / 100.0 * 0.15;
            let bs = -rs;
            cmd.extend([
                "-channel".into(),
                "R".into(),
                "-evaluate".into(),
                "multiply".into(),
                format!("{:.3}", 1.0 + rs),
                "+channel".into(),
                "-channel".into(),
                "B".into(),
                "-evaluate".into(),
                "multiply".into(),
                format!("{:.3}", 1.0 + bs),
                "+channel".into(),
            ]);
        }

        let sh = self.sharp.value() as i32;
        if sh > 0 {
            cmd.extend(["-unsharp".into(), format!("0x{}", sh as f64 * scale)]);
        }
        let bl = self.blur.value() as i32;
        if bl > 0 {
            cmd.extend(["-blur".into(), format!("0x{}", bl as f64 * scale)]);
        }
        let vig = self.vignette.value() as i32;
        if vig > 0 {
            cmd.extend([
                "-background".into(),
                "black".into(),
                "-vignette".into(),
                format!("0x{}", (20 + vig) as f64 * scale),
            ]);
        }

        if matches!(fmt, "jpg" | "webp") {
            cmd.extend(["-quality".into(), "92".into()]);
        }
        cmd.push(out.to_string_lossy().into_owned());
        cmd
    }
}
