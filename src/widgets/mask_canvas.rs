use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;

#[derive(Clone)]
struct Stroke {
    points: Vec<(f64, f64)>,
    radius: i32,
}

#[derive(Clone)]
pub struct MaskCanvas {
    pub widget: gtk::DrawingArea,
    surface: Rc<RefCell<Option<cairo::ImageSurface>>>,
    img_dims: Rc<RefCell<(u32, u32)>>,
    strokes: Rc<RefCell<Vec<Stroke>>>,
    current_stroke: Rc<RefCell<Option<Stroke>>>,
    redo_stack: Rc<RefCell<Vec<Stroke>>>,
    brush_radius: Rc<RefCell<i32>>,
    cursor: Rc<RefCell<Option<(f64, f64)>>>,
    painting_enabled: Rc<Cell<bool>>,
}

impl MaskCanvas {
    pub fn new() -> Self {
        let widget = gtk::DrawingArea::new();
        widget.set_vexpand(true);
        widget.set_size_request(-1, 320);

        let surface: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::new(RefCell::new(None));
        let img_dims: Rc<RefCell<(u32, u32)>> = Rc::new(RefCell::new((0, 0)));
        let strokes: Rc<RefCell<Vec<Stroke>>> = Rc::new(RefCell::new(Vec::new()));
        let current_stroke: Rc<RefCell<Option<Stroke>>> = Rc::new(RefCell::new(None));
        let redo_stack: Rc<RefCell<Vec<Stroke>>> = Rc::new(RefCell::new(Vec::new()));
        let brush_radius: Rc<RefCell<i32>> = Rc::new(RefCell::new(40));
        let cursor: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let painting_enabled: Rc<Cell<bool>> = Rc::new(Cell::new(true));

        {
            let s = surface.clone();
            let st = strokes.clone();
            let cs = current_stroke.clone();
            let br = brush_radius.clone();
            let cur = cursor.clone();
            let pen = painting_enabled.clone();
            widget.set_draw_func(move |area, cr, w, h| {
                // Follow the theme: dark text colour => light UI => light mask backdrop.
                let fg = area.color();
                let light_ui = (0.2126 * fg.red() + 0.7152 * fg.green() + 0.0722 * fg.blue()) < 0.5;
                draw_all(
                    cr,
                    w,
                    h,
                    light_ui,
                    &s.borrow(),
                    &st.borrow(),
                    cs.borrow().as_ref(),
                    *br.borrow(),
                    *cur.borrow(),
                    pen.get(),
                );
            });
        }

        // ---- paint drag ----
        {
            let surface_c = surface.clone();
            let dims = img_dims.clone();
            let st = strokes.clone();
            let cs = current_stroke.clone();
            let br = brush_radius.clone();
            let widget_ref = widget.clone();
            let start_ix: Rc<RefCell<(f64, f64)>> = Rc::new(RefCell::new((0.0, 0.0)));
            let pen_begin = painting_enabled.clone();

            let drag = gtk::GestureDrag::new();
            drag.set_button(1);

            {
                let start_ix = start_ix.clone();
                let dims = dims.clone();
                let cs = cs.clone();
                let br = br.clone();
                let surface_c = surface_c.clone();
                let widget_ref = widget_ref.clone();
                let pen = pen_begin.clone();
                drag.connect_drag_begin(move |_, x, y| {
                    if !pen.get() {
                        return;
                    }
                    if surface_c.borrow().is_none() {
                        return;
                    }
                    let (dx, dy, scale) = transform(&widget_ref, *dims.borrow());
                    let ix = (x - dx) / scale;
                    let iy = (y - dy) / scale;
                    let iw = dims.borrow().0 as f64;
                    let ih = dims.borrow().1 as f64;
                    if ix < 0.0 || ix >= iw || iy < 0.0 || iy >= ih {
                        return;
                    }
                    *start_ix.borrow_mut() = (ix, iy);
                    *cs.borrow_mut() = Some(Stroke {
                        points: vec![(ix, iy)],
                        radius: *br.borrow(),
                    });
                });
            }

            {
                let start_ix = start_ix.clone();
                let dims = dims.clone();
                let cs = cs.clone();
                let widget_ref = widget_ref.clone();
                let pen = painting_enabled.clone();
                drag.connect_drag_update(move |_, ox, oy| {
                    if !pen.get() {
                        return;
                    }
                    let (sx, sy) = *start_ix.borrow();
                    let (_, _, scale) = transform(&widget_ref, *dims.borrow());
                    let nx = sx + ox / scale;
                    let ny = sy + oy / scale;
                    if let Some(stroke) = cs.borrow_mut().as_mut() {
                        let last = stroke.points.last().copied().unwrap_or((sx, sy));
                        let dx = nx - last.0;
                        let dy = ny - last.1;
                        if (dx * dx + dy * dy).sqrt() > 1.0 {
                            stroke.points.push((nx, ny));
                            widget_ref.queue_draw();
                        }
                    }
                });
            }

            {
                let st = st.clone();
                let cs = cs.clone();
                let pen = painting_enabled.clone();
                drag.connect_drag_end(move |_, _, _| {
                    if !pen.get() {
                        return;
                    }
                    if let Some(stroke) = cs.borrow_mut().take() {
                        st.borrow_mut().push(stroke);
                    }
                });
            }

            widget.add_controller(drag);
        }

        // ---- motion ----
        {
            let cur_on_motion = cursor.clone();
            let cur_on_leave = cursor.clone();
            let widget_ref = widget.clone();
            let motion = gtk::EventControllerMotion::new();
            motion.connect_motion(move |_, x, y| {
                *cur_on_motion.borrow_mut() = Some((x, y));
                widget_ref.queue_draw();
            });
            motion.connect_leave(move |_| {
                *cur_on_leave.borrow_mut() = None;
            });
            widget.add_controller(motion);
        }

        Self {
            widget,
            surface,
            img_dims,
            strokes,
            current_stroke,
            redo_stack,
            brush_radius,
            cursor,
            painting_enabled,
        }
    }

