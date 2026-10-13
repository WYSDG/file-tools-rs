use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;
use crate::util;

const CHARSETS: &[(&str, &str)] = &[
    ("Classic", "@%#*+=-:. "),
    (
        "Detailed",
        "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ",
    ),
    ("Blocks", "█▓▒░ "),
    ("Shade", "█▉▊▋▌▍▎▏ "),
    ("Minimal", "@#:. "),
    ("Binary", "10 "),
    ("Numbers", "9876543210 "),
    ("Letters", "ZYXWVUTSRQPONMLKJIHGFEDCBA "),
    ("Symbols", "@#$%&*+=~-:. "),
];

pub struct AsciiTab {
    pub base: BaseTab,
    pub charset: gtk::DropDown,
    pub width: gtk::Scale,
    pub font_size: gtk::Scale,
    pub bright: gtk::Scale,
    pub contrast: gtk::Scale,
    pub opacity: gtk::Scale,
    pub invert: gtk::Switch,
    pub fg: gtk::ColorDialogButton,
    pub base_mode: gtk::DropDown,
    pub base_op: gtk::Scale,
    pub copy_btn: gtk::Button,
    pub save_btn: gtk::Button,
    // Latest generated ASCII text, in memory
    pub last_text: Rc<RefCell<String>>,
}

