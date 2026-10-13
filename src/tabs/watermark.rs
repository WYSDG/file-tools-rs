use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;
use crate::util;

pub struct WatermarkTab {
    pub base: BaseTab,
    pub text: gtk::Entry,
    pub font: gtk::Entry,
    pub bold: gtk::ToggleButton,
    pub italic: gtk::ToggleButton,
    pub underline: gtk::ToggleButton,
    pub case_dd: gtk::DropDown,
    pub h_pos: gtk::DropDown,
    pub v_pos: gtk::DropDown,
    pub op: gtk::Scale,
    pub size: gtk::Scale,
    pub pad: gtk::Scale,
    pub color_btn: gtk::ColorDialogButton,
    pub fmt: gtk::DropDown,
}

impl WatermarkTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new(
            "Gradient",
            Some("Gradients"),
            "Gradient Generator",
            app.clone(),
        );

        let text_lbl = gtk::Label::new(Some("Text"));
        text_lbl.set_xalign(0.0);
        base.controls.append(&text_lbl);
        let text = gtk::Entry::new();
        text.set_text("(c) YourName");
        base.controls.append(&text);

        let style_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let bold = gtk::ToggleButton::with_label("B");
        let italic = gtk::ToggleButton::with_label("I");
        let underline = gtk::ToggleButton::with_label("U");
        let case_dd =
            gtk::DropDown::from_strings(&["None", "UPPERCASE", "lowercase", "Title Case"]);
        style_row.append(&bold);
        style_row.append(&italic);
        style_row.append(&underline);
        style_row.append(&gtk::Label::new(Some("Case")));
        style_row.append(&case_dd);
        base.controls.append(&style_row);

        let font_lbl = gtk::Label::new(Some("Font"));
        font_lbl.set_xalign(0.0);
        base.controls.append(&font_lbl);
        let font = gtk::Entry::new();
        font.set_placeholder_text(Some("Font family or /path/to/font.ttf"));
        base.controls.append(&font);

        let pos_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        pos_row.append(&gtk::Label::new(Some("H")));
        let h_pos = gtk::DropDown::from_strings(&["Left", "Center", "Right"]);
        h_pos.set_selected(2);
        pos_row.append(&h_pos);
        pos_row.append(&gtk::Label::new(Some("V")));
        let v_pos = gtk::DropDown::from_strings(&["Top", "Middle", "Bottom"]);
        v_pos.set_selected(2);
        pos_row.append(&v_pos);
        base.controls.append(&pos_row);

        let op = base.tracked_slider("Opacity %", 0.0, 100.0, 30.0);
        let size = base.tracked_slider("Size pt", 8.0, 200.0, 36.0);
        let pad = base.tracked_slider("Padding px", 0.0, 200.0, 10.0);

        let color_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        color_row.append(&gtk::Label::new(Some("Color")));
        let dialog = gtk::ColorDialog::new();
        let color_btn = gtk::ColorDialogButton::new(Some(dialog));
        color_btn.set_rgba(&gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
        color_row.append(&color_btn);
        color_row.append(&crate::util::make_eyedropper_button(
            color_btn.clone(),
            app.clone(),
        ));
        base.controls.append(&color_row);
        {
            let b = base.clone();
            color_btn.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);
        let fmt = gtk::DropDown::from_strings(&["Same as input", "png", "jpg", "webp"]);
        base.controls.append(&fmt);

        base.register_entry("text", &text);
        base.register_entry("font", &font);
        base.register_toggle("bold", &bold);
        base.register_toggle("italic", &italic);
        base.register_toggle("underline", &underline);
        base.register_dropdown("case", &case_dd);
        base.register_dropdown("h_pos", &h_pos);
        base.register_dropdown("v_pos", &v_pos);
        base.register_slider("op", &op);
        base.register_slider("size", &size);
        base.register_slider("pad", &pad);
        base.register_color("color", &color_btn);
        base.register_dropdown("fmt", &fmt);

        Rc::new(Self {
            base,
            text,
            font,
            bold,
            italic,
            underline,
            case_dd,
            h_pos,
            v_pos,
            op,
            size,
            pad,
            color_btn,
            fmt,
        })
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };

        let ext = match self.fmt.selected() {
            0 => input
                .extension()
                .and_then(|e| e.to_str())
                .filter(|e| matches!(*e, "png" | "jpg" | "jpeg" | "webp"))
                .unwrap_or("png")
                .to_string(),
            1 => "png".to_string(),
            2 => "jpg".to_string(),
            _ => "webp".to_string(),
        };
        let out = config::tmp_file(&format!(".{}", ext));

        let mut txt = self.text.text().to_string();
        if txt.is_empty() {
            txt = "(c)".into();
        }
        txt = match self.case_dd.selected() {
            1 => txt.to_uppercase(),
            2 => txt.to_lowercase(),
            3 => txt
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => txt,
        };

        let font_input = self.font.text().to_string();
        let font_path = util::resolve_font(&font_input);

        let h = ["Left", "Center", "Right"]
            .get(self.h_pos.selected() as usize)
            .copied()
            .unwrap_or("Right");
        let v = ["Top", "Middle", "Bottom"]
            .get(self.v_pos.selected() as usize)
            .copied()
            .unwrap_or("Bottom");
        let gravity = match (h, v) {
            ("Left", "Top") => "northwest",
            ("Center", "Top") => "north",
            ("Right", "Top") => "northeast",
            ("Left", "Middle") => "west",
            ("Center", "Middle") => "center",
            ("Right", "Middle") => "east",
            ("Left", "Bottom") => "southwest",
            ("Center", "Bottom") => "south",
            _ => "southeast",
        };

        let rgba = self.color_btn.rgba();
        let color_hex = format!(
            "#{:02X}{:02X}{:02X}",
            (rgba.red() * 255.0) as u8,
            (rgba.green() * 255.0) as u8,
            (rgba.blue() * 255.0) as u8,
        );

        let op = self.op.value() as i32;
        let sz = self.size.value() as i32;
        let pad = self.pad.value() as i32;
        let safe = txt.replace('\\', "\\\\").replace('"', "\\\"");

        let mut inner: Vec<String> = vec![
            "-clone".into(),
            "0".into(),
            "-fill".into(),
            color_hex.clone(),
            "-pointsize".into(),
            sz.to_string(),
        ];
        if let Some(fp) = font_path {
            inner.push("-font".into());
            inner.push(fp);
        }
        inner.push("-weight".into());
        inner.push(if self.bold.is_active() {
            "700".into()
        } else {
            "400".into()
        });
        if self.italic.is_active() {
            inner.push("-style".into());
            inner.push("Italic".into());
        }
        if self.underline.is_active() {
            inner.push("-undercolor".into());
            inner.push(color_hex.clone());
            inner.push("-stroke".into());
            inner.push(color_hex);
        }
        inner.push("-gravity".into());
        inner.push(gravity.into());
        inner.push("-annotate".into());
        inner.push(format!("+{}+{}", pad, pad));
        inner.push(safe);

        let mut cmd: Vec<String> = vec![
            "magick".into(),
            input.to_string_lossy().into_owned(),
            "(".into(),
        ];
        cmd.extend(inner);
        cmd.push(")".into());
        cmd.extend([
            "-compose".into(),
            "dissolve".into(),
            "-define".into(),
            format!("compose:args={}", op),
            "-composite".into(),
        ]);
        if matches!(ext.as_str(), "jpg" | "jpeg") {
            cmd.extend(["-quality".into(), "92".into()]);
        }
        cmd.push(out.to_string_lossy().into_owned());

        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, vec![cmd]);
    }
}
