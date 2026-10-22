//! Exporting the current image: format, quality and optional resize.
//! (This is the "backend" of Save As; the old Convert tool is folded into it.)

use image::{DynamicImage, ImageEncoder};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    Webp,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
        }
    }
    pub fn from_ext(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Format::Png),
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "webp" => Some(Format::Webp),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExportOptions {
    pub format: Format,
    /// 1..=100 (JPEG / WebP).
    pub quality: u8,
    /// Exact output size (width, height); `None` keeps the image size.
    pub resize: Option<(u32, u32)>,
}

/// Flatten transparency onto white (JPEG has no alpha channel).
fn flatten_on_white(img: &DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());
    for (d, s) in out.pixels_mut().zip(rgba.pixels()) {
        let a = s[3] as u32;
        let f = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        d.0 = [f(s[0]), f(s[1]), f(s[2])];
    }
    out
}

pub fn export_image(src: &Path, dst: &Path, o: &ExportOptions) -> Result<(), String> {
    let mut img = image::open(src).map_err(|e| format!("can't read the image: {e}"))?;
    if let Some((w, h)) = o.resize {
        if w > 0 && h > 0 && (w, h) != (img.width(), img.height()) {
            img = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
        }
    }
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match o.format {
        Format::Png => img
            .to_rgba8()
            .save_with_format(dst, image::ImageFormat::Png)
            .map_err(|e| e.to_string()),
        Format::Jpeg => {
            let rgb = flatten_on_white(&img);
            let file = std::fs::File::create(dst).map_err(|e| e.to_string())?;
            image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), o.quality.clamp(1, 100))
                .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())
        }
        Format::Webp => {
            // The `image` crate only writes lossless WebP; use ImageMagick for lossy quality when available.
            let tmp = crate::config::tmp_file(".png");
            img.to_rgba8().save(&tmp).map_err(|e| e.to_string())?;
            let ok = std::process::Command::new("magick")
                .arg(&tmp)
                .args(["-quality", &o.quality.clamp(1, 100).to_string()])
                .arg(dst)
                .output()
                .map(|r| r.status.success())
                .unwrap_or(false);
            let _ = std::fs::remove_file(&tmp);
            if ok {
                Ok(())
            } else {
                img.to_rgba8().save_with_format(dst, image::ImageFormat::WebP).map_err(|e| e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(path: &Path) {
        let mut img = image::RgbaImage::new(120, 80);
        for (x, y, px) in img.enumerate_pixels_mut() {
            px.0 = if x < 60 { [(x * 4) as u8, (y * 3) as u8, 120, 255] } else { [10, 10, 10, 0] };
        }
        img.save(path).unwrap();
    }

    #[test]
    fn exports_png_and_jpeg_with_resize_and_quality() {
        let dir = std::env::temp_dir().join("export_test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.png");
        sample(&src);

        // PNG keeps transparency and can be resized
        let out = dir.join("o.png");
        export_image(&src, &out, &ExportOptions { format: Format::Png, quality: 100, resize: Some((60, 40)) }).unwrap();
        let img = image::open(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (60, 40));
        assert_eq!(img.get_pixel(55, 5)[3], 0, "transparent area stays transparent in PNG");

        // JPEG: smaller file at lower quality, transparency flattened to white
        let (hi, lo) = (dir.join("hi.jpg"), dir.join("lo.jpg"));
        export_image(&src, &hi, &ExportOptions { format: Format::Jpeg, quality: 100, resize: None }).unwrap();
        export_image(&src, &lo, &ExportOptions { format: Format::Jpeg, quality: 30, resize: None }).unwrap();
        assert!(std::fs::metadata(&lo).unwrap().len() < std::fs::metadata(&hi).unwrap().len());
        let j = image::open(&hi).unwrap().to_rgb8();
        let p = j.get_pixel(100, 40);
        assert!(p[0] > 240 && p[1] > 240 && p[2] > 240, "transparent -> white in JPEG: {p:?}");
    }

    #[test]
    fn format_from_extension() {
        assert_eq!(Format::from_ext("JPG"), Some(Format::Jpeg));
        assert_eq!(Format::from_ext("webp"), Some(Format::Webp));
        assert_eq!(Format::from_ext("gif"), None);
    }
}
