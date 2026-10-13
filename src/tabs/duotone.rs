use gtk::prelude::*;
use gtk4 as gtk;
use image::{DynamicImage, Rgba, RgbaImage};
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct DuotoneTab {
    pub base: BaseTab,
    pub c1: gtk::ColorDialogButton,
    pub c2: gtk::ColorDialogButton,
    pub intensity: gtk::Scale,
    pub gamma: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl DuotoneTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Duotone", Some("Duotone"), "Duotone", app.clone());

        let row1 = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row1.append(&gtk::Label::new(Some("Shadow")));
        let d1 = gtk::ColorDialog::new();
        let c1 = gtk::ColorDialogButton::new(Some(d1));
        c1.set_rgba(&gtk::gdk::RGBA::new(0.04, 0.24, 0.30, 1.0));
        row1.append(&c1);
        row1.append(&crate::util::make_eyedropper_button(
            c1.clone(),
            app.clone(),
        ));
        base.controls.append(&row1);
        {
            let b = base.clone();
            c1.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let row2 = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row2.append(&gtk::Label::new(Some("Highlight")));
        let d2 = gtk::ColorDialog::new();
        let c2 = gtk::ColorDialogButton::new(Some(d2));
        c2.set_rgba(&gtk::gdk::RGBA::new(1.0, 0.70, 0.42, 1.0));
        row2.append(&c2);
        row2.append(&crate::util::make_eyedropper_button(
            c2.clone(),
            app.clone(),
        ));
        base.controls.append(&row2);
        {
            let b = base.clone();
            c2.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let swap = gtk::Button::with_label("⇅ Swap Colors");
        {
            let c1c = c1.clone();
            let c2c = c2.clone();
            swap.connect_clicked(move |_| {
                let a = c1c.rgba();
                let b = c2c.rgba();
                c1c.set_rgba(&b);
                c2c.set_rgba(&a);
            });
        }
        base.controls.append(&swap);

        let intensity = base.tracked_slider("Intensity", 0.0, 100.0, 100.0);
        let gamma = base.tracked_slider("Midtone Gamma x100", 20.0, 300.0, 100.0);

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_color("c1", &c1);
        base.register_color("c2", &c2);
        base.register_slider("intensity", &intensity);
        base.register_slider("gamma", &gamma);
        base.register_dropdown("fmt", &fmt);

        Rc::new(Self {
            base,
            c1,
            c2,
            intensity,
            gamma,
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

        let r1 = self.c1.rgba();
        let r2 = self.c2.rgba();
        let c1 = [
            (r1.red() * 255.0) as u8,
            (r1.green() * 255.0) as u8,
            (r1.blue() * 255.0) as u8,
        ];
        let c2 = [
            (r2.red() * 255.0) as u8,
            (r2.green() * 255.0) as u8,
            (r2.blue() * 255.0) as u8,
        ];
        let intensity = self.intensity.value() / 100.0;
        let gamma = self.gamma.value() / 100.0;
        let fmt_owned = fmt.to_string();

        self.base.set_status("Processing…", false);
        self.base.go.set_sensitive(false);

        let base = self.base.clone();
        let app = app.clone();

        // Two separate clones for the block and the show_result
        let input_block = input.clone();
        let out_block = out.clone();
        let out_after = out.clone();

        gtk::glib::MainContext::default().spawn_local(async move {
            let result: Result<(), String> = gtk::gio::spawn_blocking(move || {
                render_duotone(
                    &input_block,
                    &out_block,
                    c1,
                    c2,
                    intensity,
                    gamma,
                    &fmt_owned,
                )
            })
            .await
            .unwrap_or_else(|_| Err("task panicked".to_string()));

            base.go.set_sensitive(true);
            match result {
                Ok(()) => {
                    base.set_status("Done.", false);
                    app.commit_history("Duotone", out_after.clone());
                }
                Err(e) => base.set_status(&e, true),
            }
        });
    }
}

fn render_duotone(
    input: &std::path::Path,
    out: &std::path::Path,
    dark: [u8; 3],
    light: [u8; 3],
    intensity: f64,
    gamma: f64,
    fmt: &str,
) -> Result<(), String> {
    let img = image::open(input).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();

    let mut lut = [0u8; 256];
    for i in 0..256 {
        let v = (i as f64 / 255.0).powf(1.0 / gamma);
        lut[i] = (v * 255.0).clamp(0.0, 255.0) as u8;
    }

    let mut result = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
            let luma = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) as u8;
            let gl = lut[luma as usize];
            let t = gl as f64 / 255.0;
            let mr = dark[0] as f64 + (light[0] as f64 - dark[0] as f64) * t;
            let mg = dark[1] as f64 + (light[1] as f64 - dark[1] as f64) * t;
            let mb = dark[2] as f64 + (light[2] as f64 - dark[2] as f64) * t;
            let fr = (r as f64) * (1.0 - intensity) + mr * intensity;
            let fg = (g as f64) * (1.0 - intensity) + mg * intensity;
            let fb = (b as f64) * (1.0 - intensity) + mb * intensity;
            result.put_pixel(
                x,
                y,
                Rgba([
                    fr.clamp(0.0, 255.0) as u8,
                    fg.clamp(0.0, 255.0) as u8,
                    fb.clamp(0.0, 255.0) as u8,
                    a,
                ]),
            );
        }
    }

    if fmt == "jpg" {
        let mut rgb = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p = result.get_pixel(x, y);
                let a = p[3] as f64 / 255.0;
                let bg = 255.0;
                let r = (p[0] as f64 * a + bg * (1.0 - a)) as u8;
                let g = (p[1] as f64 * a + bg * (1.0 - a)) as u8;
                let b = (p[2] as f64 * a + bg * (1.0 - a)) as u8;
                rgb.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        DynamicImage::ImageRgba8(rgb)
            .save(out)
            .map_err(|e| e.to_string())?;
    } else {
        DynamicImage::ImageRgba8(result)
            .save(out)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