impl AsciiTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Ascii", Some("Ascii"), "ASCII Art", app);

        // 1. Charset
        let c_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        c_row.append(&gtk::Label::new(Some("Charset")));
        let charset_labels: Vec<&str> = CHARSETS.iter().map(|(n, _)| *n).collect();
        let charset = gtk::DropDown::from_strings(&charset_labels);
        c_row.append(&charset);
        base.controls.append(&c_row);

        // 2. Width + font size at top
        let width = base.tracked_slider("Width (chars)", 20.0, 400.0, 120.0);
        let font_size = base.tracked_slider("Font size", 4.0, 48.0, 12.0);

        // 3. Remaining controls
        let bright = base.tracked_slider("Brightness", -100.0, 100.0, 0.0);
        let contrast = base.tracked_slider("Contrast", -100.0, 100.0, 0.0);
        let opacity = base.tracked_slider("Text opacity %", 0.0, 100.0, 100.0);

        let inv_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inv_row.append(&gtk::Label::new(Some("Invert")));
        let invert = gtk::Switch::new();
        invert.set_active(true);
        inv_row.append(&invert);
        base.controls.append(&inv_row);

        let fg_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        fg_row.append(&gtk::Label::new(Some("Text color")));
        let d = gtk::ColorDialog::new();
        let fg = gtk::ColorDialogButton::new(Some(d));
        fg.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
        fg_row.append(&fg);
        base.controls.append(&fg_row);

        // Base layer under the text: none / black & white image / pure black / original image / white.
        let bi_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bi_row.append(&gtk::Label::new(Some("Base")));
        let base_mode = gtk::DropDown::from_strings(&[
            "None (transparent)",
            "Black & white",
            "Pure black",
            "Original image",
            "White",
        ]);
        bi_row.append(&base_mode);
        base.controls.append(&bi_row);
        let base_op = base.tracked_slider("Image opacity %", 0.0, 100.0, 100.0);
        let base_op_widgets = base.slider_widgets(&base_op);
        for w in &base_op_widgets {
            w.set_visible(false);
        }
        {
            let b = base.clone();
            base_mode.connect_selected_notify(move |d| {
                let sel = d.selected();
                let show = sel == 1 || sel == 3;
                for w in &base_op_widgets {
                    w.set_visible(show);
                }
                b.schedule_live_process();
            });
        }

        // Copy / Save buttons
        let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let copy_btn = gtk::Button::with_label("📋 Copy Text");
        let save_btn = gtk::Button::with_label("💾 Save as TXT");
        btn_row.append(&copy_btn);
        btn_row.append(&save_btn);
        base.controls.append(&btn_row);

        let last_text = Rc::new(RefCell::new(String::new()));

        base.register_dropdown("charset", &charset);
        base.register_slider("width", &width);
        base.register_slider("font_size", &font_size);
        base.register_slider("bright", &bright);
        base.register_slider("contrast", &contrast);
        base.register_slider("opacity", &opacity);
        base.register_switch("invert", &invert);
        base.register_color("fg", &fg);
        base.register_dropdown("base_mode", &base_mode);
        base.register_slider("base_op", &base_op);

        Rc::new(Self {
            base,
            charset,
            width,
            font_size,
            bright,
            contrast,
            opacity,
            invert,
            fg,
            base_mode,
            base_op,
            copy_btn,
            save_btn,
            last_text,
        })
    }

    fn charset_str(&self) -> &'static str {
        let idx = self.charset.selected() as usize;
        CHARSETS.get(idx).map(|(_, c)| *c).unwrap_or("@%#*+=-:. ")
    }

    pub fn copy_text(self: &Rc<Self>) {
        let text = self.last_text.borrow().clone();
        if text.is_empty() {
            self.base
                .set_status("Nothing to copy — process an image first", true);
            return;
        }
        if util::copy_text_to_clipboard(&text) {
            self.base.set_status("✓ Copied ASCII text", false);
        } else {
            self.base
                .set_status("Copy failed — is wl-clipboard installed?", true);
        }
    }

    pub fn save_txt(self: &Rc<Self>) {
        let text = self.last_text.borrow().clone();
        if text.is_empty() {
            self.base
                .set_status("Nothing to save — process an image first", true);
            return;
        }
        let stem = self
            .base
            .get_input()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "ascii".into());
        let dst = self.base.save_dir().join(format!("{}_Ascii.txt", stem));
        match std::fs::write(&dst, &text) {
            Ok(_) => self
                .base
                .set_status(&format!("Saved → {}", dst.display()), false),
            Err(e) => self.base.set_status(&format!("Save failed: {}", e), true),
        }
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let out = config::tmp_file(".png");

        let width = self.width.value().round() as u32;
        let bright = self.bright.value().round() as i32;
        let contrast = self.contrast.value().round() as i32;
        let invert = self.invert.is_active();
        let font_size = self.font_size.value().round() as f64;
        let opacity = self.opacity.value() / 100.0;
        let charset = self.charset_str().to_string();
        let fg = self.fg.rgba();
        let fg_rgb = [
            (fg.red() * 255.0) as u8,
            (fg.green() * 255.0) as u8,
            (fg.blue() * 255.0) as u8,
        ];
        let base_mode = self.base_mode.selected();
        let base_op = self.base_op.value() / 100.0;

        self.base.set_status("Processing…", false);
        self.base.go.set_sensitive(false);

        let base = self.base.clone();
        let app = app.clone();
        let input_block = input.clone();
        let out_block = out.clone();
        let out_after = out.clone();
        let last_text = self.last_text.clone();

        gtk::glib::MainContext::default().spawn_local(async move {
            let result: Result<String, String> = gtk::gio::spawn_blocking(move || {
                // Resolve a monospace font via fontconfig instead of a hard-coded path.
                let font = crate::util::resolve_font("DejaVu Sans Mono")
                    .or_else(|| crate::util::resolve_font("monospace"));

                // Measure the real character cell so the text grid has the same aspect ratio as the
                // image (a fixed "2:1" guess is what stretched the picture slightly).
                let metrics = probe_cell_metrics(&font, font_size as i32);
                let text = generate_ascii(&input_block, width, &charset, invert, bright, contrast, metrics);

                // 0 none, 1 B&W image, 2 pure black, 3 original image, 4 white.
                // Image bases render the glyphs on a transparent layer and composite below.
                let bg_arg = match base_mode {
                    0 | 1 | 3 => "none",
                    2 => "black",
                    _ => "white",
                };
                let fill = format!(
                    "#{:02X}{:02X}{:02X}",
                    ((fg_rgb[0] as f64) * opacity) as u8,
                    ((fg_rgb[1] as f64) * opacity) as u8,
                    ((fg_rgb[2] as f64) * opacity) as u8
                );

                let txt_tmp = crate::config::tmp_file(".txt");
                std::fs::write(&txt_tmp, text.trim_end_matches('\n')).map_err(|e| e.to_string())?;

                let mut cmd = vec![
                    "magick".to_string(),
                    "-background".into(),
                    bg_arg.into(),
                    "-fill".into(),
                    fill,
                ];
                if let Some(f) = font {
                    cmd.push("-font".into());
                    cmd.push(f);
                }
                cmd.extend([
                    "-pointsize".to_string(),
                    format!("{}", font_size as i32),
                    format!("label:@{}", txt_tmp.display()),
                    out_block.to_string_lossy().into_owned(),
                ]);

                let output = std::process::Command::new(&cmd[0])
                    .args(&cmd[1..])
                    .output()
                    .map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).to_string());
                }

                if base_mode == 1 || base_mode == 3 {
                    // Put the glyph layer over the (resized) source image.
                    let (gw, gh) = crate::util::image_dimensions(&out_block).map_err(|e| e.to_string())?;
                    let layer = crate::config::tmp_file(".png");
                    std::fs::copy(&out_block, &layer).map_err(|e| e.to_string())?;
                    let mut c2: Vec<String> = vec![
                        "magick".into(),
                        input_block.to_string_lossy().into_owned(),
                        "-alpha".into(),
                        "on".into(),
                    ];
                    if base_mode == 1 {
                        c2.extend(["-colorspace", "Gray", "-colorspace", "sRGB"].map(String::from));
                    }
                    // Fill the text layer's area WITHOUT stretching: scale to cover, then crop the overflow.
                    c2.extend(
                        [
                            "-resize".to_string(),
                            format!("{}x{}^", gw, gh),
                            "-gravity".to_string(),
                            "center".to_string(),
                            "-extent".to_string(),
                            format!("{}x{}", gw, gh),
                            "+repage".to_string(),
                        ],
                    );
                    if base_op < 0.999 {
                        c2.extend(
                            ["-channel", "A", "-evaluate", "Multiply", &format!("{:.3}", base_op), "+channel"]
                                .map(String::from),
                        );
                    }
                    c2.extend([
                        layer.to_string_lossy().into_owned(),
                        "-compose".into(),
                        "Over".into(),
                        "-composite".into(),
                        out_block.to_string_lossy().into_owned(),
                    ]);
                    let o2 = std::process::Command::new(&c2[0])
                        .args(&c2[1..])
                        .output()
                        .map_err(|e| e.to_string())?;
                    if !o2.status.success() {
                        return Err(String::from_utf8_lossy(&o2.stderr).to_string());
                    }
                }
                Ok(text)
            })
            .await
            .unwrap_or_else(|_| Err("task panicked".to_string()));

            base.go.set_sensitive(true);
            match result {
                Ok(text) => {
                    *last_text.borrow_mut() = text;
                    base.set_status("Done.", false);
                    app.commit_history("Ascii", out_after.clone());
                }
                Err(e) => base.set_status(&e, true),
            }
        });
    }
}

