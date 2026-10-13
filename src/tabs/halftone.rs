use gtk::prelude::*;
use gtk4 as gtk;
use image::{DynamicImage, Rgba, RgbaImage};
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct HalftoneTab {
    pub base: BaseTab,
    pub mode: gtk::DropDown,
    pub shape: gtk::DropDown,
    pub base_res: gtk::Scale,
    pub dot_size: gtk::Scale,
    pub spacing: gtk::Scale,
    pub angle: gtk::Scale,
    pub bright: gtk::Scale,
    pub contrast: gtk::Scale,
    pub invert: gtk::Switch,
    pub fg: gtk::ColorDialogButton,
    pub dot_op: gtk::Scale,
    pub src_color: gtk::Switch,
    pub img_op: gtk::Scale,
    pub bg_mode: gtk::DropDown,
    pub bg_custom: gtk::ColorDialogButton,
    pub bg_custom_row: gtk::Box,
    pub fmt: gtk::DropDown,
}

impl HalftoneTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Halftone", Some("Halftone"), "Halftone", app.clone());

        let mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        mode_row.append(&gtk::Label::new(Some("Mode")));
        let mode = gtk::DropDown::from_strings(&["Dots", "Lines"]);
        mode_row.append(&mode);
        mode_row.append(&gtk::Label::new(Some("Shape")));
        let shape = gtk::DropDown::from_strings(&["Circle", "Square", "Diamond", "Cross"]);
        mode_row.append(&shape);
        base.controls.append(&mode_row);

        let base_res = base.tracked_slider("Base resolution", 200.0, 4000.0, 800.0);
        let dot_size = base.tracked_slider("Dot size", 2.0, 40.0, 10.0);
        let spacing = base.tracked_slider("Spacing", 3.0, 80.0, 7.0);
        let angle = base.tracked_slider("Angle", 0.0, 180.0, 45.0);
        let bright = base.tracked_slider("Brightness", -100.0, 100.0, 0.0);
        let contrast = base.tracked_slider("Contrast", -100.0, 100.0, 0.0);

        let invert_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        invert_row.append(&gtk::Label::new(Some("Invert")));
        let invert = gtk::Switch::new();
        invert_row.append(&invert);
        base.controls.append(&invert_row);

        // Foreground color
        let fg_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        fg_row.append(&gtk::Label::new(Some("Foreground")));
        let d = gtk::ColorDialog::new();
        let fg = gtk::ColorDialogButton::new(Some(d));
        fg.set_rgba(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
        fg_row.append(&fg);
        fg_row.append(&crate::util::make_eyedropper_button(
            fg.clone(),
            app.clone(),
        ));
        base.controls.append(&fg_row);
        {
            let b = base.clone();
            fg.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let dot_op = base.tracked_slider("Dot opacity %", 0.0, 100.0, 100.0);

        let src_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        src_row.append(&gtk::Label::new(Some("Dots use image colors")));
        let src_color = gtk::Switch::new();
        src_row.append(&src_color);
        base.controls.append(&src_row);
        {
            let b = base.clone();
            src_color.connect_active_notify(move |_| b.schedule_live_process());
        }

        // ---- Background selector ----
        let bg_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bg_row.append(&gtk::Label::new(Some("Background")));
        let bg_mode = gtk::DropDown::from_strings(&[
            "None (transparent)",
            "Black & white",
            "Pure black",
            "Original image",
            "White",
            "Custom color",
        ]);
        bg_mode.set_selected(0);
        bg_row.append(&bg_mode);
        base.controls.append(&bg_row);

        let bg_custom_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bg_custom_row.append(&gtk::Label::new(Some("Custom BG")));
        let d2 = gtk::ColorDialog::new();
        let bg_custom = gtk::ColorDialogButton::new(Some(d2));
        bg_custom.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
        bg_custom_row.append(&bg_custom);
        bg_custom_row.set_visible(false);
        base.controls.append(&bg_custom_row);

        // Base-image opacity: only shown when an "Image (...)" background is chosen.
        let img_op = base.tracked_slider("Image opacity %", 0.0, 100.0, 100.0);
        let img_op_widgets = base.slider_widgets(&img_op);
        for w in &img_op_widgets {
            w.set_visible(false);
        }

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "webp"]);
        base.controls.append(&fmt);

        base.register_dropdown("mode", &mode);
        base.register_dropdown("shape", &shape);
        base.register_slider("base_res", &base_res);
        base.register_slider("dot_size", &dot_size);
        base.register_slider("spacing", &spacing);
        base.register_slider("angle", &angle);
        base.register_slider("bright", &bright);
        base.register_slider("contrast", &contrast);
        base.register_switch("invert", &invert);
        base.register_color("fg", &fg);
        base.register_slider("dot_op", &dot_op);
        base.register_dropdown("bg_mode", &bg_mode);
        base.register_color("bg_custom", &bg_custom);
        base.register_dropdown("fmt", &fmt);

        let this = Rc::new(Self {
            base,
            mode,
            shape,
            base_res,
            dot_size,
            spacing,
            angle,
            bright,
            contrast,
            invert,
            fg,
            dot_op,
            src_color,
            img_op,
            bg_mode,
            bg_custom,
            bg_custom_row,
            fmt,
        });

        // Show/hide custom BG row + trigger live preview on change
        {
            let this_c = this.clone();
            let img_w = img_op_widgets.clone();
            this.bg_mode.connect_selected_notify(move |_| {
                this_c
                    .bg_custom_row
                    .set_visible(this_c.bg_mode.selected() == 5);
                let sel = this_c.bg_mode.selected();
                let is_img = sel == 1 || sel == 3;
                for w in &img_w {
                    w.set_visible(is_img);
                }
                this_c.base.schedule_live_process();
            });
        }
        {
            let b = this.base.clone();
            this.bg_custom
                .connect_rgba_notify(move |_| b.schedule_live_process());
        }

        // Live preview trigger for sliders, mode changes
        {
            let this_c = this.clone();
            this.mode
                .connect_selected_notify(move |_| this_c.base.schedule_live_process());
        }
        {
            let this_c = this.clone();
            this.shape
                .connect_selected_notify(move |_| this_c.base.schedule_live_process());
        }
        {
            let b = this.base.clone();
            this.invert.connect_state_set(move |_, _| {
                b.schedule_live_process();
                gtk::glib::Propagation::Proceed
            });
        }

        this
    }

    fn bg_mode_str(&self) -> &'static str {
        match self.bg_mode.selected() {
            0 => "transparent",
            1 => "img_gray",
            2 => "black",
            3 => "img_color",
            4 => "white",
            _ => "custom",
        }
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let live = app.live_run.get();
        let step_params = self.base.get_preset_data();
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

        let fg_rgba = self.fg.rgba();
        let fg_rgb = [
            (fg_rgba.red() * 255.0) as u8,
            (fg_rgba.green() * 255.0) as u8,
            (fg_rgba.blue() * 255.0) as u8,
        ];

        let bg_custom_rgba = self.bg_custom.rgba();
        let bg_custom_rgb = [
            (bg_custom_rgba.red() * 255.0) as u8,
            (bg_custom_rgba.green() * 255.0) as u8,
            (bg_custom_rgba.blue() * 255.0) as u8,
        ];

        let params = HalftoneParams {
            base_res: self.base_res.value() as u32,
            dot_size: self.dot_size.value() as f32,
            spacing: self.spacing.value() as i32,
            angle: self.angle.value() as f64,
            mode: self.mode.selected(),
            shape: self.shape.selected(),
            brightness: self.bright.value() as f32,
            contrast: self.contrast.value() as f32,
            invert: self.invert.is_active(),
            fg: fg_rgb,
            opacity: (self.dot_op.value() / 100.0) as f32,
            max_dim: None,
            bg_mode: self.bg_mode_str().to_string(),
            src_color: self.src_color.is_active(),
            img_opacity: (self.img_op.value() / 100.0) as f32,
            bg_custom: bg_custom_rgb,
        };

        self.base.set_status("Processing…", false);
        self.base.go.set_sensitive(false);

        let base = self.base.clone();
        let app = app.clone();
        let input_block = input.clone();
        let out_block = out.clone();
        let out_after = out.clone();

        gtk::glib::MainContext::default().spawn_local(async move {
            let result: Result<(), String> = gtk::gio::spawn_blocking(move || {
                render_halftone(&input_block, &out_block, &params)
            })
            .await
            .unwrap_or_else(|_| Err("task panicked".to_string()));

            base.go.set_sensitive(true);
            match result {
                Ok(()) => {
                    base.set_status("Done.", false);
                    app.commit_step("Halftone", out_after, step_params, live);
                }
                Err(e) => base.set_status(&e, true),
            }
        });
    }
}

