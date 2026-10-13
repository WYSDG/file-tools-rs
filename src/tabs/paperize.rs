//! Paperize — turns any image into a layered paper-cutout artwork.
//!
//! How it works
//! 1. The image is smoothed, then reduced to N flat colours (k-means), sorted dark → light.
//! 2. Each colour becomes one sheet of paper. Sheet `i` covers every pixel whose colour rank is >= i,
//!    so lighter sheets sit on top of darker ones, exactly like stacked cut paper.
//! 3. Sheet edges are blurred and wobbled with noise so they look hand-cut, not pixel-perfect.
//! 4. Every sheet casts a soft drop shadow on the sheets below it and gets a thin highlight on its
//!    lit edge (light comes from the top-left).
//! 5. Optionally a paper texture is added: fibres and grain, plus "weathering" — stains,
//!    brown age spots, yellowing, and white frayed edges — controlled by one slider.
//!
//! Transparent areas of the source stay transparent (handy for cut-out subjects).
//! Everything is rendered in Rust (no ImageMagick) so it can use the fast big-image live preview.

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
    pub layers: usize,
    /// Pre-blur in pixels (at full resolution).
    pub smooth: f32,
    /// 0..1 hand-cut edge wobble.
    pub rough: f32,
    /// 0..1 shadow strength.
    pub shadow: f32,
    /// Paper thickness = shadow offset in pixels (at full resolution).
    pub thick: f32,
    pub gray: bool,
    pub texture: bool,
    /// 0..1
    pub grain: f32,
    /// 0..1
    pub weather: f32,
}

pub struct PaperizeTab {
    pub base: BaseTab,
    pub layers: gtk::Scale,
    pub smooth: gtk::Scale,
    pub rough: gtk::Scale,
    pub shadow: gtk::Scale,
    pub thick: gtk::Scale,
    pub palette: gtk::DropDown,
    pub texture: gtk::Switch,
    pub grain: gtk::Scale,
    pub weather: gtk::Scale,
    pub fmt: gtk::DropDown,
}

impl PaperizeTab {
    pub fn new(app: AppState) -> Self {
        let base = BaseTab::new("Paperize", Some("Paperize"), "Paperize (paper cutout)", app);

        let layers = base.tracked_slider("Paper layers", 2.0, 10.0, 5.0);
        let smooth = base.tracked_slider("Smoothing", 0.0, 20.0, 4.0);
        let rough = base.tracked_slider("Edge roughness %", 0.0, 100.0, 35.0);
        let shadow = base.tracked_slider("Shadow strength %", 0.0, 100.0, 55.0);
        let thick = base.tracked_slider("Paper thickness", 1.0, 16.0, 5.0);

        let pal_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        pal_row.append(&gtk::Label::new(Some("Colors")));
        let palette = gtk::DropDown::from_strings(&["Image colors", "Grayscale"]);
        pal_row.append(&palette);
        base.controls.append(&pal_row);

        let tex_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        tex_row.append(&gtk::Label::new(Some("Paper texture")));
        let texture = gtk::Switch::new();
        texture.set_active(true);
        tex_row.append(&texture);
        base.controls.append(&tex_row);

        let grain = base.tracked_slider("Paper grain %", 0.0, 100.0, 40.0);
        let weather = base.tracked_slider("Weathering %", 0.0, 100.0, 25.0);
        let tex_widgets: Vec<gtk::Widget> = base
            .slider_widgets(&grain)
            .into_iter()
            .chain(base.slider_widgets(&weather))
            .collect();
        {
            let widgets = tex_widgets.clone();
            texture.connect_active_notify(move |s| {
                for w in &widgets {
                    w.set_visible(s.is_active());
                }
            });
        }

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_slider("layers", &layers);
        base.register_slider("smooth", &smooth);
        base.register_slider("rough", &rough);
        base.register_slider("shadow", &shadow);
        base.register_slider("thick", &thick);
        base.register_dropdown("palette", &palette);
        base.register_switch("texture", &texture);
        base.register_slider("grain", &grain);
        base.register_slider("weather", &weather);
        base.register_dropdown("fmt", &fmt);

        {
            let b = base.clone();
            palette.connect_selected_notify(move |_| b.schedule_live_process());
            let b = base.clone();
            texture.connect_active_notify(move |_| b.schedule_live_process());
        }

        Self { base, layers, smooth, rough, shadow, thick, palette, texture, grain, weather, fmt }
    }

