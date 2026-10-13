use gtk::prelude::*;
use gtk4 as gtk;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

pub fn run_capture(cmd: &[&str]) -> Result<String> {
    let out = Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| anyhow!("spawn failed: {}", e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("{}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn run_quiet(cmd: &[&str]) -> Result<()> {
    run_capture(cmd).map(|_| ())
}

pub fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.is_file())
    })
}

pub fn image_dimensions(path: &Path) -> Result<(u32, u32)> {
    // Fast path: read just the file header in-process (no subprocess).
    if let Ok(d) = image::image_dimensions(path) {
        return Ok(d);
    }
    // Fallback for formats the `image` crate can't read.
    let arg = format!("{}[0]", path.display());
    let out = run_capture(&["magick", "identify", "-format", "%wx%h", &arg])?;
    let trimmed = out.trim();
    let (w, h) = trimmed
        .split_once('x')
        .ok_or_else(|| anyhow!("bad identify output: {}", trimmed))?;
    Ok((w.parse()?, h.parse()?))
}

pub fn escape_annotate(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn resolve_font(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    let p = PathBuf::from(name);
    if p.exists() {
        return Some(name.to_string());
    }
    let out = Command::new("fc-match")
        .args(["-f", "%{file}", name])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !s.is_empty() && PathBuf::from(&s).exists() {
        Some(s)
    } else {
        None
    }
}

use std::io::Write as _;

pub fn copy_text_to_clipboard(text: &str) -> bool {
    if which("wl-copy").is_none() {
        return false;
    }
    Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn copy_image_to_clipboard(path: &Path) -> bool {
    if which("wl-copy").is_none() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut child) = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::from(file))
        .spawn()
    else {
        return false;
    };
    child.wait().map(|s| s.success()).unwrap_or(false)
}

pub fn hex_to_rgb(h: &str) -> Option<(u8, u8, u8)> {
    let h = h.trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

pub fn mix_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let f = |x: u8, y: u8| ((x as f64) + (y as f64 - x as f64) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 {
        d / (2.0 - mx - mn)
    } else {
        d / (mx + mn)
    };
    let h = if mx == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if mx == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s < 1e-9 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h2r = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        (h2r(h + 1.0 / 3.0) * 255.0).round() as u8,
        (h2r(h) * 255.0).round() as u8,
        (h2r(h - 1.0 / 3.0) * 255.0).round() as u8,
    )
}

pub fn tints(hex: &str, steps: usize) -> Vec<String> {
    let Some(rgb) = hex_to_rgb(hex) else {
        return vec![];
    };
    (1..=steps)
        .map(|i| {
            let t = i as f64 / (steps as f64 + 1.0);
            let c = mix_rgb(rgb, (255, 255, 255), t);
            rgb_to_hex(c.0, c.1, c.2)
        })
        .collect()
}

pub fn shades(hex: &str, steps: usize) -> Vec<String> {
    let Some(rgb) = hex_to_rgb(hex) else {
        return vec![];
    };
    (1..=steps)
        .map(|i| {
            let t = i as f64 / (steps as f64 + 1.0);
            let c = mix_rgb(rgb, (0, 0, 0), t);
            rgb_to_hex(c.0, c.1, c.2)
        })
        .collect()
}

pub fn midtones(hex: &str, steps: usize) -> Vec<String> {
    let Some(rgb) = hex_to_rgb(hex) else {
        return vec![];
    };
    let (h, s, _) = rgb_to_hsl(rgb.0, rgb.1, rgb.2);
    let lo = 0.30;
    let hi = 0.72;
    (0..steps)
        .map(|i| {
            let t = if steps > 1 {
                i as f64 / (steps as f64 - 1.0)
            } else {
                0.5
            };
            let l = lo + (hi - lo) * t;
            let (r, g, b) = hsl_to_rgb(h, s, l);
            rgb_to_hex(r, g, b)
        })
        .collect()
}