struct HalftoneParams {
    base_res: u32,
    dot_size: f32,
    spacing: i32,
    angle: f64,
    mode: u32,
    shape: u32,
    brightness: f32,
    contrast: f32,
    invert: bool,
    fg: [u8; 3],
    opacity: f32,
    max_dim: Option<u32>,
    bg_mode: String,
    src_color: bool,
    img_opacity: f32,
    bg_custom: [u8; 3],
}

fn render_halftone(
    input: &std::path::Path,
    out: &std::path::Path,
    p: &HalftoneParams,
) -> Result<(), String> {
    use image::imageops::FilterType;
    let img = image::open(input).map_err(|e| e.to_string())?.to_rgba8();
    let (w0, h0) = img.dimensions();

    let cap = p.max_dim.map(|m| p.base_res.min(m)).unwrap_or(p.base_res);
    let longest = w0.max(h0);
    let s = cap as f64 / longest as f64;
    let iw = (w0 as f64 * s).round().max(1.0) as u32;
    let ih = (h0 as f64 * s).round().max(1.0) as u32;
    let resized = image::imageops::resize(&img, iw, ih, FilterType::Lanczos3);

    let mut gray: Vec<f32> = Vec::with_capacity((iw * ih) as usize);
    let cf = if p.contrast != 0.0 {
        (259.0 * (p.contrast + 255.0)) / (255.0 * (259.0 - p.contrast))
    } else {
        1.0
    };
    for y in 0..ih {
        for x in 0..iw {
            let px = resized.get_pixel(x, y);
            let mut v = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
            if p.brightness != 0.0 {
                v = (v + p.brightness * 2.55).clamp(0.0, 255.0);
            }
            if p.contrast != 0.0 {
                v = (cf * (v - 128.0) + 128.0).clamp(0.0, 255.0);
            }
            if p.invert {
                v = 255.0 - v;
            }
            // Transparent pixels get no dot, semi-transparent ones smaller dots. (Tools like RemBg
            // leave the original colours hidden under the transparent pixels, which used to be
            // sampled as if they were visible.)
            v = 255.0 - (255.0 - v) * (px[3] as f32 / 255.0);
            gray.push(v);
        }
    }

    const SS: u32 = 3;
    let cw = iw * SS;
    let ch = ih * SS;
    let mut canvas = vec![0u8; (cw * ch * 4) as usize];

    // Fill background
    if p.bg_mode.starts_with("img_") {
        // Base layer made from the image itself (colour / grayscale / inverted grayscale).
        // Nearest sampling is exact here: the 3x3 downsample at the end averages identical pixels.
        for y in 0..ch {
            let sy = (y / SS).min(ih - 1);
            for x in 0..cw {
                let sx = (x / SS).min(iw - 1);
                let px = resized.get_pixel(sx, sy);
                let luma =
                    (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) as u8;
                let (r, g, b) = match p.bg_mode.as_str() {
                    "img_gray" => (luma, luma, luma),
                    _ => (px[0], px[1], px[2]),
                };
                let a = (px[3] as f32 * p.img_opacity.clamp(0.0, 1.0)) as u8;
                let i = ((y * cw + x) * 4) as usize;
                canvas[i] = r;
                canvas[i + 1] = g;
                canvas[i + 2] = b;
                canvas[i + 3] = a;
            }
        }
    } else {
        let (bg_r, bg_g, bg_b, bg_a) = match p.bg_mode.as_str() {
            "transparent" => (0u8, 0u8, 0u8, 0u8),
            "black" => (0, 0, 0, 255),
            "white" => (255, 255, 255, 255),
            _ => (p.bg_custom[0], p.bg_custom[1], p.bg_custom[2], 255),
        };
        if bg_a > 0 {
            for i in (0..canvas.len()).step_by(4) {
                canvas[i] = bg_r;
                canvas[i + 1] = bg_g;
                canvas[i + 2] = bg_b;
                canvas[i + 3] = bg_a;
            }
        }
    }

    let alpha = (255.0 * p.opacity.clamp(0.0, 1.0)) as u8;
    let fg = [p.fg[0], p.fg[1], p.fg[2], alpha];
    // Dot colour: fixed foreground, or sampled from the image under each dot.
    let pick = |ix: u32, iy: u32| -> [u8; 4] {
        if p.src_color {
            let px = resized.get_pixel(ix.min(iw - 1), iy.min(ih - 1));
            [px[0], px[1], px[2], alpha]
        } else {
            fg
        }
    };

    let rad = p.angle.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();
    let step = p.spacing.max(2);
    let diag = ((iw * iw + ih * ih) as f64).sqrt() as i32 + step * 2;
    let cx = iw as f64 / 2.0;
    let cy = ih as f64 / 2.0;

    if p.mode == 1 {
        let mut v = -diag;
        while v < diag {
            let mut u = -diag;
            while u < diag {
                let x = u as f64 * cos_a - v as f64 * sin_a + cx;
                let y = u as f64 * sin_a + v as f64 * cos_a + cy;
                if x >= 0.0 && x < iw as f64 && y >= 0.0 && y < ih as f64 {
                    let ix = x as u32;
                    let iy = y as u32;
                    let b = gray[(iy * iw + ix) as usize];
                    let thick = (1.0 - b / 255.0) * p.dot_size;
                    if thick >= 0.5 {
                        let r = (thick * SS as f32 / 2.0) as i32;
                        draw_circle(
                            &mut canvas,
                            cw as i32,
                            ch as i32,
                            (x * SS as f64) as i32,
                            (y * SS as f64) as i32,
                            r,
                            pick(ix, iy),
                        );
                    }
                }
                u += step;
            }
            v += step;
        }
    } else {
        for v in (-diag..diag).step_by(step as usize) {
            for u in (-diag..diag).step_by(step as usize) {
                let x = u as f64 * cos_a - v as f64 * sin_a + cx;
                let y = u as f64 * sin_a + v as f64 * cos_a + cy;
                if x < -(step as f64)
                    || x > iw as f64 + step as f64
                    || y < -(step as f64)
                    || y > ih as f64 + step as f64
                {
                    continue;
                }
                let ix = x.round().clamp(0.0, (iw - 1) as f64) as u32;
                let iy = y.round().clamp(0.0, (ih - 1) as f64) as u32;
                let b = gray[(iy * iw + ix) as usize];
                let r = ((1.0 - b / 255.0) * p.dot_size * SS as f32 / 2.0) as i32;
                if r < 1 {
                    continue;
                }
                let px = (x * SS as f64) as i32;
                let py = (y * SS as f64) as i32;
                match p.shape {
                    0 => draw_circle(&mut canvas, cw as i32, ch as i32, px, py, r, pick(ix, iy)),
                    1 => draw_rect(&mut canvas, cw as i32, ch as i32, px, py, r, pick(ix, iy)),
                    2 => draw_diamond(&mut canvas, cw as i32, ch as i32, px, py, r, pick(ix, iy)),
                    _ => draw_cross(&mut canvas, cw as i32, ch as i32, px, py, r, pick(ix, iy)),
                }
            }
        }
    }

    let mut out_img = RgbaImage::new(iw, ih);
    let n = (SS * SS) as u32;
    for y in 0..ih {
        for x in 0..iw {
            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
            for dy in 0..SS {
                for dx in 0..SS {
                    let i = (((y * SS + dy) * cw + (x * SS + dx)) * 4) as usize;
                    r += canvas[i] as u32;
                    g += canvas[i + 1] as u32;
                    b += canvas[i + 2] as u32;
                    a += canvas[i + 3] as u32;
                }
            }
            out_img.put_pixel(
                x,
                y,
                Rgba([(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]),
            );
        }
    }

    DynamicImage::ImageRgba8(out_img)
        .save(out)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn put(canvas: &mut [u8], cw: i32, ch: i32, x: i32, y: i32, fg: [u8; 4]) {
    if x < 0 || x >= cw || y < 0 || y >= ch {
        return;
    }
    let i = ((y * cw + x) * 4) as usize;
    let sa = fg[3] as u32;
    let da = canvas[i + 3] as u32;
    let out_a = sa + da * (255 - sa) / 255;
    if out_a == 0 {
        return;
    }
    for k in 0..3 {
        let s = fg[k] as u32;
        let d = canvas[i + k] as u32;
        let v = (s * sa + d * da * (255 - sa) / 255) / out_a;
        canvas[i + k] = v.min(255) as u8;
    }
    canvas[i + 3] = out_a.min(255) as u8;
}

fn draw_circle(canvas: &mut [u8], cw: i32, ch: i32, cx: i32, cy: i32, r: i32, fg: [u8; 4]) {
    if r < 1 {
        return;
    }
    let r2 = r * r;
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                put(canvas, cw, ch, x, y, fg);
            }
        }
    }
}