fn generate_ascii(
    input: &std::path::Path,
    width: u32,
    charset: &str,
    invert: bool,
    brightness: i32,
    contrast: i32,
    m: CellMetrics,
) -> String {
    use image::imageops::FilterType;
    let img = image::open(input)
        .unwrap_or_else(|_| image::DynamicImage::ImageRgba8(image::RgbaImage::new(1, 1)))
        .to_rgba8();
    let (w0, h0) = img.dimensions();
    if w0 == 0 || h0 == 0 {
        return String::new();
    }

    // Pick the grid (columns x rows) whose *finished text layer* (incl. ImageMagick's fixed padding)
    // best matches the image's aspect ratio. Rows are whole numbers, so let the width flex +-2 columns.
    let img_aspect = w0 as f64 / h0 as f64;
    let (width, height) = ((width.saturating_sub(2).max(8))..=(width + 2))
        .map(|c| {
            let lw = m.pad_w + c as f64 * m.cell_w;
            let r = ((lw / img_aspect - m.pad_h) / m.cell_h).round().max(1.0) as u32;
            let lh = m.pad_h + r as f64 * m.cell_h;
            let err = ((lw / lh) - img_aspect).abs() / img_aspect;
            (err, c, r)
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, c, r)| (c, r))
        .unwrap_or((width, 1));
    let small = image::imageops::resize(&img, width, height, FilterType::Lanczos3);

    let cf = if contrast != 0 {
        (259.0 * (contrast as f64 + 255.0)) / (255.0 * (259.0 - contrast as f64))
    } else {
        1.0
    };

    let chars: Vec<char> = if invert {
        charset.chars().rev().collect()
    } else {
        charset.chars().collect()
    };
    if chars.is_empty() {
        return String::new();
    }
    let n = chars.len();

    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            let px = small.get_pixel(x, y);
            let alpha = px[3];
            if alpha < 32 {
                out.push(' ');
                continue;
            }
            let mut v = 0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64;
            if brightness != 0 {
                v = (v + brightness as f64 * 2.55).clamp(0.0, 255.0);
            }
            if contrast != 0 {
                v = (cf * (v - 128.0) + 128.0).clamp(0.0, 255.0);
            }
            let idx = ((v * n as f64 / 256.0) as usize).min(n - 1);
            out.push(chars[idx]);
        }
        out.push('\n');
    }
    out
}

/// Size of one text cell plus the fixed padding ImageMagick adds around a text block, for the
/// font + size we render with.
#[derive(Clone, Copy)]
struct CellMetrics {
    cell_w: f64,
    cell_h: f64,
    pad_w: f64,
    pad_h: f64,
}

impl CellMetrics {
    const FALLBACK: CellMetrics = CellMetrics { cell_w: 1.0, cell_h: 2.0, pad_w: 0.0, pad_h: 0.0 };
}

