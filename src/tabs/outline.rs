use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct OutlineTab {
    pub base: BaseTab,
    pub color_btn: gtk::ColorDialogButton,
    pub thick: gtk::Scale,
    pub roundness: gtk::Scale,
    pub smooth: gtk::Scale,
    pub xoff: gtk::Scale,
    pub yoff: gtk::Scale,
    pub transp: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl OutlineTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Outline", Some("Outline"), "Outline", app.clone());

        let color_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        color_row.append(&gtk::Label::new(Some("Outline Color")));
        let dialog = gtk::ColorDialog::new();
        let color_btn = gtk::ColorDialogButton::new(Some(dialog));
        color_btn.set_rgba(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
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

        let thick = base.tracked_slider("Thickness", 1.0, 60.0, 6.0);
        let roundness = base.tracked_slider("Roundness", 0.0, 20.0, 0.0);
        let smooth = base.tracked_slider("Smoothness", 0.0, 20.0, 0.0);
        let xoff = base.tracked_slider("X Offset", -200.0, 200.0, 0.0);
        let yoff = base.tracked_slider("Y Offset", -200.0, 200.0, 0.0);
        let transp = base.tracked_slider("Transparency %", 0.0, 100.0, 100.0);

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "webp"]);
        base.controls.append(&fmt);

        base.register_color("color", &color_btn);
        base.register_slider("thick", &thick);
        base.register_slider("roundness", &roundness);
        base.register_slider("smooth", &smooth);
        base.register_slider("xoff", &xoff);
        base.register_slider("yoff", &yoff);
        base.register_slider("transp", &transp);
        base.register_dropdown("fmt", &fmt);

        Rc::new(Self {
            base,
            color_btn,
            thick,
            roundness,
            smooth,
            xoff,
            yoff,
            transp,
            fmt,
        })
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let fmts = ["png", "webp"];
        let fmt = fmts
            .get(self.fmt.selected() as usize)
            .copied()
            .unwrap_or("png");
        let out = config::tmp_file(&format!(".{}", fmt));

        let (iw, ih) = match crate::util::image_dimensions(&input) {
            Ok(d) => d,
            Err(e) => {
                self.base
                    .set_status(&format!("Cannot read dimensions: {}", e), true);
                return;
            }
        };

        let rgba = self.color_btn.rgba();
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            (rgba.red() * 255.0) as u8,
            (rgba.green() * 255.0) as u8,
            (rgba.blue() * 255.0) as u8,
        );

        let thick = self.thick.value().max(1.0) as i32;
        let roundness = self.roundness.value() as i32;
        let smooth = self.smooth.value() as i32;
        let xo = self.xoff.value() as i32;
        let yo = self.yoff.value() as i32;
        let transp = self.transp.value() as f64;
        let am = (transp / 100.0).clamp(0.0, 1.0);

        let mut inner: Vec<String> = vec![
            "+clone".into(),
            "-alpha".into(),
            "extract".into(),
            "-threshold".into(),
            "50%".into(),
            "-morphology".into(),
            "Dilate".into(),
            format!("Octagon:{}", thick),
        ];

        // Roundness — rounds the outer corners of the outline
        if roundness > 0 {
            inner.extend([
                "-morphology".into(),
                "Smooth".into(),
                format!("Octagon:{}", roundness),
            ]);
        }

        if smooth > 0 {
            inner.push("-blur".into());
            inner.push(format!("0x{}", smooth));
        }
        if xo != 0 || yo != 0 {
            let sx = -xo;
            let sy = -yo;
            inner.extend([
                "-background".into(),
                "none".into(),
                "-extent".into(),
                format!("{}x{}{:+}{:+}", iw, ih, sx, sy),
            ]);
        }
        inner.extend(["-background".into(), hex, "-alpha".into(), "shape".into()]);
        if am < 1.0 {
            inner.extend([
                "-channel".into(),
                "A".into(),
                "-evaluate".into(),
                "multiply".into(),
                format!("{:.3}", am),
                "+channel".into(),
            ]);
        }

        let mut cmd: Vec<String> = vec![
            "magick".into(),
            input.to_string_lossy().into_owned(),
            "(".into(),
        ];
        cmd.extend(inner);
        cmd.extend([
            ")".into(),
            "-compose".into(),
            "DstOver".into(),
            "-composite".into(),
            out.to_string_lossy().into_owned(),
        ]);

        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, vec![cmd]);
    }
}