fn draw_rect(canvas: &mut [u8], cw: i32, ch: i32, cx: i32, cy: i32, r: i32, fg: [u8; 4]) {
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            put(canvas, cw, ch, x, y, fg);
        }
    }
}

fn draw_diamond(canvas: &mut [u8], cw: i32, ch: i32, cx: i32, cy: i32, r: i32, fg: [u8; 4]) {
    if r < 1 {
        return;
    }
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            if (x - cx).abs() + (y - cy).abs() <= r {
                put(canvas, cw, ch, x, y, fg);
            }
        }
    }
}

fn draw_cross(canvas: &mut [u8], cw: i32, ch: i32, cx: i32, cy: i32, r: i32, fg: [u8; 4]) {
    let w = (r as f32 * 0.4).max(1.0) as i32;
    for x in (cx - r)..=(cx + r) {
        for dy in -(w / 2)..=(w / 2) {
            put(canvas, cw, ch, x, cy + dy, fg);
        }
    }
    for y in (cy - r)..=(cy + r) {
        for dx in -(w / 2)..=(w / 2) {
            put(canvas, cw, ch, cx + dx, y, fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(bg: &str, src_color: bool) -> HalftoneParams {
        HalftoneParams {
            base_res: 100,
            dot_size: 6.0,
            spacing: 6,
            angle: 45.0,
            mode: 0,
            shape: 0,
            brightness: 0.0,
            contrast: 0.0,
            invert: false,
            fg: [0, 0, 0],
            opacity: 1.0,
            max_dim: None,
            bg_mode: bg.to_string(),
            bg_custom: [0, 0, 0],
            src_color,
            img_opacity: 1.0,
        }
    }

    fn render(bg: &str, src_color: bool, name: &str) -> image::RgbaImage {
        let dir = std::env::temp_dir();
        let (inp, out) = (dir.join(format!("{name}_in.png")), dir.join(format!("{name}_out.png")));
        image::RgbaImage::from_pixel(100, 100, image::Rgba([200, 40, 40, 255]))
            .save(&inp)
            .unwrap();
        render_halftone(&inp, &out, &params(bg, src_color)).unwrap();
        image::open(&out).unwrap().to_rgba8()
    }

    fn has(img: &image::RgbaImage, f: impl Fn(&image::Rgba<u8>) -> bool) -> bool {
        img.pixels().any(|p| f(p))
    }

    #[test]
    fn transparent_pixels_get_no_dots() {
        // Left half transparent but with dark colour hidden underneath (as RemBg leaves it),
        // right half opaque and dark.
        let dir = std::env::temp_dir();
        let (inp, out) = (dir.join("ht_alpha_in.png"), dir.join("ht_alpha_out.png"));
        let mut img = image::RgbaImage::new(100, 100);
        for (x, _, px) in img.enumerate_pixels_mut() {
            px.0 = if x < 50 { [10, 10, 10, 0] } else { [10, 10, 10, 255] };
        }
        img.save(&inp).unwrap();
        render_halftone(&inp, &out, &params("transparent", false)).unwrap();
        let res = image::open(&out).unwrap().to_rgba8();
        let (w, _) = res.dimensions();
        let left_ink = res.enumerate_pixels().filter(|(x, _, p)| *x < w / 2 - 6 && p[3] > 0).count();
        let right_ink = res.enumerate_pixels().filter(|(x, _, p)| *x > w / 2 + 6 && p[3] > 0).count();
        assert_eq!(left_ink, 0, "no dots over the transparent half");
        assert!(right_ink > 500, "dots over the opaque half ({right_ink})");
    }

    #[test]
    fn image_base_modes() {
        // base layer visible between the dots
        assert!(has(&render("img_color", false, "ht1"), |p| p.0 == [200, 40, 40, 255]));
        assert!(has(&render("img_gray", false, "ht2"), |p| p.0 == [87, 87, 87, 255]));
        // old modes unchanged: transparent background still has fully transparent pixels
        assert!(has(&render("transparent", false, "ht4"), |p| p[3] == 0));
        // dots take the image colour when enabled (black bg, red dots)
        assert!(has(&render("black", true, "ht5"), |p| p[0] > 150 && p[1] < 80 && p[3] == 255));
        // ...and stay black when disabled
        assert!(!has(&render("black", false, "ht6"), |p| p[0] > 150));
    }
}
