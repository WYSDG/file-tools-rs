//! Bundled tool icons (Phosphor Icons, MIT — see assets/icons/LICENSE-phosphor-icons.txt).
//!
//! The SVGs are compiled into the binary and written to a per-user cache folder at startup, which is
//! added to GTK's icon search path. They are named `*-symbolic`, so GTK recolours them to the
//! current text colour — they follow every theme automatically and don't depend on the system icon theme.

use gtk4 as gtk;
use std::path::PathBuf;

macro_rules! icon {
    ($n:literal) => {
        (
            concat!($n, "-symbolic"),
            include_str!(concat!("../assets/icons/", $n, "-symbolic.svg")),
        )
    };
}

const ICONS: &[(&str, &str)] = &[
    icon!("ft-magic-wand"),
    icon!("ft-sliders"),
    icon!("ft-crop"),
    icon!("ft-bucket"),
    icon!("ft-palette"),
    icon!("ft-duotone"),
    icon!("ft-halftone"),
    icon!("ft-ascii"),
    icon!("ft-pixelate"),
    icon!("ft-outline"),
    icon!("ft-stamp"),
    icon!("ft-convert"),
    icon!("ft-paperize"),
    icon!("ft-open"),
    icon!("ft-paste"),
    icon!("ft-save"),
    icon!("ft-copy"),
    icon!("ft-undo"),
    icon!("ft-redo"),
    icon!("ft-reset"),
    icon!("ft-settings"),
    icon!("ft-tools"),
    icon!("ft-resize"),
    icon!("ft-recent"),
];

fn cache_root() -> PathBuf {
    crate::config::home().join(".cache").join("file-tools").join("icons")
}

/// Write the icons to disk (only when missing/changed) and register the folder with GTK.
pub fn install() {
    let root = cache_root();
    let dir = root.join("hicolor").join("scalable").join("actions");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, svg) in ICONS {
        let path = dir.join(format!("{name}.svg"));
        if std::fs::read_to_string(&path).map(|s| s != *svg).unwrap_or(true) {
            let _ = std::fs::write(&path, svg);
        }
    }
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&root);
    }
}

/// Icon name for a tab (as named in `RAIL_LAYOUT`), if we have one.
pub fn tab_icon(tab: &str) -> Option<&'static str> {
    let name = match tab {
        "Remove" => "ft-magic-wand-symbolic",
        "Adjust" => "ft-sliders-symbolic",
        "Crop" => "ft-crop-symbolic",
        "Fill Background" => "ft-bucket-symbolic",
        "Palette" => "ft-palette-symbolic",
        "Duotone" => "ft-duotone-symbolic",
        "Halftone" => "ft-halftone-symbolic",
        "ASCII" => "ft-ascii-symbolic",
        "Pixelate" => "ft-pixelate-symbolic",
        "Outline" => "ft-outline-symbolic",
        "Watermark" => "ft-stamp-symbolic",
        "Convert" => "ft-convert-symbolic",
        "Paperize" => "ft-paperize-symbolic",
        _ => return None,
    };
    // SVG icons need gdk-pixbuf's SVG loader (package `librsvg`). Without it GTK would draw a
    // "missing image" glyph, so fall back to text-only buttons instead.
    if !svg_supported() {
        return None;
    }
    // Only offer icons GTK can actually find.
    let found = gtk::gdk::Display::default()
        .map(|d| gtk::IconTheme::for_display(&d).has_icon(name))
        .unwrap_or(false);
    found.then_some(name)
}

fn svg_supported() -> bool {
    use std::sync::OnceLock;
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        gtk::gdk_pixbuf::Pixbuf::formats()
            .iter()
            .any(|f| f.name().map(|n| n.eq_ignore_ascii_case("svg")).unwrap_or(false))
    })
}

/// Icon for a toolbar/menu button (`key` = open, paste, save, copy, undo, redo, reset, settings, tools).
pub fn ui_icon(key: &str) -> Option<&'static str> {
    let name = match key {
        "open" => "ft-open-symbolic",
        "paste" => "ft-paste-symbolic",
        "save" => "ft-save-symbolic",
        "copy" => "ft-copy-symbolic",
        "undo" => "ft-undo-symbolic",
        "redo" => "ft-redo-symbolic",
        "reset" => "ft-reset-symbolic",
        "settings" => "ft-settings-symbolic",
        "tools" => "ft-tools-symbolic",
        "resize" => "ft-resize-symbolic",
        "recent" => "ft-recent-symbolic",
        _ => return None,
    };
    if !svg_supported() {
        return None;
    }
    let found = gtk::gdk::Display::default()
        .map(|d| gtk::IconTheme::for_display(&d).has_icon(name))
        .unwrap_or(false);
    found.then_some(name)
}