    pub fn set_painting_enabled(&self, enabled: bool) {
        self.painting_enabled.set(enabled);
        self.widget.queue_draw();
    }

    pub fn set_image(&self, path: &Path) {
        self.strokes.borrow_mut().clear();
        self.redo_stack.borrow_mut().clear();
        *self.current_stroke.borrow_mut() = None;

        // Fast path: decode in-process and keep transparency (drawn over a checkerboard).
        #[cfg(target_endian = "little")]
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            if let Some(surf) = crate::widgets::preview::surface_from_rgba(&rgba) {
                *self.img_dims.borrow_mut() = (surf.width() as u32, surf.height() as u32);
                *self.surface.borrow_mut() = Some(surf);
                self.widget.queue_draw();
                return;
            }
        }

        // Fallback for formats the `image` crate can't read: let magick convert, keep alpha.
        let tmp = crate::config::tmp_file(".png");
        let ok = Command::new("magick")
            .args([
                path.to_string_lossy().as_ref(),
                "-background",
                "none",
                "-alpha",
                "on",
                tmp.to_string_lossy().as_ref(),
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            *self.surface.borrow_mut() = None;
            *self.img_dims.borrow_mut() = (0, 0);
            self.widget.queue_draw();
            return;
        }
        if let Ok(mut file) = std::fs::File::open(&tmp) {
            if let Ok(surf) = cairo::ImageSurface::create_from_png(&mut file) {
                *self.img_dims.borrow_mut() = (surf.width() as u32, surf.height() as u32);
                *self.surface.borrow_mut() = Some(surf);
            }
        }
        self.widget.queue_draw();
    }

    pub fn set_brush(&self, radius: i32) {
        *self.brush_radius.borrow_mut() = radius;
        self.widget.queue_draw();
    }

    pub fn undo(&self) {
        if let Some(s) = self.strokes.borrow_mut().pop() {
            self.redo_stack.borrow_mut().push(s);
            self.widget.queue_draw();
        }
    }

    pub fn redo(&self) {
        if let Some(s) = self.redo_stack.borrow_mut().pop() {
            self.strokes.borrow_mut().push(s);
            self.widget.queue_draw();
        }
    }

    pub fn clear(&self) {
        self.strokes.borrow_mut().clear();
        self.redo_stack.borrow_mut().clear();
        *self.current_stroke.borrow_mut() = None;
        self.widget.queue_draw();
    }

    pub fn has_image(&self) -> bool {
        self.surface.borrow().is_some()
    }

    pub fn has_strokes(&self) -> bool {
        !self.strokes.borrow().is_empty()
    }

    pub fn build_mask(&self, out_path: &Path) -> Result<(), String> {
        let (iw, ih) = *self.img_dims.borrow();
        if iw == 0 || ih == 0 {
            return Err("No image loaded".into());
        }

        let mut mask = image::GrayImage::new(iw, ih);
        for s in self.strokes.borrow().iter() {
            let r = s.radius as i32;
            if s.points.len() == 1 {
                draw_dot(&mut mask, s.points[0], r);
            } else {
                for w in s.points.windows(2) {
                    let (x0, y0) = w[0];
                    let (x1, y1) = w[1];
                    draw_line(&mut mask, (x0, y0), (x1, y1), r);
                }
                if let Some(&first) = s.points.first() {
                    draw_dot(&mut mask, first, r);
                }
                if let Some(&last) = s.points.last() {
                    draw_dot(&mut mask, last, r);
                }
            }
        }

        mask.save(out_path).map_err(|e| e.to_string())
    }
}

