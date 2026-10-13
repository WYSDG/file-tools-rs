//! Paperize — gives any image a ripped-paper border and a weathered, grungy look.
//!
//! * **Torn border** (optional): either a paper border around the photo ("photo on a torn sheet")
//!   or the tear eating into the photo's own edges. The tear is hand-ripped looking (multi-scale
//!   noise), with a pale fibrous paper core along the rip and a soft drop shadow.
//! * **Weathering** (one slider): fading and yellowing, stains, mottling, burnt edges, dust,
//!   scratches and creases, all scaled together. A separate grain slider adds film/paper grain.
//!
//! The photo itself is never turned into layers. Rendered in Rust (no ImageMagick) so it can use the
//! fast big-image live preview.

use gtk::prelude::*;
use gtk4 as gtk;
use image::{GrayImage, RgbaImage};
use std::path::Path;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::{BaseTab, RustJob};

#[derive(Clone, Copy)]
pub struct PaperParams {
    /// Torn border on/off (off = weathering only).
    pub border: bool,
    /// 0 = paper border around the photo, 1 = the tear eats into the photo.
    pub style: u32,
    /// Border size as a fraction of the shorter side (0..0.25).
    pub size: f32,
    /// 0..1 tear roughness.
    pub rough: f32,
    /// Width in px (full resolution) of the pale torn-paper core.
    pub rim: f32,
    /// 0..1 shadow strength.
    pub shadow: f32,
    /// Shadow offset in px (full resolution).
    pub thick: f32,
    /// 0 cream, 1 white, 2 kraft, 3 black.
    pub paper: u32,
    /// 0 transparent, 1 dark, 2 white.
    pub bg: u32,
    /// 0..1 weathering amount.
    pub weather: f32,
    /// 0..1 grain.
    pub grain: f32,
}

