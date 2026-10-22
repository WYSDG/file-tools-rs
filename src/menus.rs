//! Menu bar (File / Edit / View / Tools / Help) and the window actions behind it.
//!
//! Actions live on the window (`win.*`). Undo/Redo/Copy/Paste shortcuts are handled by the window's
//! key controller in `app.rs`, so no accelerators are registered for those here.

use gtk::gio;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::app::{AppState, RAIL_LAYOUT};
use crate::config;

const IMAGE_EXTS: [&str; 8] = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff"];

fn add_action(window: &gtk::ApplicationWindow, name: &str, f: impl Fn() + 'static) {
    let a = gio::SimpleAction::new(name, None);
    a.connect_activate(move |_, _| f());
    window.add_action(&a);
}

fn string_action(window: &gtk::ApplicationWindow, name: &str, f: impl Fn(String) + 'static) {
    let a = gio::SimpleAction::new(name, Some(&String::static_variant_type()));
    a.connect_activate(move |_, p| {
        if let Some(s) = p.and_then(|v| v.get::<String>()) {
            f(s);
        }
    });
    window.add_action(&a);
}

/// Rebuild the "Open Recent" submenu from the most recently saved images.
pub fn fill_recent(menu: &gio::Menu) {
    menu.remove_all();
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(config::save_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_string_lossy().to_lowercase();
            if !IMAGE_EXTS.contains(&ext.as_str()) {
                return None;
            }
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, p))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    if files.is_empty() {
        menu.append(Some("No recent files"), None);
        return;
    }
    for (_, p) in files.into_iter().take(12) {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let item = gio::MenuItem::new(Some(&name), None);
        item.set_action_and_target_value(Some("win.open_recent"), Some(&p.to_string_lossy().to_string().to_variant()));
        menu.append_item(&item);
    }
}

fn info_dialog(window: &gtk::ApplicationWindow, title: &str, text: &str) {
    gtk::AlertDialog::builder().message(title).detail(text).build().show(Some(window));
}

/// Registers all window actions and returns the menu bar widget.
pub fn install(
    window: &gtk::ApplicationWindow,
    app: &AppState,
    rail_buttons: Rc<RefCell<HashMap<String, gtk::ToggleButton>>>,
    gear: &gtk::MenuButton,
    open_cb: Rc<dyn Fn()>,
) -> gtk::PopoverMenuBar {
    // ---- actions ----
    add_action(window, "open", move || open_cb());
    {
        let a = app.clone();
        add_action(window, "paste", move || a.paste_from_clipboard());
    }
    {
        let a = app.clone();
        add_action(window, "save_as", move || a.save_as());
    }
    {
        let a = app.clone();
        add_action(window, "copy", move || a.copy_result_image());
    }
    {
        let a = app.clone();
        add_action(window, "reset", move || a.reset_to_original());
    }
    {
        let a = app.clone();
        add_action(window, "undo", move || a.undo());
    }
    {
        let a = app.clone();
        add_action(window, "redo", move || a.redo());
    }
    {
        let w = window.clone();
        add_action(window, "quit", move || w.close());
    }
    for (name, which) in [("zoom_in", 0), ("zoom_out", 1), ("zoom_fit", 2), ("zoom_actual", 3)] {
        let a = app.clone();
        add_action(window, name, move || {
            let p = &a.preview;
            match which {
                0 => p.zoom_in_btn.emit_clicked(),
                1 => p.zoom_out_btn.emit_clicked(),
                2 => p.fit_btn.emit_clicked(),
                _ => p.one_btn.emit_clicked(),
            }
        });
    }
    {
        let g = gear.clone();
        add_action(window, "settings", move || g.popup());
    }
    {
        let w = window.clone();
        add_action(window, "shortcuts", move || {
            info_dialog(
                &w,
                "Keyboard shortcuts",
                "Ctrl+O  Open image\nCtrl+V  Paste image\nCtrl+S  Save as\nCtrl+C  Copy result\n\
                 Ctrl+Z  Undo\nCtrl+Shift+Z / Ctrl+Y  Redo\nCtrl+Enter  Process the current tool\n\
                 Ctrl + / -  Zoom in / out\nCtrl+0  Fit to window\nCtrl+1  Actual size (100%)\n\
                 Esc  Cancel the eyedropper\nCtrl+Q  Quit",
            );
        });
    }
    {
        let w = window.clone();
        add_action(window, "about", move || {
            info_dialog(
                &w,
                &format!("File Tools {}", env!("CARGO_PKG_VERSION")),
                "A personal image toolkit: background removal, adjustments, halftone, ASCII, paper effects and more.\n\n\
                 Tool icons: Phosphor Icons (MIT).",
            );
        });
    }
    string_action(window, "tool", move |name| {
        if let Some(b) = rail_buttons.borrow().get(&name) {
            b.set_active(true);
        }
    });
    {
        let a = app.clone();
        string_action(window, "open_recent", move |path| {
            let p = PathBuf::from(path);
            if p.exists() {
                a.open_path(p);
            }
        });
    }

    // ---- menu model ----
    let recent = gio::Menu::new();
    fill_recent(&recent);
    *app.recent_menu.borrow_mut() = Some(recent.clone());

    let file = gio::Menu::new();
    let s1 = gio::Menu::new();
    s1.append(Some("Open Image…"), Some("win.open"));
    s1.append_submenu(Some("Open Recent"), &recent);
    s1.append(Some("Paste Image"), Some("win.paste"));
    file.append_section(None, &s1);
    let s2 = gio::Menu::new();
    s2.append(Some("Save As…"), Some("win.save_as"));
    s2.append(Some("Copy Image"), Some("win.copy"));
    file.append_section(None, &s2);
    let s3 = gio::Menu::new();
    s3.append(Some("Reset to Original"), Some("win.reset"));
    s3.append(Some("Quit"), Some("win.quit"));
    file.append_section(None, &s3);

    let edit = gio::Menu::new();
    edit.append(Some("Undo"), Some("win.undo"));
    edit.append(Some("Redo"), Some("win.redo"));
    let e2 = gio::Menu::new();
    e2.append(Some("Reset to Original"), Some("win.reset"));
    edit.append_section(None, &e2);

    let view = gio::Menu::new();
    let v1 = gio::Menu::new();
    v1.append(Some("Zoom In"), Some("win.zoom_in"));
    v1.append(Some("Zoom Out"), Some("win.zoom_out"));
    v1.append(Some("Fit to Window"), Some("win.zoom_fit"));
    v1.append(Some("Actual Size (100%)"), Some("win.zoom_actual"));
    view.append_section(None, &v1);
    let v2 = gio::Menu::new();
    v2.append(Some("Settings…"), Some("win.settings"));
    view.append_section(None, &v2);

    // Tools: one section per rail group, in rail order.
    let tools = gio::Menu::new();
    let mut groups: Vec<(&str, gio::Menu)> = Vec::new();
    for (name, _short, tip, group) in RAIL_LAYOUT {
        let idx = match groups.iter().position(|(g, _)| g == group) {
            Some(i) => i,
            None => {
                groups.push((group, gio::Menu::new()));
                groups.len() - 1
            }
        };
        let item = gio::MenuItem::new(Some(tip), None);
        item.set_action_and_target_value(Some("win.tool"), Some(&name.to_string().to_variant()));
        groups[idx].1.append_item(&item);
    }
    for (group, menu) in &groups {
        tools.append_section(Some(group), menu);
    }

    let help = gio::Menu::new();
    help.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
    help.append(Some("About File Tools"), Some("win.about"));

    let root = gio::Menu::new();
    root.append_submenu(Some("_File"), &file);
    root.append_submenu(Some("_Edit"), &edit);
    root.append_submenu(Some("_View"), &view);
    root.append_submenu(Some("_Tools"), &tools);
    root.append_submenu(Some("_Help"), &help);

    // ---- accelerators (display + activation; undo/redo/copy/paste are in the key controller) ----
    if let Some(a) = window.application() {
        a.set_accels_for_action("win.open", &["<Control>o"]);
        a.set_accels_for_action("win.save_as", &["<Control>s"]);
        a.set_accels_for_action("win.quit", &["<Control>q"]);
        a.set_accels_for_action("win.zoom_in", &["<Control>plus", "<Control>equal"]);
        a.set_accels_for_action("win.zoom_out", &["<Control>minus"]);
        a.set_accels_for_action("win.zoom_fit", &["<Control>0"]);
        a.set_accels_for_action("win.zoom_actual", &["<Control>1"]);
    }

    let bar = gtk::PopoverMenuBar::from_model(Some(&root));
    bar.add_css_class("menu-bar");
    bar
}