pub fn extract_palette(path: &Path, n: i32) -> Result<Vec<String>, String> {
    let output = Command::new("magick")
        .args([
            path.to_string_lossy().as_ref(),
            "-resize",
            "200x200>",
            "-colors",
            &n.to_string(),
            "-unique-colors",
            "-depth",
            "8",
            "txt:-",
        ])
        .output()
        .map_err(|e| format!("spawn error: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen = std::collections::HashSet::new();
    let mut colors = Vec::new();
    for line in text.lines() {
        if let Some(hash_pos) = line.find('#') {
            let after = &line[hash_pos + 1..];
            if after.len() >= 6 && after[..6].chars().all(|c| c.is_ascii_hexdigit()) {
                let full = format!("#{}", after[..6].to_uppercase());
                if seen.insert(full.clone()) {
                    colors.push(full);
                }
            }
        }
    }
    Ok(colors)
}

/// Generate a 3-note chime WAV once and cache it.
/// Returns the path to the chime, or None if generation fails.
/// Generate a two-note elevator chime (E5 → C5) and cache it.
fn ensure_chime() -> Option<PathBuf> {
    let cached = crate::config::config_dir().join("chime-v2.wav");
    if cached.exists() {
        return Some(cached);
    }

    if which("sox").is_none() {
        // Fallback to a soft system sound
        for f in [
            "/usr/share/sounds/gnome/default/alerts/glass.ogg",
            "/usr/share/sounds/gnome/default/alerts/sonar.ogg",
            "/usr/share/sounds/freedesktop/stereo/bell.oga",
        ] {
            let p = PathBuf::from(f);
            if p.exists() {
                return Some(p);
            }
        }
        return None;
    }

    let tmp_a = crate::config::config_dir().join(".chime-a.wav");
    let tmp_b = crate::config::config_dir().join(".chime-b.wav");

    // Note 1: E5, bell-like
    let ok_a = Command::new("sox")
        .args([
            "-n",
            "-r",
            "44100",
            "-c",
            "1",
            tmp_a.to_string_lossy().as_ref(),
            "synth",
            "0.55",
            "sine",
            "659",
            "vol",
            "0.35",
            "fade",
            "0.015",
            "0.55",
            "0.35",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Note 2: C5, slightly longer with a softer tail
    let ok_b = Command::new("sox")
        .args([
            "-n",
            "-r",
            "44100",
            "-c",
            "1",
            tmp_b.to_string_lossy().as_ref(),
            "synth",
            "0.95",
            "sine",
            "523",
            "vol",
            "0.35",
            "fade",
            "0.015",
            "0.95",
            "0.55",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !(ok_a && ok_b) {
        let _ = std::fs::remove_file(&tmp_a);
        let _ = std::fs::remove_file(&tmp_b);
        return None;
    }

    // Concatenate with a small gap
    let ok_concat = Command::new("sox")
        .args([
            tmp_a.to_string_lossy().as_ref(),
            tmp_b.to_string_lossy().as_ref(),
            cached.to_string_lossy().as_ref(),
            "pad",
            "0",
            "0.08",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let _ = std::fs::remove_file(&tmp_a);
    let _ = std::fs::remove_file(&tmp_b);

    if ok_concat && cached.exists() {
        Some(cached)
    } else {
        None
    }
}

/// Play the chime 3 times with small gaps.
pub fn play_done_sound(volume: f64) {
    let vol = volume.clamp(0.0, 1.0);

    let Some(chime) = ensure_chime() else {
        return;
    };

    // Pick a player
    let player: Option<(&str, u32)> = if which("paplay").is_some() {
        Some(("paplay", (vol * 65536.0).round() as u32))
    } else if which("pw-play").is_some() {
        Some(("pw-play", (vol * 100.0).round() as u32))
    } else if which("ffplay").is_some() {
        Some(("ffplay", (vol * 100.0).round() as u32))
    } else if which("aplay").is_some() {
        Some(("aplay", 0))
    } else {
        None
    };

    let Some((player, v)) = player else {
        return;
    };
    let player = player.to_string();
    let chime_str = chime.to_string_lossy().to_string();

    // Play 3 times with a small gap. Runs in a detached thread.
    std::thread::spawn(move || {
        for _ in 0..3 {
            play_once(&player, &chime_str, v);
            std::thread::sleep(std::time::Duration::from_millis(320));
        }
    });
}

fn play_once(player: &str, sound: &str, v: u32) {
    let mut cmd = Command::new(player);
    match player {
        "paplay" => {
            cmd.arg(format!("--volume={}", v));
        }
        "pw-play" => {
            cmd.arg(format!("--volume={:.2}", v as f64 / 100.0));
        }
        "ffplay" => {
            cmd.args(["-nodisp", "-autoexit", "-loglevel", "quiet"]);
            cmd.arg("-volume");
            cmd.arg(v.to_string());
        }
        _ => {}
    }
    cmd.arg(sound);
    // Detach stdin/stdout/stderr so the process doesn't tie up the parent
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let _ = cmd.spawn();
}

pub fn hex_to_gdk_rgba(hex: &str) -> Option<gtk::gdk::RGBA> {
    let (r, g, b) = hex_to_rgb(hex)?;
    Some(gtk::gdk::RGBA::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ))
}

/// Build a small 🔍 button that, when clicked, activates preview eyedropper
/// mode. The next click on the preview will set `target` to the picked color.
pub fn make_eyedropper_button(
    target: gtk::ColorDialogButton,
    app: crate::app::AppState,
) -> gtk::Button {
    let btn = gtk::Button::with_label("🔍");
    btn.set_tooltip_text(Some("Pick color from the preview"));
    btn.add_css_class("flat");
    btn.set_valign(gtk::Align::Center);
    let target_c = target.clone();
    btn.connect_clicked(move |_| {
        let t = target_c.clone();
        app.start_eyedropper(std::rc::Rc::new(move |hex: String| {
            if let Some(rgba) = hex_to_gdk_rgba(&hex) {
                t.set_rgba(&rgba);
            }
        }));
    });
    btn
}

// ---------------------------------------------------------------------------
// Preview proxy: a small copy of a big image, so live previews stay fast.
// ---------------------------------------------------------------------------

/// Longest side of the proxy image.
pub const PROXY_MAX: u32 = 1280;
/// Only images whose longest side exceeds this get a proxy (smaller ones are fast enough already).
pub const PROXY_TRIGGER: u32 = 1800;

thread_local! {
    static PROXY_CACHE: std::cell::RefCell<std::collections::HashMap<String, PathBuf>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// For a big image returns `(proxy_path, scale)` where `scale` = proxy size / original size
/// (use it to scale pixel-valued parameters such as blur radius). `None` for small images.
/// Cached per file (path + size + mtime), so only the first call pays for the downscale.
pub fn preview_proxy(path: &Path) -> Option<(PathBuf, f64)> {
    use image::ImageEncoder;
    let (w, h) = image::image_dimensions(path).ok()?;
    if w.max(h) <= PROXY_TRIGGER {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    let key = format!("{}|{}|{:?}", path.display(), meta.len(), meta.modified().ok());
    let scale = PROXY_MAX as f64 / w.max(h) as f64;
    if let Some(p) = PROXY_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        if p.exists() {
            return Some((p, scale));
        }
    }
    let img = image::open(path).ok()?;
    let small = img.resize(PROXY_MAX, PROXY_MAX, image::imageops::FilterType::Triangle);
    let rgba = small.to_rgba8();
    let out = crate::config::tmp_file(".png");
    let file = std::fs::File::create(&out).ok()?;
    // Fast PNG settings: this is a throwaway preview file.
    image::codecs::png::PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(rgba.as_raw(), rgba.width(), rgba.height(), image::ExtendedColorType::Rgba8)
    .ok()?;
    let real_scale = rgba.width().max(rgba.height()) as f64 / w.max(h) as f64;
    PROXY_CACHE.with(|c| c.borrow_mut().insert(key, out.clone()));
    Some((out, real_scale))
}

#[cfg(test)]
mod proxy_tests {
    use super::*;

    #[test]
    fn proxy_only_for_big_images_and_keeps_aspect() {
        let dir = std::env::temp_dir();
        let small = dir.join("px_small.png");
        image::RgbaImage::from_pixel(1000, 800, image::Rgba([10, 20, 30, 255])).save(&small).unwrap();
        assert!(preview_proxy(&small).is_none(), "small images must not get a proxy");

        let big = dir.join("px_big.png");
        image::RgbaImage::from_pixel(3000, 2000, image::Rgba([10, 20, 30, 255])).save(&big).unwrap();
        let (p, scale) = preview_proxy(&big).expect("big image gets a proxy");
        let (w, h) = image::image_dimensions(&p).unwrap();
        assert_eq!(w, 1280);
        assert!((h as f64 - 853.0).abs() <= 1.0, "aspect kept, got {w}x{h}");
        assert!((scale - 1280.0 / 3000.0).abs() < 1e-6);
        // second call is served from the cache (same file)
        assert_eq!(preview_proxy(&big).unwrap().0, p);
    }
}