pub struct PaperizeTab {
    pub base: BaseTab,
    pub border: gtk::Switch,
    pub style: gtk::DropDown,
    pub size: gtk::Scale,
    pub rough: gtk::Scale,
    pub rim: gtk::Scale,
    pub shadow: gtk::Scale,
    pub thick: gtk::Scale,
    pub paper: gtk::DropDown,
    pub bg: gtk::DropDown,
    pub weather: gtk::Scale,
    pub grain: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl PaperizeTab {
    pub fn new(app: AppState) -> Self {
        let base = BaseTab::new("Paperize", Some("Paperize"), "Paperize (torn paper + weathering)", app);

        let row = |label: &str| {
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            r.append(&gtk::Label::new(Some(label)));
            r
        };

        // --- torn border ---
        let border_row = row("Torn paper border");
        let border = gtk::Switch::new();
        border.set_active(true);
        border_row.append(&border);
        base.controls.append(&border_row);

        let style_row = row("Style");
        let style = gtk::DropDown::from_strings(&["Paper border", "Torn photo edges"]);
        style_row.append(&style);
        base.controls.append(&style_row);

        let size = base.tracked_slider("Border size %", 0.0, 25.0, 6.0);
        let rough = base.tracked_slider("Tear roughness %", 0.0, 100.0, 55.0);
        let rim = base.tracked_slider("Torn rim width", 0.0, 40.0, 10.0);
        let shadow = base.tracked_slider("Shadow strength %", 0.0, 100.0, 45.0);
        let thick = base.tracked_slider("Shadow depth", 1.0, 16.0, 5.0);

        let paper_row = row("Paper");
        let paper = gtk::DropDown::from_strings(&["Cream", "White", "Kraft", "Black"]);
        paper_row.append(&paper);
        base.controls.append(&paper_row);

        let bg_row = row("Background");
        let bg = gtk::DropDown::from_strings(&["Transparent", "Dark", "White"]);
        bg_row.append(&bg);
        base.controls.append(&bg_row);

        // Everything that only matters when the torn border is on.
        let mut border_widgets: Vec<gtk::Widget> = vec![
            style_row.clone().upcast(),
            paper_row.clone().upcast(),
            bg_row.clone().upcast(),
        ];
        for s in [&size, &rough, &rim, &shadow, &thick] {
            border_widgets.extend(base.slider_widgets(s));
        }
        {
            let widgets = border_widgets.clone();
            border.connect_active_notify(move |sw| {
                for w in &widgets {
                    w.set_visible(sw.is_active());
                }
            });
        }

        // --- weathering ---
        let weather = base.tracked_slider("Weathering %", 0.0, 100.0, 40.0);
        let grain = base.tracked_slider("Grain %", 0.0, 100.0, 25.0);

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_switch("border", &border);
        base.register_dropdown("style", &style);
        base.register_slider("size", &size);
        base.register_slider("rough", &rough);
        base.register_slider("rim", &rim);
        base.register_slider("shadow", &shadow);
        base.register_slider("thick", &thick);
        base.register_dropdown("paper", &paper);
        base.register_dropdown("bg", &bg);
        base.register_slider("weather", &weather);
        base.register_slider("grain", &grain);
        base.register_dropdown("fmt", &fmt);

        {
            let b = base.clone();
            border.connect_active_notify(move |_| b.schedule_live_process());
            for d in [&style, &paper, &bg] {
                let b = base.clone();
                d.connect_selected_notify(move |_| b.schedule_live_process());
            }
        }

        Self { base, border, style, size, rough, rim, shadow, thick, paper, bg, weather, grain, fmt }
    }

    fn params(&self) -> PaperParams {
        PaperParams {
            border: self.border.is_active(),
            style: self.style.selected(),
            size: (self.size.value() / 100.0) as f32,
            rough: (self.rough.value() / 100.0) as f32,
            rim: self.rim.value() as f32,
            shadow: (self.shadow.value() / 100.0) as f32,
            thick: self.thick.value() as f32,
            paper: self.paper.selected(),
            bg: self.bg.selected(),
            weather: (self.weather.value() / 100.0) as f32,
            grain: (self.grain.value() / 100.0) as f32,
        }
    }

    /// Reads the controls now and returns a job that can run on a worker thread.
    fn make_job(&self, full: bool) -> RustJob {
        let p = self.params();
        let fmts = ["png", "jpg", "webp"];
        let fmt = if full {
            fmts.get(self.fmt.selected() as usize).copied().unwrap_or("png")
        } else {
            "png"
        };
        let out = config::tmp_file(&format!(".{}", fmt));
        Box::new(move |input, scale| {
            render_paperize(input, &out, &p, scale as f32)?;
            Ok(out)
        })
    }

    /// Explicit Process button: full resolution, committed to the pipeline.
    pub fn run(self: &Rc<Self>, app: &AppState) {
        let tc = self.clone();
        self.base.run_rust_full(app, Rc::new(move |full| tc.make_job(full)));
    }

    /// Live preview (fast downscaled copy for big images, see `BaseTab::run_live_rust`).
    pub fn run_live(self: &Rc<Self>, app: &AppState) {
        let tc = self.clone();
        self.base.run_live_rust(app, Rc::new(move |full| tc.make_job(full)));
    }
}

// ---------------------------------------------------------------------------
// Noise helpers (deterministic, so the preview doesn't flicker between slider moves)
// ---------------------------------------------------------------------------

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (xi as i32, yi as i32);
    let a = hash2(ix, iy, seed);
    let b = hash2(ix + 1, iy, seed);
    let c = hash2(ix, iy + 1, seed);
    let d = hash2(ix + 1, iy + 1, seed);
    (a + (b - a) * u) + ((c + (d - c) * u) - (a + (b - a) * u)) * v
}

/// Fractal noise, roughly 0..1.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * vnoise(x * freq, y * freq, seed.wrapping_add(o * 101));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lum(c: &[f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// Blur + offset a coverage map to make a drop shadow (returns 0..1 per pixel).
fn shadow_map(cov: &[f32], w: u32, h: u32, off: i32, sigma: f32) -> Vec<f32> {
    let (wu, hu) = (w as usize, h as usize);
    let mut g = GrayImage::new(w, h);
    for y in 0..hu {
        for x in 0..wu {
            let sx = x as i32 - off;
            let sy = y as i32 - off;
            let c = if sx >= 0 && sy >= 0 && (sx as usize) < wu && (sy as usize) < hu {
                cov[sy as usize * wu + sx as usize]
            } else {
                0.0
            };
            g.as_mut()[y * wu + x] = (c * 255.0) as u8;
        }
    }
    let b = if sigma > 0.3 { image::imageops::blur(&g, sigma) } else { g };
    b.as_raw().iter().map(|&v| v as f32 / 255.0).collect()
}


// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn paper_base(kind: u32) -> [f32; 3] {
    match kind {
        1 => [0.97, 0.97, 0.95],
        2 => [0.72, 0.56, 0.38],
        3 => [0.10, 0.10, 0.11],
        _ => [0.94, 0.91, 0.83],
    }
}

