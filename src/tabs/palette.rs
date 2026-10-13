use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::AppState;
use crate::tabs::base::BaseTab;
use crate::util;
use crate::widgets::swatch::make_swatch;

pub struct PaletteTab {
    pub base: BaseTab,
    pub n_colors: gtk::Scale,
    pub palette_box: gtk::FlowBox,
    pub active_swatch: gtk::DrawingArea,
    pub active_hex: gtk::Label,
    pub tints_box: gtk::FlowBox,
    pub shades_box: gtk::FlowBox,
    pub midtones_box: gtk::FlowBox,
    pub active_color: Rc<RefCell<String>>,
}

impl PaletteTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new("Palette", None, "Palette Generator", app);
        base.go.set_label("Extract Palette (Ctrl+Enter)");

        let n_colors = base.tracked_slider("Colors to extract", 3.0, 16.0, 8.0);

        let pal_hdr = gtk::Label::new(Some("EXTRACTED — click to copy"));
        pal_hdr.set_xalign(0.0);
        pal_hdr.add_css_class("section-label");
        base.controls.append(&pal_hdr);

        let palette_box = gtk::FlowBox::new();
        palette_box.set_selection_mode(gtk::SelectionMode::None);
        palette_box.set_max_children_per_line(12);
        base.controls.append(&palette_box);

        let active_hdr = gtk::Label::new(Some("ACTIVE COLOR — click to copy"));
        active_hdr.set_xalign(0.0);
        active_hdr.add_css_class("section-label");
        base.controls.append(&active_hdr);

        let active_swatch = gtk::DrawingArea::new();
        active_swatch.set_size_request(-1, 48);
        let active_color: Rc<RefCell<String>> = Rc::new(RefCell::new("#808080".to_string()));
        {
            let ac = active_color.clone();
            active_swatch.set_draw_func(move |_, cr, w, h| {
                let hex = ac.borrow().clone();
                if let Some((r, g, b)) = util::hex_to_rgb(&hex) {
                    cr.set_source_rgb(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
                    cr.rectangle(0.0, 0.0, w as f64, h as f64);
                    let _ = cr.fill();
                    cr.set_source_rgba(0.55, 0.55, 0.55, 0.5);
                    cr.set_line_width(1.0);
                    cr.rectangle(0.5, 0.5, w as f64 - 1.0, h as f64 - 1.0);
                    let _ = cr.stroke();
                }
            });
        }
        base.controls.append(&active_swatch);

        // Click the swatch to copy the active hex
        {
            let ac = active_color.clone();
            let st = base.clone();
            let gesture = gtk::GestureClick::new();
            gesture.set_button(1);
            gesture.connect_released(move |_, _, _, _| {
                let hex = ac.borrow().clone();
                if util::copy_text_to_clipboard(&hex) {
                    st.set_status(&format!("✓ Copied {}", hex), false);
                } else {
                    st.set_status("Could not copy — is wl-clipboard installed?", true);
                }
            });
            active_swatch.add_controller(gesture);
        }

        let active_hex = gtk::Label::new(Some("#808080"));
        active_hex.set_xalign(0.0);
        active_hex.add_css_class("big-hex");
        base.controls.append(&active_hex);

        let t_hdr = gtk::Label::new(Some("TINTS"));
        t_hdr.set_xalign(0.0);
        t_hdr.add_css_class("section-label");
        base.controls.append(&t_hdr);
        let tints_box = gtk::FlowBox::new();
        tints_box.set_selection_mode(gtk::SelectionMode::None);
        tints_box.set_max_children_per_line(12);
        base.controls.append(&tints_box);

        let s_hdr = gtk::Label::new(Some("SHADES"));
        s_hdr.set_xalign(0.0);
        s_hdr.add_css_class("section-label");
        base.controls.append(&s_hdr);
        let shades_box = gtk::FlowBox::new();
        shades_box.set_selection_mode(gtk::SelectionMode::None);
        shades_box.set_max_children_per_line(12);
        base.controls.append(&shades_box);

        let m_hdr = gtk::Label::new(Some("MIDTONES"));
        m_hdr.set_xalign(0.0);
        m_hdr.add_css_class("section-label");
        base.controls.append(&m_hdr);
        let midtones_box = gtk::FlowBox::new();
        midtones_box.set_selection_mode(gtk::SelectionMode::None);
        midtones_box.set_max_children_per_line(12);
        base.controls.append(&midtones_box);

        Rc::new(Self {
            base,
            n_colors,
            palette_box,
            active_swatch,
            active_hex,
            tints_box,
            shades_box,
            midtones_box,
            active_color,
        })
    }

    fn clear_box(b: &gtk::FlowBox) {
        while let Some(c) = b.first_child() {
            b.remove(&c);
        }
    }

    pub fn set_active_color(self: &Rc<Self>, hex: &str) {
        *self.active_color.borrow_mut() = hex.to_string();
        self.active_hex.set_text(hex);
        self.active_swatch.queue_draw();
        self.refresh_ramps();
    }

    fn refresh_ramps(self: &Rc<Self>) {
        let hex = self.active_color.borrow().clone();
        Self::clear_box(&self.tints_box);
        Self::clear_box(&self.shades_box);
        Self::clear_box(&self.midtones_box);

        let mk = |hexs: Vec<String>, tc: Rc<PaletteTab>, box_: &gtk::FlowBox| {
            for h in hexs {
                let tc2 = tc.clone();
                let h2 = h.clone();
                let cb: Rc<dyn Fn(String)> = Rc::new(move |_| {
                    if util::copy_text_to_clipboard(&h2) {
                        tc2.base.set_status(&format!("✓ Copied {}", h2), false);
                    } else {
                        tc2.base.set_status("Copy failed", true);
                    }
                });
                let sw = make_swatch(&h, 28, Some(cb));
                box_.append(&sw);
            }
        };
        let tc = self.clone();
        mk(util::tints(&hex, 9), tc.clone(), &self.tints_box);
        mk(util::shades(&hex, 9), tc.clone(), &self.shades_box);
        mk(util::midtones(&hex, 7), tc, &self.midtones_box);
    }

    fn populate_palette(self: &Rc<Self>, colors: Vec<String>) {
        Self::clear_box(&self.palette_box);
        if colors.is_empty() {
            self.base.set_status("No colors extracted", true);
            return;
        }
        for hex in &colors {
            let tc = self.clone();
            let h = hex.clone();
            let cb: Rc<dyn Fn(String)> = Rc::new(move |_| {
                tc.set_active_color(&h);
            });
            let sw = make_swatch(hex, 44, Some(cb));
            self.palette_box.append(&sw);
        }
        let first = colors[0].clone();
        self.set_active_color(&first);
        self.base
            .set_status(&format!("Extracted {} colors", colors.len()), false);
    }

    pub fn extract(self: &Rc<Self>) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let n = self.n_colors.value() as i32;
        self.base.set_status("Extracting…", false);
        let tc = self.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            let colors: Result<Vec<String>, String> =
                gtk::gio::spawn_blocking(move || util::extract_palette(&input, n))
                    .await
                    .unwrap_or_else(|_| Err("task panicked".to_string()));
            match colors {
                Ok(cs) => tc.populate_palette(cs),
                Err(e) => tc.base.set_status(&e, true),
            }
        });
    }
}
