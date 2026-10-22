//! "Save As" dialog: name, folder, format, quality, optional resize (with ratio lock).
//! Always saves the *current* image (the History head), even with no edits or after Undo.

use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::app::AppState;
use crate::export::{export_image, ExportOptions, Format};

fn labelled_row(text: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_width_chars(8);
    row.append(&l);
    widget.set_hexpand(true);
    row.append(widget);
    row
}

fn sanitize_stem(s: &str) -> String {
    let stem = Path::new(s.trim())
        .file_stem()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_default();
    stem.chars().filter(|c| !matches!(c, '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect()
}

pub fn show(app: &AppState, parent: &gtk::Window) {
    let Some(src) = app.current_head() else {
        app.toast("Open an image first.");
        return;
    };
    let settings = app.settings.borrow().clone();
    let (src_w, src_h) = crate::util::image_dimensions(&src).unwrap_or((0, 0));

    let win = gtk::Window::builder()
        .title("Save As")
        .modal(true)
        .transient_for(parent)
        .resizable(false)
        .default_width(470)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(18);
    root.set_margin_end(18);
    win.set_child(Some(&root));

    // ---- name ----
    let current = app.fname.text().to_string();
    let name_entry = gtk::Entry::new();
    let stem0 = sanitize_stem(&current);
    name_entry.set_text(if stem0.is_empty() { "image" } else { &stem0 });
    root.append(&labelled_row("Name", &name_entry));

    // ---- folder ----
    let folder: Rc<RefCell<PathBuf>> = Rc::new(RefCell::new(settings.effective_output_dir()));
    let folder_lbl = gtk::Label::new(Some(&folder.borrow().to_string_lossy()));
    folder_lbl.set_xalign(0.0);
    folder_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Start);
    folder_lbl.set_hexpand(true);
    let change_btn = gtk::Button::with_label("Change…");
    let folder_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    folder_box.append(&folder_lbl);
    folder_box.append(&change_btn);
    root.append(&labelled_row("Folder", &folder_box));
    {
        let (folder, lbl, win_c) = (folder.clone(), folder_lbl.clone(), win.clone());
        change_btn.connect_clicked(move |_| {
            let dlg = gtk::FileDialog::builder().title("Choose a folder").build();
            let (folder, lbl) = (folder.clone(), lbl.clone());
            dlg.select_folder(Some(&win_c), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(f) = res {
                    if let Some(p) = f.path() {
                        lbl.set_text(&p.to_string_lossy());
                        *folder.borrow_mut() = p;
                    }
                }
            });
        });
    }

    // ---- format + quality ----
    let format_dd = gtk::DropDown::from_strings(&["PNG", "JPEG", "WebP"]);
    let start_fmt = Path::new(&current)
        .extension()
        .and_then(|e| Format::from_ext(&e.to_string_lossy()))
        .map(|f| match f {
            Format::Png => 0,
            Format::Jpeg => 1,
            Format::Webp => 2,
        })
        .unwrap_or(settings.save_format.min(2));
    format_dd.set_selected(start_fmt);
    root.append(&labelled_row("Format", &format_dd));

    let quality = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 100.0, 1.0);
    quality.set_value(settings.save_quality.clamp(1, 100) as f64);
    quality.set_draw_value(true);
    quality.set_value_pos(gtk::PositionType::Right);
    let quality_row = labelled_row("Quality", &quality);
    root.append(&quality_row);
    {
        let (row, dd) = (quality_row.clone(), format_dd.clone());
        let update = move || row.set_visible(dd.selected() != 0); // PNG is lossless
        update();
        format_dd.connect_selected_notify(move |_| update());
    }

    // ---- resize (optional, with ratio lock) ----
    let resize_chk = gtk::CheckButton::with_label("Resize image");
    root.append(&resize_chk);
    let w_entry = gtk::Entry::new();
    let h_entry = gtk::Entry::new();
    w_entry.set_text(&src_w.to_string());
    h_entry.set_text(&src_h.to_string());
    w_entry.set_width_chars(6);
    h_entry.set_width_chars(6);
    let lock = gtk::CheckButton::with_label("Keep ratio");
    lock.set_active(true);
    let size_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    size_row.set_margin_start(18);
    for w in [
        gtk::Label::new(Some("W")).upcast::<gtk::Widget>(),
        w_entry.clone().upcast(),
        gtk::Label::new(Some("H")).upcast(),
        h_entry.clone().upcast(),
        lock.clone().upcast(),
    ] {
        size_row.append(&w);
    }
    size_row.set_visible(false);
    root.append(&size_row);
    {
        let row = size_row.clone();
        resize_chk.connect_toggled(move |c| row.set_visible(c.is_active()));
    }
    let syncing = Rc::new(Cell::new(false));
    for (this, other, from_w) in [(&w_entry, &h_entry, true), (&h_entry, &w_entry, false)] {
        let (other, lock, syncing) = (other.clone(), lock.clone(), syncing.clone());
        this.connect_changed(move |e| {
            if syncing.get() || !lock.is_active() || src_w == 0 || src_h == 0 {
                return;
            }
            if let Ok(v) = e.text().trim().parse::<f64>() {
                let r = if from_w { v * src_h as f64 / src_w as f64 } else { v * src_w as f64 / src_h as f64 };
                syncing.set(true);
                other.set_text(&(r.round().max(1.0) as u32).to_string());
                syncing.set(false);
            }
        });
    }

    // ---- buttons ----
    let btns = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btns.set_halign(gtk::Align::End);
    btns.set_margin_top(6);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save");
    save.add_css_class("suggested-action");
    btns.append(&cancel);
    btns.append(&save);
    root.append(&btns);
    name_entry.set_activates_default(true);
    win.set_default_widget(Some(&save));
    {
        let w = win.clone();
        cancel.connect_clicked(move |_| w.close());
    }

    {
        let (app, win_c) = (app.clone(), win.clone());
        save.connect_clicked(move |_| {
            let stem = sanitize_stem(&name_entry.text());
            if stem.is_empty() {
                app.toast("Please enter a file name.");
                return;
            }
            let format = match format_dd.selected() {
                1 => Format::Jpeg,
                2 => Format::Webp,
                _ => Format::Png,
            };
            let q = quality.value().round() as u8;
            let resize = if resize_chk.is_active() {
                match (w_entry.text().trim().parse::<u32>(), h_entry.text().trim().parse::<u32>()) {
                    (Ok(w), Ok(h)) if w > 0 && h > 0 => Some((w, h)),
                    _ => {
                        app.toast("Enter a valid width and height.");
                        return;
                    }
                }
            } else {
                None
            };
            {
                let mut s = app.settings.borrow_mut();
                s.save_format = format_dd.selected();
                s.save_quality = q as u32;
                s.save();
            }
            let dst = folder.borrow().join(format!("{}.{}", stem, format.ext()));
            let opts = ExportOptions { format, quality: q, resize };
            win_c.close();

            let (app_c, src_c, dst_c) = (app.clone(), src.clone(), dst.clone());
            let do_save: Rc<dyn Fn()> = Rc::new(move || start_export(&app_c, src_c.clone(), dst_c.clone(), opts));
            let exists = dst.exists();
            if exists && app.settings.borrow().confirm_overwrite {
                let name = dst.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                app.toast_actions(
                    &format!("“{}” already exists. Overwrite it?", name),
                    vec![("✓  Overwrite".to_string(), do_save), ("✕".to_string(), Rc::new(|| {}))],
                    10,
                );
            } else {
                do_save();
            }
        });
    }

    win.present();
}

/// Encode on a worker thread, then report with a toast.
fn start_export(app: &AppState, src: PathBuf, dst: PathBuf, opts: ExportOptions) {
    app.toast("Saving…");
    let exif = app.settings.borrow().exiftool_args();
    let app = app.clone();
    gtk::glib::spawn_future_local(async move {
        let (d, s) = (dst.clone(), src.clone());
        let result = gtk::gio::spawn_blocking(move || {
            export_image(&s, &d, &opts)?;
            if let Some(args) = exif {
                let _ = std::process::Command::new("exiftool").args(&args).arg(&d).output();
            }
            Ok::<(), String>(())
        })
        .await
        .unwrap_or_else(|_| Err("the save task crashed".to_string()));
        match result {
            Ok(()) => {
                if let Some(n) = dst.file_name() {
                    // Keep the name the owner chose; tools no longer overwrite it.
                    app.fname.set_text(&n.to_string_lossy());
                }
                app.toast(&format!("Saved → {}", dst.display()));
                app.refresh_panels();
            }
            Err(e) => app.toast(&format!("Save failed: {e}")),
        }
    });
}