fn transform(widget: &gtk::DrawingArea, dims: (u32, u32)) -> (f64, f64, f64) {
    let w = widget.width() as f64;
    let h = widget.height() as f64;
    let (iw, ih) = (dims.0 as f64, dims.1 as f64);
    if iw == 0.0 || ih == 0.0 {
        return (0.0, 0.0, 1.0);
    }
    let scale = (w / iw).min(h / ih);
    let dw = iw * scale;
    let dh = ih * scale;
    ((w - dw) / 2.0, (h - dh) / 2.0, scale)
}

fn draw_all(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    light_ui: bool,
    surface: &Option<cairo::ImageSurface>,
    strokes: &[Stroke],
    current: Option<&Stroke>,
    brush: i32,
    cursor: Option<(f64, f64)>,
    painting_enabled: bool,
) {
    let (panel, check_a, check_b) = if light_ui { (0.90, 0.80, 0.92) } else { (0.14, 0.22, 0.28) };
    cr.set_source_rgb(panel, panel, panel);
    cr.rectangle(0.0, 0.0, w as f64, h as f64);
    let _ = cr.fill();

    let Some(surf) = surface else {
        cr.set_source_rgb(0.55, 0.55, 0.55);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(14.0);
        cr.move_to(16.0, 28.0);
        let _ = cr.show_text("No image loaded");
        return;
    };

    let (iw, ih) = (surf.width() as f64, surf.height() as f64);
    let scale = (w as f64 / iw).min(h as f64 / ih);
    let dw = iw * scale;
    let dh = ih * scale;
    let dx = (w as f64 - dw) / 2.0;
    let dy = (h as f64 - dh) / 2.0;

    // Checkerboard behind the image so transparent areas read as transparent (not white).
    {
        let cell = 10.0_f64;
        let _ = cr.save();
        cr.rectangle(dx, dy, dw, dh);
        cr.clip();
        cr.set_source_rgb(check_a, check_a, check_a);
        let _ = cr.paint();
        cr.set_source_rgb(check_b, check_b, check_b);
        let cols = (dw / cell).ceil() as i32;
        let rows = (dh / cell).ceil() as i32;
        for ry in 0..rows {
            for rx in 0..cols {
                if (rx + ry) % 2 == 0 {
                    cr.rectangle(dx + rx as f64 * cell, dy + ry as f64 * cell, cell, cell);
                }
            }
        }
        let _ = cr.fill();
        let _ = cr.restore();
    }

    let _ = cr.save();
    cr.translate(dx, dy);
    cr.scale(scale, scale);
    let _ = cr.set_source_surface(surf, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();

    if painting_enabled {
        cr.set_source_rgba(1.0, 0.25, 0.25, 0.9);
    } else {
        cr.set_source_rgba(1.0, 0.25, 0.25, 0.35); // dimmed
    }
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);

    let paint_stroke = |cr: &cairo::Context, s: &Stroke| {
        let r = s.radius as f64 * scale;
        if s.points.len() == 1 {
            let (x, y) = s.points[0];
            cr.arc(dx + x * scale, dy + y * scale, r, 0.0, 6.2831853);
            let _ = cr.fill();
        } else {
            cr.set_line_width(r * 2.0);
            cr.move_to(dx + s.points[0].0 * scale, dy + s.points[0].1 * scale);
            for p in &s.points[1..] {
                cr.line_to(dx + p.0 * scale, dy + p.1 * scale);
            }
            let _ = cr.stroke();
        }
    };

    for s in strokes {
        paint_stroke(cr, s);
    }
    if let Some(s) = current {
        paint_stroke(cr, s);
    }

    // Cursor ring — only when painting is enabled
    if painting_enabled {
        if let Some((cx, cy)) = cursor {
            let r = brush as f64 * scale;
            cr.set_line_width(3.0);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.85);
            cr.arc(cx, cy, r, 0.0, 6.2831853);
            let _ = cr.stroke();
            cr.set_line_width(1.5);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            cr.arc(cx, cy, r, 0.0, 6.2831853);
            let _ = cr.stroke();
        }
    }
}

fn draw_dot(mask: &mut image::GrayImage, p: (f64, f64), r: i32) {
    let (w, h) = mask.dimensions();
    let cx = p.0 as i32;
    let cy = p.1 as i32;
    let r2 = r * r;
    for y in (cy - r)..=(cy + r) {
        if y < 0 || y >= h as i32 {
            continue;
        }
        for x in (cx - r)..=(cx + r) {
            if x < 0 || x >= w as i32 {
                continue;
            }
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 {
                mask.put_pixel(x as u32, y as u32, image::Luma([255]));
            }
        }
    }
}

fn draw_line(mask: &mut image::GrayImage, a: (f64, f64), b: (f64, f64), r: i32) {
    let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let steps = (dist / (r.max(1) as f64 * 0.25)).ceil() as i32;
    if steps <= 1 {
        draw_dot(mask, a, r);
        draw_dot(mask, b, r);
        return;
    }
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        draw_dot(mask, p, r);
    }
}