/// `scale` = size of `input` relative to the real image (pixel-valued settings are multiplied by it).
/// The output can be larger than the input (the paper border and shadow need room).
pub fn render_paperize(input: &Path, out: &Path, p: &PaperParams, scale: f32) -> Result<(), String> {
    let src = image::open(input).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = src.dimensions();
    let sc = scale.clamp(0.05, 1.0);
    let min_side = w.min(h) as f32;

    // --- geometry -------------------------------------------------------------------------
    let size_px = if p.border { p.size * min_side } else { 0.0 };
    let paper_style = p.style == 0;
    // The sheet's nominal edge = the photo rectangle grown (paper border) or shrunk (torn photo edges).
    let grow = if paper_style { size_px } else { -size_px };
    let amp = if p.border { (p.rough * (0.30 * size_px).max(10.0 * sc)).min(0.12 * min_side) } else { 0.0 };
    let rim = if p.border { p.rim * sc } else { 0.0 };
    let thick = (p.thick * sc).max(1.0);
    let shadow_on = p.border && p.shadow > 0.0;
    let pad = if p.border { amp + rim + if shadow_on { thick * 3.5 } else { 0.0 } + 4.0 } else { 0.0 };
    let ext = (grow.max(0.0) + pad).ceil() as u32;
    let (cw, ch) = (w + 2 * ext, h + 2 * ext);
    let (cwu, chu) = (cw as usize, ch as usize);
    let (ox, oy) = (ext as i32, ext as i32);
    let (wi, hi) = (w as i32, h as i32);

    let (rx0, rx1, ry0, ry1) = if p.border {
        (ox as f32 - grow, (ox + wi) as f32 + grow, oy as f32 - grow, (oy + hi) as f32 + grow)
    } else {
        (-1.0e6, 1.0e6, -1.0e6, 1.0e6)
    };

    let paper_col = paper_base(p.paper);
    let core = [0.97f32, 0.96, 0.92]; // the pale paper core revealed by the tear
    let wa = p.weather.clamp(0.0, 1.0);
    let burn_len = (0.045 * min_side).max(6.0 * sc);

    // --- 1. build the sheet (premultiplied RGBA) -------------------------------------------
    let mut sheet = vec![[0f32; 4]; cwu * chu];
    for y in 0..ch as i32 {
        for x in 0..cw as i32 {
            let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);
            let sd = (xf - rx0).min(rx1 - xf).min(yf - ry0).min(ry1 - yf);
            let (xr, yr) = (xf / sc, yf / sc);

            let near = amp > 0.0 && sd.abs() < amp * 1.6 + rim + 3.0;
            let off = if near {
                amp * ((fbm(xr / 70.0, yr / 70.0, 3, 41) - 0.5) * 1.6
                    + (fbm(xr / 16.0, yr / 16.0, 2, 43) - 0.5) * 0.9
                    + (hash2(x, y, 47) - 0.5) * 0.35)
            } else {
                0.0
            };
            let v = sd + off; // > 0 inside the torn sheet
            let cov = smoothstep(-0.8, 0.8, v);
            if cov <= 0.001 {
                continue;
            }

            let in_photo = x >= ox && x < ox + wi && y >= oy && y < oy + hi;
            let mut col;
            let mut a_src = 1.0f32;
            if in_photo {
                let s = src.get_pixel((x - ox) as u32, (y - oy) as u32);
                a_src = s[3] as f32 / 255.0;
                col = [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0];
            } else {
                col = [0.0; 3];
            }
            if !in_photo || a_src < 1.0 {
                // paper (with fibres), also showing through transparent parts of the photo
                let fibers = (vnoise(xr * 0.7, yr * 0.09, 11) + vnoise(xr * 0.09, yr * 0.7, 13)) * 0.5 - 0.5;
                let fine = hash2(x, y, 5) - 0.5;
                let k = 1.0 + 0.12 * fibers + 0.05 * fine;
                let pc = [paper_col[0] * k, paper_col[1] * k, paper_col[2] * k];
                col = if in_photo { lerp3(pc, col, a_src) } else { pc };
            }
            // pale fibrous core along the rip
            if rim > 0.5 && v < rim {
                let t = (v / rim).clamp(0.0, 1.0);
                let n = (fbm(xr / 9.0, yr / 9.0, 2, 53) - 0.5) * 0.7;
                let m = 1.0 - smoothstep(0.0, 1.0, (t + n).clamp(0.0, 1.0));
                col = lerp3(col, core, m);
            }

            // --- per-pixel weathering ---
            if wa > 0.0 {
                let l = 0.299 * col[0] + 0.587 * col[1] + 0.114 * col[2];
                for c in col.iter_mut() {
                    *c += (l - *c) * 0.30 * wa; // fade
                    *c = 0.5 + (*c - 0.5) * (1.0 - 0.22 * wa); // flatten contrast
                }
                col = [col[0], col[1] * (1.0 - 0.035 * wa), col[2] * (1.0 - 0.11 * wa)]; // yellowing
                let st = smoothstep(0.5, 0.88, fbm(xr / 150.0, yr / 150.0, 4, 23)) * wa * 0.75;
                col = lerp3(col, [col[0] * 0.78, col[1] * 0.62, col[2] * 0.42], st); // stains
                let mo = 1.0 + (fbm(xr / 22.0, yr / 22.0, 3, 29) - 0.5) * 0.22 * wa; // mottling
                col = [col[0] * mo, col[1] * mo, col[2] * mo];
                let edge_d = if p.border {
                    v.max(0.0)
                } else {
                    (x.min(cw as i32 - 1 - x)).min(y.min(ch as i32 - 1 - y)) as f32
                };
                let burn = (-edge_d / burn_len).exp() * wa * 0.55; // scorched edges
                col = [col[0] * (1.0 - burn * 0.30), col[1] * (1.0 - burn * 0.48), col[2] * (1.0 - burn * 0.70)];
                let dark = smoothstep(0.90, 0.96, vnoise(xr * 0.9, yr * 0.9, 77)) * wa; // dust
                let light = smoothstep(0.92, 0.97, vnoise(xr * 0.8 + 50.0, yr * 0.8, 79)) * wa;
                col = lerp3(col, [0.10, 0.08, 0.06], dark * 0.7);
                col = lerp3(col, [1.0, 1.0, 0.95], light * 0.5);
            }

            let alpha = if in_photo { cov * (a_src.max(if p.border { 1.0 } else { 0.0 })) } else { cov };
            let alpha = if p.border { cov } else { alpha };
            sheet[y as usize * cwu + x as usize] = [
                col[0].clamp(0.0, 1.0) * alpha,
                col[1].clamp(0.0, 1.0) * alpha,
                col[2].clamp(0.0, 1.0) * alpha,
                alpha,
            ];
        }
    }

    // --- 2. scratches ------------------------------------------------------------------------
    if wa > 0.05 {
        let n = (((cwu * chu) as f32 / 1.2e5) * wa * 14.0).min(120.0) as u32;
        for s in 0..n {
            let sx = hash2(s as i32, 1, 61) * cw as f32;
            let sy = hash2(s as i32, 2, 61) * ch as f32;
            let ang = hash2(s as i32, 3, 61) * std::f32::consts::PI;
            let len = (30.0 + hash2(s as i32, 4, 61) * 220.0) * sc;
            let light = hash2(s as i32, 5, 61) > 0.35;
            let strength = (0.15 + hash2(s as i32, 6, 61) * 0.35) * wa;
            let steps = len.max(2.0) as i32;
            for t in 0..steps {
                let tf = t as f32;
                let wob = (vnoise(tf * 0.05, s as f32, 83) - 0.5) * 3.0 * sc;
                let px = (sx + ang.cos() * tf - ang.sin() * wob).round() as i32;
                let py = (sy + ang.sin() * tf + ang.cos() * wob).round() as i32;
                if px < 0 || py < 0 || px >= cw as i32 || py >= ch as i32 {
                    continue;
                }
                let c = &mut sheet[py as usize * cwu + px as usize];
                if c[3] < 0.9 {
                    continue;
                }
                let taper = (std::f32::consts::PI * tf / steps as f32).sin();
                let a = strength * taper;
                for k in 0..3 {
                    c[k] = if light { c[k] + (1.0 - c[k]) * a } else { c[k] * (1.0 - a) };
                }
            }
        }
    }

    // --- 3. creases (two soft fold lines) ----------------------------------------------------
    if wa > 0.35 {
        let amt = (wa - 0.2) * 0.2;
        for f in 0..2 {
            let px = (0.25 + hash2(f, 1, 67) * 0.5) * cw as f32;
            let py = (0.25 + hash2(f, 2, 67) * 0.5) * ch as f32;
            let ang = hash2(f, 3, 67) * std::f32::consts::PI;
            let (sn, cs) = (ang.sin(), ang.cos());
            for y in 0..chu {
                for x in 0..cwu {
                    let c = &mut sheet[y * cwu + x];
                    if c[3] < 0.9 {
                        continue;
                    }
                    let d = (x as f32 - px) * sn - (y as f32 - py) * cs;
                    let dark = (-(d / (1.3 * sc.max(0.5))).powi(2)).exp() * amt;
                    let lite = (-((d - 2.6 * sc.max(0.5)) / (1.6 * sc.max(0.5))).powi(2)).exp() * amt * 0.8;
                    for k in 0..3 {
                        c[k] = (c[k] * (1.0 - dark) + (1.0 - c[k]) * lite).clamp(0.0, 1.0);
                    }
                }
            }
        }
    }

    // --- 4. grain -----------------------------------------------------------------------------
    if p.grain > 0.0 {
        for (i, c) in sheet.iter_mut().enumerate() {
            if c[3] < 0.01 {
                continue;
            }
            let (x, y) = ((i % cwu) as i32, (i / cwu) as i32);
            let n = 1.0 + (hash2(x, y, 5) - 0.5) * 0.18 * p.grain;
            c[0] *= n;
            c[1] *= n;
            c[2] *= n;
        }
    }

    // --- 5. shadow + background, then un-premultiply -----------------------------------------
    let alpha_map: Vec<f32> = sheet.iter().map(|c| c[3]).collect();
    let sh = if shadow_on {
        shadow_map(&alpha_map, cw, ch, thick.round().max(1.0) as i32, thick * 1.1)
    } else {
        vec![0.0; cwu * chu]
    };
    let bg_col: Option<[f32; 3]> = match p.bg {
        1 => Some([0.09, 0.09, 0.10]),
        2 => Some([0.96, 0.96, 0.96]),
        _ => None,
    };
    let mut result = RgbaImage::new(cw, ch);
    for (i, d) in result.pixels_mut().enumerate() {
        let s = sheet[i];
        let sa = (sh[i] * p.shadow * 0.75).clamp(0.0, 1.0);
        // layer below the sheet (premultiplied)
        let below = match bg_col {
            Some(b) => [b[0] * (1.0 - sa), b[1] * (1.0 - sa), b[2] * (1.0 - sa), 1.0],
            None => [0.0, 0.0, 0.0, sa],
        };
        let inv = 1.0 - s[3];
        let pr = [s[0] + below[0] * inv, s[1] + below[1] * inv, s[2] + below[2] * inv, s[3] + below[3] * inv];
        let a = pr[3].clamp(0.0, 1.0);
        let un = |v: f32| if a > 0.001 { ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 };
        d.0 = [un(pr[0]), un(pr[1]), un(pr[2]), (a * 255.0).round() as u8];
    }

    let is_jpg = out
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .map(|e| e == "jpg" || e == "jpeg")
        .unwrap_or(false);
    if is_jpg {
        // JPEG has no transparency: flatten onto white.
        let mut flat = image::RgbImage::new(cw, ch);
        for (d, s) in flat.pixels_mut().zip(result.pixels()) {
            let a = s[3] as f32 / 255.0;
            let f = |c: u8| (c as f32 * a + 245.0 * (1.0 - a)).round() as u8;
            d.0 = [f(s[0]), f(s[1]), f(s[2])];
        }
        flat.save(out).map_err(|e| e.to_string())
    } else {
        result.save(out).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> PaperParams {
        PaperParams {
            border: true,
            style: 0,
            size: 0.10,
            rough: 0.5,
            rim: 8.0,
            shadow: 0.5,
            thick: 4.0,
            paper: 0,
            bg: 0,
            weather: 0.0,
            grain: 0.0,
        }
    }

    fn solid(path: &Path, w: u32, h: u32) {
        RgbaImage::from_pixel(w, h, image::Rgba([30, 120, 200, 255])).save(path).unwrap();
    }

    #[test]
    fn paper_border_grows_the_canvas_and_surrounds_the_photo_with_paper() {
        let dir = std::env::temp_dir();
        let (inp, out) = (dir.join("pz2_in.png"), dir.join("pz2_out.png"));
        solid(&inp, 200, 150);
        render_paperize(&inp, &out, &params(), 1.0).unwrap();
        let res = image::open(&out).unwrap().to_rgba8();
        let (cw, ch) = res.dimensions();
        assert!(cw > 200 && ch > 150, "canvas must grow, got {cw}x{ch}");
        assert_eq!(res.get_pixel(0, 0)[3], 0, "far outside the paper is transparent");
        let c = res.get_pixel(cw / 2, ch / 2);
        assert!(c[3] == 255 && c[2] > 150 && c[0] < 80, "photo intact in the middle: {c:?}");
        // halfway into the border zone on the left: cream paper, not photo blue
        let ext = (cw - 200) / 2;
        let b = res.get_pixel(ext - 7, ch / 2);
        assert!(b[3] == 255 && b[0] > 180 && b[1] > 170 && b[2] > 120, "paper border expected, got {b:?}");
    }

    #[test]
    fn torn_photo_edges_eat_into_the_photo() {
        let dir = std::env::temp_dir();
        let (inp, out) = (dir.join("pz3_in.png"), dir.join("pz3_out.png"));
        solid(&inp, 200, 150);
        let mut p = params();
        p.style = 1;
        render_paperize(&inp, &out, &p, 1.0).unwrap();
        let res = image::open(&out).unwrap().to_rgba8();
        let (cw, ch) = res.dimensions();
        let ext = (cw - 200) / 2;
        // the photo's own left edge has been torn away, but the middle survives
        assert!(res.get_pixel(ext + 1, ch / 2)[3] < 80, "edge should be torn off");
        let c = res.get_pixel(cw / 2, ch / 2);
        assert!(c[3] == 255 && c[2] > 150, "middle intact");
    }

    #[test]
    fn weathering_changes_colours_but_not_the_shape() {
        let dir = std::env::temp_dir();
        let (inp, a, b) = (dir.join("pz4_in.png"), dir.join("pz4_a.png"), dir.join("pz4_b.png"));
        solid(&inp, 160, 120);
        let mut p = params();
        p.border = false;
        render_paperize(&inp, &a, &p, 1.0).unwrap();
        p.weather = 0.9;
        render_paperize(&inp, &b, &p, 1.0).unwrap();
        let (ia, ib) = (image::open(&a).unwrap().to_rgba8(), image::open(&b).unwrap().to_rgba8());
        assert_eq!(ia.dimensions(), (160, 120), "no border: same size");
        assert!(ia.pixels().zip(ib.pixels()).all(|(x, y)| x[3] == y[3]), "alpha unchanged");
        let diff = ia.pixels().zip(ib.pixels()).filter(|(x, y)| x != y).count();
        assert!(diff > 160 * 120 / 2, "weathering should visibly change the photo ({diff})");
    }
}