/// Popover list of recently saved images (names are ellipsized, so long names can't blow up the width).
pub fn build_recent_popover(app: &AppState) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_activate_on_single_click(true);
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_width(330)
        .max_content_width(330)
        .min_content_height(60)
        .max_content_height(360)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    scroll.set_child(Some(&list));
    popover.set_child(Some(&scroll));

    let paths: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    // Rebuilt every time it is shown, so new saves always appear.
    {
        let (list, paths) = (list.clone(), paths.clone());
        popover.connect_map(move |_| {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(config::save_dir())
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    let ext = p.extension()?.to_string_lossy().to_lowercase();
                    if !e.file_type().ok()?.is_file() || !IMAGE_EXTS.contains(&ext.as_str()) {
                        return None;
                    }
                    Some((e.metadata().ok()?.modified().ok()?, p))
                })
                .collect();
            files.sort_by(|a, b| b.0.cmp(&a.0));
            let mut stored = paths.borrow_mut();
            stored.clear();
            if files.is_empty() {
                let l = gtk::Label::new(Some("No saved images yet"));
                l.set_margin_top(10);
                l.set_margin_bottom(10);
                l.set_margin_start(14);
                l.set_margin_end(14);
                list.append(&l);
            }
            for (_, p) in files.into_iter().take(20) {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let l = gtk::Label::new(Some(&name));
                l.set_xalign(0.0);
                l.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                l.set_width_chars(34);
                l.set_max_width_chars(34);
                l.set_margin_top(6);
                l.set_margin_bottom(6);
                l.set_margin_start(10);
                l.set_margin_end(10);
                l.set_tooltip_text(Some(&p.to_string_lossy()));
                list.append(&l);
                stored.push(p);
            }
        });
    }
    {
        let (paths, app, pop) = (paths.clone(), app.clone(), popover.clone());
        list.connect_row_activated(move |_, row| {
            let idx = row.index();
            if idx >= 0 {
                if let Some(p) = paths.borrow().get(idx as usize).cloned() {
                    pop.popdown();
                    app.open_path(p);
                }
            }
        });
    }
    popover
}