    fn params(&self) -> PaperParams {
        PaperParams {
            layers: self.layers.value().round().clamp(2.0, 10.0) as usize,
            smooth: self.smooth.value() as f32,
            rough: (self.rough.value() / 100.0) as f32,
            shadow: (self.shadow.value() / 100.0) as f32,
            thick: self.thick.value() as f32,
            gray: self.palette.selected() == 1,
            texture: self.texture.is_active(),
            grain: (self.grain.value() / 100.0) as f32,
            weather: (self.weather.value() / 100.0) as f32,
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

// ---------------------------------------------------------------------------
// Colour reduction
// ---------------------------------------------------------------------------

fn nearest(centers: &[[f32; 3]], c: &[f32; 3]) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, k) in centers.iter().enumerate() {
        let d = (k[0] - c[0]).powi(2) + (k[1] - c[1]).powi(2) + (k[2] - c[2]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// k-means on a sample of the image. Result is sorted dark → light.
fn kmeans_palette(img: &RgbaImage, k: usize) -> Vec<[f32; 3]> {
    let total = (img.width() * img.height()) as usize;
    let step = (total / 20_000).max(1);
    let mut samples: Vec<[f32; 3]> = img
        .pixels()
        .step_by(step)
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();
    if samples.is_empty() {
        return vec![[128.0; 3]; k];
    }
    samples.sort_by(|a, b| lum(a).partial_cmp(&lum(b)).unwrap_or(std::cmp::Ordering::Equal));
    let mut centers: Vec<[f32; 3]> = (0..k)
        .map(|i| samples[(((i as f32 + 0.5) / k as f32) * samples.len() as f32) as usize % samples.len()])
        .collect();
    for _ in 0..10 {
        let mut sum = vec![[0f32; 3]; k];
        let mut cnt = vec![0u32; k];
        for s in &samples {
            let j = nearest(&centers, s);
            for c in 0..3 {
                sum[j][c] += s[c];
            }
            cnt[j] += 1;
        }
        for j in 0..k {
            if cnt[j] > 0 {
                for c in 0..3 {
                    centers[j][c] = sum[j][c] / cnt[j] as f32;
                }
            }
        }
    }
    centers.sort_by(|a, b| lum(a).partial_cmp(&lum(b)).unwrap_or(std::cmp::Ordering::Equal));
    centers
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Anti-aliased, wobbly coverage of one sheet plus a "frayed edge" band (for weathering).
struct Sheet {
    cov: Vec<f32>,
    band: Vec<f32>,
}

fn shape_sheet(
    mask: &GrayImage,
    sigma: f32,
    rough: f32,
    wob: &[f32],
    off: (usize, usize),
    want_band: bool,
) -> Sheet {
    let (w, h) = mask.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let blurred = if sigma > 0.3 { image::imageops::blur(mask, sigma) } else { mask.clone() };
    let k = (sigma * 2.5).clamp(3.0, 20.0);
    let mut cov = vec![0f32; wu * hu];
    let mut band = if want_band { vec![0f32; wu * hu] } else { Vec::new() };
    for y in 0..hu {
        let wy = (y + off.1) % hu;
        for x in 0..wu {
            let wx = (x + off.0) % wu;
            let v0 = blurred.as_raw()[y * wu + x] as f32 / 255.0;
            let v = v0 + (wob[wy * wu + wx] - 0.5) * rough * 0.9;
            cov[y * wu + x] = ((v - 0.5) * k + 0.5).clamp(0.0, 1.0);
            if want_band {
                band[y * wu + x] = (1.0 - (v0 - 0.5).abs() * 5.0).clamp(0.0, 1.0);
            }
        }
    }
    Sheet { cov, band }
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

/// `scale` = size of `input` relative to the real image (pixel-valued settings are multiplied by it).
pub fn render_paperize(input: &Path, out: &Path, p: &PaperParams, scale: f32) -> Result<(), String> {
    let src = image::open(input).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = src.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let n = p.layers.clamp(2, 10);
    let sc = scale.clamp(0.05, 1.0);

    // --- 1. prepare colours: fill transparent areas with the average colour so blurring doesn't bleed black
    let (mut sr, mut sg, mut sb, mut cnt) = (0f64, 0f64, 0f64, 0f64);
    for px in src.pixels().step_by(((wu * hu) / 5000).max(1)) {
        if px[3] > 200 {
            sr += px[0] as f64;
            sg += px[1] as f64;
            sb += px[2] as f64;
            cnt += 1.0;
        }
    }
    let mean = if cnt > 0.0 { [sr / cnt, sg / cnt, sb / cnt] } else { [128.0; 3] };
    let mut work = RgbaImage::new(w, h);
    for (d, s) in work.pixels_mut().zip(src.pixels()) {
        let a = s[3] as f64 / 255.0;
        let mixc = |c: usize| (s[c] as f64 * a + mean[c] * (1.0 - a)).round() as u8;
        let (mut r, mut g, mut b) = (mixc(0), mixc(1), mixc(2));
        if p.gray {
            let l = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
            r = l;
            g = l;
            b = l;
        }
        d.0 = [r, g, b, 255];
    }

    // --- 2. smooth, then reduce to N colours
    let smooth = p.smooth * sc;
    let smoothed = if smooth > 0.3 { image::imageops::blur(&work, smooth) } else { work };
    let palette = kmeans_palette(&smoothed, n);
    let idx: Vec<u8> = smoothed
        .pixels()
        .map(|px| nearest(&palette, &[px[0] as f32, px[1] as f32, px[2] as f32]) as u8)
        .collect();

    // --- 3. noise fields shared by all sheets (coordinates in "real" pixels so preview == final)
    let wob: Vec<f32> = (0..wu * hu)
        .map(|i| {
            let (x, y) = ((i % wu) as f32 / sc, (i / wu) as f32 / sc);
            fbm(x / 11.0, y / 11.0, 2, 7)
        })
        .collect();

    // --- 4. stack the sheets (premultiplied RGBA floats)
    let mut canvas = vec![[0f32; 4]; wu * hu];
    let edge_sigma = (1.2 + smooth * 0.35).max(1.0);
    let thick = (p.thick * sc).max(1.0);
    let off = thick.round().max(1.0) as i32;
    let hl_off = ((thick * 0.45).round() as i32).max(1);
    let paper_white = [0.97f32, 0.95, 0.90];

    for layer in 0..n {
        let col = [palette[layer][0] / 255.0, palette[layer][1] / 255.0, palette[layer][2] / 255.0];

        // sheet shape
        let sheet = if layer == 0 {
            // base sheet = the source's own silhouette (fully opaque unless the image has transparency)
            Sheet { cov: src.pixels().map(|px| px[3] as f32 / 255.0).collect(), band: Vec::new() }
        } else {
            let mut m = GrayImage::new(w, h);
            for i in 0..wu * hu {
                if idx[i] as usize >= layer && src.as_raw()[i * 4 + 3] > 127 {
                    m.as_mut()[i] = 255;
                }
            }
            let offset = ((layer * 37) % wu.max(1), (layer * 61) % hu.max(1));
            shape_sheet(&m, edge_sigma, p.rough, &wob, offset, p.texture && p.weather > 0.0)
        };

        // drop shadow onto the sheets below
        if layer > 0 && p.shadow > 0.0 {
            let sh = shadow_map(&sheet.cov, w, h, off, thick * 0.9);
            for i in 0..wu * hu {
                if canvas[i][3] > 0.001 {
                    let k = 1.0 - sh[i] * p.shadow * 0.75;
                    canvas[i][0] *= k;
                    canvas[i][1] *= k;
                    canvas[i][2] *= k;
                }
            }
        }

        // paint the sheet
        for y in 0..hu {
            for x in 0..wu {
                let i = y * wu + x;
                let c = sheet.cov[i];
                if c <= 0.001 {
                    continue;
                }
                let mut rgb = col;
                if layer > 0 {
                    // lit top-left edge
                    let (px, py) = (x as i32 - hl_off, y as i32 - hl_off);
                    let behind = if px >= 0 && py >= 0 { sheet.cov[py as usize * wu + px as usize] } else { 0.0 };
                    let hl = c * (1.0 - behind) * 0.35;
                    for k in 0..3 {
                        rgb[k] += (1.0 - rgb[k]) * hl;
                    }
                    // frayed white fibres along the cut edge
                    if !sheet.band.is_empty() {
                        let f = hash2(x as i32, y as i32, 99);
                        let k = (sheet.band[i] * f * p.weather * 1.3).clamp(0.0, 1.0);
                        for j in 0..3 {
                            rgb[j] += (paper_white[j] - rgb[j]) * k;
                        }
                    }
                }
                let d = &mut canvas[i];
                let inv = 1.0 - c;
                d[0] = rgb[0] * c + d[0] * inv;
                d[1] = rgb[1] * c + d[1] * inv;
                d[2] = rgb[2] * c + d[2] * inv;
                d[3] = c + d[3] * inv;
            }
        }
    }

    // --- 5. paper texture + weathering
    if p.texture && (p.grain > 0.0 || p.weather > 0.0) {
        let w_amt = p.weather;
        for y in 0..hu {
            for x in 0..wu {
                let i = y * wu + x;
                if canvas[i][3] <= 0.001 {
                    continue;
                }
                let (xr, yr) = (x as f32 / sc, y as f32 / sc);
                let mut m = 1.0f32;
                if p.grain > 0.0 {
                    let fine = hash2(x as i32, y as i32, 5) - 0.5;
                    let fibers = (vnoise(xr * 0.7, yr * 0.09, 11) + vnoise(xr * 0.09, yr * 0.7, 13)) * 0.5 - 0.5;
                    let cloud = fbm(xr / 60.0, yr / 60.0, 3, 17) - 0.5;
                    m += p.grain * (0.16 * fine + 0.30 * fibers + 0.22 * cloud);
                }
                let mut tint = [1.0f32; 3];
                if w_amt > 0.0 {
                    // stains
                    let s = smoothstep(0.45, 0.85, fbm(xr / 160.0, yr / 160.0, 4, 23)) * w_amt * 0.6;
                    let sepia = [0.80f32, 0.64, 0.42];
                    for k in 0..3 {
                        tint[k] = 1.0 + (sepia[k] - 1.0) * s;
                    }
                    // overall yellowing
                    let y_t = [1.0f32, 0.965, 0.89];
                    for k in 0..3 {
                        tint[k] *= 1.0 + (y_t[k] - 1.0) * w_amt * 0.7;
                    }
                }
                for k in 0..3 {
                    canvas[i][k] *= m * tint[k];
                }
            }
        }
        // brown age spots ("foxing")
        if w_amt > 0.0 {
            let count = (((wu * hu) as f32 / 30_000.0) * w_amt * 1.5).min(300.0) as u32;
            for s in 0..count {
                let cx = (hash2(s as i32, 1, 31) * w as f32) as i32;
                let cy = (hash2(s as i32, 2, 31) * h as f32) as i32;
                let r = ((2.0 + hash2(s as i32, 3, 31) * 8.0) * sc).max(1.5);
                let strength = 0.15 + hash2(s as i32, 4, 31) * 0.3 * w_amt;
                let ri = r.ceil() as i32 * 2;
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        let (x, y) = (cx + dx, cy + dy);
                        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                            continue;
                        }
                        let d2 = (dx * dx + dy * dy) as f32 / (r * r);
                        let a = (-d2 * 1.4).exp() * strength;
                        if a < 0.01 {
                            continue;
                        }
                        let c = &mut canvas[y as usize * wu + x as usize];
                        if c[3] <= 0.001 {
                            continue;
                        }
                        c[0] *= 1.0 - a * 0.45;
                        c[1] *= 1.0 - a * 0.65;
                        c[2] *= 1.0 - a * 0.85;
                    }
                }
            }
        }
    }

    // --- 6. un-premultiply and save
    let mut result = RgbaImage::new(w, h);
    for (d, c) in result.pixels_mut().zip(canvas.iter()) {
        let a = c[3].clamp(0.0, 1.0);
        let un = |v: f32| if a > 0.001 { ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8 } else { 0 };
        d.0 = [un(c[0]), un(c[1]), un(c[2]), (a * 255.0).round() as u8];
    }
    let is_jpg = out
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .map(|e| e == "jpg" || e == "jpeg")
        .unwrap_or(false);
    if is_jpg {
        // JPEG has no transparency: flatten onto paper white.
        let mut flat = image::RgbImage::new(w, h);
        for (d, s) in flat.pixels_mut().zip(result.pixels()) {
            let a = s[3] as f32 / 255.0;
            let f = |c: u8, bg: f32| (c as f32 * a + bg * 255.0 * (1.0 - a)).round() as u8;
            d.0 = [f(s[0], paper_white[0]), f(s[1], paper_white[1]), f(s[2], paper_white[2])];
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
            layers: 4,
            smooth: 2.0,
            rough: 0.3,
            shadow: 0.6,
            thick: 4.0,
            gray: false,
            texture: false,
            grain: 0.0,
            weather: 0.0,
        }
    }

    /// Left-to-right gradient on an opaque canvas.
    fn gradient(path: &Path) {
        let mut img = RgbaImage::new(200, 120);
        for (x, _, px) in img.enumerate_pixels_mut() {
            let v = (x as f32 / 199.0 * 255.0) as u8;
            px.0 = [v, 255 - v, 128, 255];
        }
        img.save(path).unwrap();
    }

    /// Share of all pixels covered by the `k` most common (coarsely quantised) colours.
    fn top_coverage(img: &RgbaImage, k: usize) -> f32 {
        let mut hist = std::collections::HashMap::new();
        for px in img.pixels() {
            *hist.entry((px[0] / 8, px[1] / 8, px[2] / 8)).or_insert(0u32) += 1;
        }
        let mut counts: Vec<u32> = hist.values().copied().collect();
        counts.sort_unstable_by(|a, b| b.cmp(a));
        counts.iter().take(k).sum::<u32>() as f32 / (img.width() * img.height()) as f32
    }

    #[test]
    fn paperize_makes_flat_layers_with_shadows_and_keeps_size() {
        let dir = std::env::temp_dir();
        let (inp, out) = (dir.join("pz_in.png"), dir.join("pz_out.png"));
        gradient(&inp);
        render_paperize(&inp, &out, &params(), 1.0).unwrap();
        let res = image::open(&out).unwrap().to_rgba8();
        assert_eq!(res.dimensions(), (200, 120));
        // flat sheets: a handful of colours must cover most of the picture (a smooth gradient never does)
        let src_cov = top_coverage(&image::open(&inp).unwrap().to_rgba8(), 8);
        let res_cov = top_coverage(&res, 8);
        assert!(res_cov > 0.5 && res_cov > src_cov * 2.0, "flat sheets expected: {res_cov} vs source {src_cov}");
        // still fully opaque
        assert!(res.pixels().all(|p| p[3] == 255));
    }

    #[test]
    fn paperize_keeps_transparency_and_texture_changes_pixels() {
        let dir = std::env::temp_dir();
        let (inp, a, b) = (dir.join("pz_t_in.png"), dir.join("pz_t_a.png"), dir.join("pz_t_b.png"));
        let mut img = RgbaImage::new(160, 160);
        for (x, y, px) in img.enumerate_pixels_mut() {
            let inside = (x as i32 - 80).pow(2) + (y as i32 - 80).pow(2) < 50 * 50;
            px.0 = if inside { [200, 60, 60, 255] } else { [0, 0, 0, 0] };
        }
        img.save(&inp).unwrap();

        let mut p = params();
        render_paperize(&inp, &a, &p, 1.0).unwrap();
        let plain = image::open(&a).unwrap().to_rgba8();
        assert_eq!(plain.get_pixel(2, 2)[3], 0, "outside the subject stays transparent");
        assert_eq!(plain.get_pixel(80, 80)[3], 255);

        p.texture = true;
        p.grain = 0.8;
        p.weather = 0.8;
        render_paperize(&inp, &b, &p, 1.0).unwrap();
        let tex = image::open(&b).unwrap().to_rgba8();
        let differing = plain.pixels().zip(tex.pixels()).filter(|(x, y)| x != y).count();
        assert!(differing > 2000, "texture/weathering should visibly change the image ({differing})");
    }
}

#[cfg(test)]
mod perf {
    use super::*;

    /// Manual timing check: `cargo test --release paperize_timing -- --ignored --nocapture`
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
            layers: 5, smooth: 4.0, rough: 0.35, shadow: 0.55, thick: 5.0,
            gray: false, texture: true, grain: 0.4, weather: 0.25,
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