/// Measured by rendering two differently sized text blocks (width/height are linear in the
/// column/row count, so the difference gives the cell size and the remainder is the padding).
/// Cached per font+size. Falls back to a 1:2 cell with no padding if anything goes wrong.
fn probe_cell_metrics(font: &Option<String>, pointsize: i32) -> CellMetrics {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, (f64, f64, f64, f64)>>> = OnceLock::new();
    let key = format!("{:?}@{}", font, pointsize);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((cw, ch, pw, ph)) = cache.lock().ok().and_then(|c| c.get(&key).copied()) {
        return CellMetrics { cell_w: cw, cell_h: ch, pad_w: pw, pad_h: ph };
    }

    let measure = |cols: usize, rows: usize| -> Option<(u32, u32)> {
        let text = vec!["M".repeat(cols); rows].join("\n");
        let txt = crate::config::tmp_file(".txt");
        let png = crate::config::tmp_file(".png");
        std::fs::write(&txt, text).ok()?;
        let mut cmd = std::process::Command::new("magick");
        cmd.args(["-background", "none", "-fill", "white"]);
        if let Some(f) = font {
            cmd.args(["-font", f]);
        }
        cmd.args(["-pointsize", &pointsize.max(4).to_string()]);
        cmd.arg(format!("label:@{}", txt.display()));
        cmd.arg(&png);
        let ok = cmd.output().map(|o| o.status.success()).unwrap_or(false);
        let dims = if ok { crate::util::image_dimensions(&png).ok() } else { None };
        let _ = std::fs::remove_file(&txt);
        let _ = std::fs::remove_file(&png);
        dims
    };

    let m = match (measure(40, 4), measure(120, 20)) {
        (Some((w1, h1)), Some((w2, h2))) if w2 > w1 && h2 > h1 => {
            let cell_w = (w2 - w1) as f64 / 80.0;
            let cell_h = (h2 - h1) as f64 / 16.0;
            let aspect = cell_h / cell_w;
            if (1.2..3.5).contains(&aspect) {
                CellMetrics {
                    cell_w,
                    cell_h,
                    pad_w: w1 as f64 - 40.0 * cell_w,
                    pad_h: h1 as f64 - 4.0 * cell_h,
                }
            } else {
                CellMetrics::FALLBACK
            }
        }
        _ => CellMetrics::FALLBACK,
    };
    if let Ok(mut c) = cache.lock() {
        c.insert(key, (m.cell_w, m.cell_h, m.pad_w, m.pad_h));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered text block must have (almost) the same shape as the source image,
    /// otherwise the picture underneath gets stretched.
    #[test]
    fn ascii_layer_matches_image_aspect() {
        let font = crate::util::resolve_font("DejaVu Sans Mono")
            .or_else(|| crate::util::resolve_font("monospace"));
        let metrics = probe_cell_metrics(&font, 12);
        let dir = std::env::temp_dir();
        for (w0, h0, cols) in [(300u32, 200u32, 100u32), (1600, 900, 80), (500, 700, 60)] {
            let inp = dir.join("asc_in.png");
            image::RgbaImage::from_pixel(w0, h0, image::Rgba([120, 120, 120, 255]))
                .save(&inp)
                .unwrap();
            let text = generate_ascii(&inp, cols, " .:-=+*#%@", false, 0, 0, metrics);
            let txt = dir.join("asc.txt");
            let png = dir.join("asc_layer.png");
            std::fs::write(&txt, text.trim_end_matches('\n')).unwrap();
            let st = std::process::Command::new("magick")
                .args(["-background", "none", "-fill", "white", "-pointsize", "12"])
                .args(["-font", font.as_deref().unwrap_or("monospace")])
                .arg(format!("label:@{}", txt.display()))
                .arg(&png)
                .status()
                .unwrap();
            assert!(st.success());
            let (gw, gh) = crate::util::image_dimensions(&png).unwrap();
            println!("grid {}x{} chars", text.lines().map(|l| l.chars().count()).max().unwrap_or(0), text.lines().count());
            let want = w0 as f64 / h0 as f64;
            let err = ((gw as f64 / gh as f64) - want).abs() / want;
            println!("{w0}x{h0}: cell {:.2}x{:.2} pad {:.1}x{:.1}, layer {gw}x{gh}, aspect error {:.2}%", metrics.cell_w, metrics.cell_h, metrics.pad_w, metrics.pad_h, err * 100.0);
            assert!(err < 0.02, "layer aspect off by {:.1}%", err * 100.0);
        }
    }
}
