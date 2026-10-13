use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

fn hex_to_rgb_float(h: &str) -> (f64, f64, f64) {
    let h = h.trim_start_matches('#');
    let r = u8::from_str_radix(&h.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let g = u8::from_str_radix(&h.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let b = u8::from_str_radix(&h.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
    (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
}

pub fn make_swatch(hex: &str, size: i32, onclick: Option<Rc<dyn Fn(String)>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_size_request(size, size);
    let (r, g, b) = hex_to_rgb_float(hex);
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(r, g, b);
        cr.rectangle(0.0, 0.0, w as f64, h as f64);
        let _ = cr.fill();
        cr.set_source_rgba(0.55, 0.55, 0.55, 0.5);
        cr.set_line_width(1.0);
        cr.rectangle(0.5, 0.5, w as f64 - 1.0, h as f64 - 1.0);
        let _ = cr.stroke();
    });
    if let Some(cb) = onclick {
        let h_str = hex.to_string();
        let gesture = gtk::GestureClick::new();
        gesture.set_button(1);
        gesture.connect_released(move |_, _, _, _| {
            cb(h_str.clone());
        });
        area.add_controller(gesture);
    }
    area
}