#[cfg(test)]
mod perf {
    use super::*;

    /// Manual timing check: `cargo test paperize_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn paperize_timing() {
        let dir = std::env::temp_dir();
        let big = dir.join("pz_big.png");
        if !big.exists() {
            let mut img = RgbaImage::new(3200, 2000);
            for (x, y, px) in img.enumerate_pixels_mut() {
                px.0 = [(x / 13) as u8, (y / 8) as u8, ((x + y) / 20) as u8, 255];
            }
            img.save(&big).unwrap();
        }
        let p = PaperParams {
            border: true, style: 0, size: 0.06, rough: 0.55, rim: 10.0, shadow: 0.45, thick: 5.0,
            paper: 0, bg: 0, weather: 0.4, grain: 0.25,
        };
        let (proxy, scale) = crate::util::preview_proxy(&big).unwrap();
        let t = std::time::Instant::now();
        render_paperize(&proxy, &dir.join("pz_perf_a.png"), &p, scale as f32).unwrap();
        println!("PREVIEW (1280px): {:?}", t.elapsed());
        let t = std::time::Instant::now();
        render_paperize(&big, &dir.join("pz_perf_b.png"), &p, 1.0).unwrap();
        println!("FULL    (3200x2000): {:?}", t.elapsed());
    }
}

#[cfg(test)]
mod samples {
    use super::*;

    /// Writes sample renders for eyeballing: `cargo test paperize_samples -- --ignored`
    #[test]
    #[ignore]
    fn paperize_samples() {
        let p = PaperParams {
            border: true, style: 1, size: 0.05, rough: 0.8, rim: 14.0, shadow: 0.5, thick: 6.0,
            paper: 0, bg: 1, weather: 0.85, grain: 0.35,
        };
        render_paperize(Path::new("/tmp/landscape.png"), Path::new("/tmp/pz_style_b.png"), &p, 1.0).unwrap();
    }
}
