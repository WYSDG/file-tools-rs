use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::process::Command;
use std::rc::Rc;

type CropRect = (u32, u32, u32, u32); // x, y, w, h in image space

#[derive(Clone)]
pub struct Preview {
    pub root: gtk::Box,
    pub canvas: gtk::DrawingArea,
    original: Rc<RefCell<Option<cairo::ImageSurface>>>,
    result: Rc<RefCell<Option<cairo::ImageSurface>>>,
    pixels: Rc<RefCell<Option<image::RgbaImage>>>,
    fade: Rc<RefCell<f64>>,
    zoom: Rc<RefCell<f64>>,
    pan_x: Rc<RefCell<f64>>,
    pan_y: Rc<RefCell<f64>>,
    last_mouse: Rc<RefCell<Option<(f64, f64)>>>,
    drag_start: Rc<RefCell<Option<(f64, f64)>>>,
    eyedropper: Rc<Cell<bool>>,
    eyedropper_cb: Rc<RefCell<Option<Rc<dyn Fn(String)>>>>,
    on_hover_hex: Rc<RefCell<Option<Rc<dyn Fn(Option<String>)>>>>,
    crop_mode: Rc<Cell<bool>>,
    crop_overlay: Rc<RefCell<Option<CropRect>>>,
    crop_drag_live: Rc<RefCell<Option<CropRect>>>,
    crop_drag_start: Rc<RefCell<Option<(f64, f64)>>>,
    crop_cb: Rc<RefCell<Option<Rc<dyn Fn(Option<CropRect>)>>>>,
    bg_color: Rc<RefCell<[f64; 3]>>,
    on_cursor: Rc<RefCell<Option<Rc<dyn Fn(Option<(i32, i32)>)>>>>,
    /// The "100%" label, so other widgets (status bar) can mirror it.
    pub zoom_lbl: gtk::Label,
    pub zoom_in_btn: gtk::Button,
    pub zoom_out_btn: gtk::Button,
    pub fit_btn: gtk::Button,
    pub one_btn: gtk::Button,
}

impl Preview {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);

        let on_cursor: Rc<RefCell<Option<Rc<dyn Fn(Option<(i32, i32)>)>>>> = Rc::new(RefCell::new(None));

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        controls.set_halign(gtk::Align::Start);

        let zoom_out = gtk::Button::with_label("−");
        let zoom_lbl = gtk::Label::new(Some("100%"));
        zoom_lbl.set_width_chars(6);
        let zoom_in = gtk::Button::with_label("+");
        let fit_btn = gtk::Button::with_label("Fit");
        let one_btn = gtk::Button::with_label("1:1");

        controls.append(&zoom_out);
        controls.append(&zoom_lbl);
        controls.append(&zoom_in);
        controls.append(&fit_btn);
        controls.append(&one_btn);
        root.append(&controls);

        let canvas = gtk::DrawingArea::new();
        canvas.set_vexpand(true);
        canvas.set_size_request(-1, 320);
        canvas.set_focusable(false);
        root.append(&canvas);

        let original: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::new(RefCell::new(None));
        let result: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::new(RefCell::new(None));
        let pixels: Rc<RefCell<Option<image::RgbaImage>>> = Rc::new(RefCell::new(None));
        let fade: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));
        let zoom: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
        let pan_x: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));
        let pan_y: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));
        let last_mouse: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let drag_start: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let eyedropper: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let eyedropper_cb: Rc<RefCell<Option<Rc<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));
        let on_hover_hex: Rc<RefCell<Option<Rc<dyn Fn(Option<String>)>>>> =
            Rc::new(RefCell::new(None));
        let crop_mode: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let crop_overlay: Rc<RefCell<Option<CropRect>>> = Rc::new(RefCell::new(None));
        let crop_drag_live: Rc<RefCell<Option<CropRect>>> = Rc::new(RefCell::new(None));
        let crop_drag_start: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let crop_cb: Rc<RefCell<Option<Rc<dyn Fn(Option<CropRect>)>>>> =
            Rc::new(RefCell::new(None));
        let bg_color: Rc<RefCell<[f64; 3]>> = Rc::new(RefCell::new([0.045, 0.050, 0.075]));

        // ---- draw ----
        {
            let o = original.clone();
            let r = result.clone();
            let f = fade.clone();
            let z = zoom.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let eb = eyedropper.clone();
            let cm = crop_mode.clone();
            let co = crop_overlay.clone();
            let cdl = crop_drag_live.clone();
            let bg = bg_color.clone();
            canvas.set_draw_func(move |_, cr, w, h| {
                draw_layers(
                    cr,
                    w,
                    h,
                    o.borrow().as_ref(),
                    r.borrow().as_ref(),
                    *f.borrow(),
                    *z.borrow(),
                    *px.borrow(),
                    *py.borrow(),
                    eb.get(),
                    cm.get(),
                    co.borrow().as_ref().copied(),
                    cdl.borrow().as_ref().copied(),
                    *bg.borrow(),
                );
            });
        }

        // ---- scroll wheel zoom ----
        {
            let o = original.clone();
            let r = result.clone();
            let z = zoom.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let last = last_mouse.clone();
            let cv = canvas.clone();
            let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            scroll.connect_scroll(move |_, _, dy| {
                let w = cv.width() as f64;
                let h = cv.height() as f64;
                if w <= 0.0 || h <= 0.0 {
                    return gtk::glib::Propagation::Proceed;
                }
                let dims = r
                    .borrow()
                    .as_ref()
                    .map(|s| (s.width() as f64, s.height() as f64))
                    .or_else(|| {
                        o.borrow()
                            .as_ref()
                            .map(|s| (s.width() as f64, s.height() as f64))
                    });
                let Some((iw, ih)) = dims else {
                    return gtk::glib::Propagation::Proceed;
                };
                let (mx, my) = last.borrow().unwrap_or((w / 2.0, h / 2.0));
                let factor = if dy < 0.0 { 1.15 } else { 1.0 / 1.15 };
                let old_zoom = *z.borrow();
                let new_zoom = (old_zoom * factor).clamp(0.01, 100.0);
                if (new_zoom - old_zoom).abs() < 1e-6 {
                    return gtk::glib::Propagation::Stop;
                }
                let base = (w / iw).min(h / ih);
                let old_scale = base * old_zoom;
                let old_dx = (w - iw * old_scale) / 2.0 + *px.borrow();
                let old_dy = (h - ih * old_scale) / 2.0 + *py.borrow();
                let ix = (mx - old_dx) / old_scale;
                let iy = (my - old_dy) / old_scale;
                let new_scale = base * new_zoom;
                let new_cx = (w - iw * new_scale) / 2.0;
                let new_cy = (h - ih * new_scale) / 2.0;
                *px.borrow_mut() = mx - new_cx - ix * new_scale;
                *py.borrow_mut() = my - new_cy - iy * new_scale;
                *z.borrow_mut() = new_zoom;
                cv.queue_draw();
                gtk::glib::Propagation::Stop
            });
            canvas.add_controller(scroll);
        }

        // ---- trackpad pinch ----
        {
            let o = original.clone();
            let r = result.clone();
            let z = zoom.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let cv = canvas.clone();
            let last = last_mouse.clone();
            let prev_scale: Rc<RefCell<f64>> = Rc::new(RefCell::new(1.0));
            let pinch = gtk::GestureZoom::new();
            {
                let prev = prev_scale.clone();
                pinch.connect_begin(move |_, _| {
                    *prev.borrow_mut() = 1.0;
                });
            }
            pinch.connect_scale_changed(move |_, scale| {
                let w = cv.width() as f64;
                let h = cv.height() as f64;
                if w <= 0.0 || h <= 0.0 {
                    return;
                }
                let dims = r
                    .borrow()
                    .as_ref()
                    .map(|s| (s.width() as f64, s.height() as f64))
                    .or_else(|| {
                        o.borrow()
                            .as_ref()
                            .map(|s| (s.width() as f64, s.height() as f64))
                    });
                let Some((iw, ih)) = dims else { return };
                let prev = *prev_scale.borrow();
                if prev <= 0.0 {
                    return;
                }
                let factor = scale / prev;
                *prev_scale.borrow_mut() = scale;
                let old_zoom = *z.borrow();
                let new_zoom = (old_zoom * factor).clamp(0.01, 100.0);
                if (new_zoom - old_zoom).abs() < 1e-9 {
                    return;
                }
                let (cx, cy) = last.borrow().unwrap_or((w / 2.0, h / 2.0));
                let base = (w / iw).min(h / ih);
                let old_scale = base * old_zoom;
                let old_dx = (w - iw * old_scale) / 2.0 + *px.borrow();
                let old_dy = (h - ih * old_scale) / 2.0 + *py.borrow();
                let ix = (cx - old_dx) / old_scale;
                let iy = (cy - old_dy) / old_scale;
                let new_scale = base * new_zoom;
                let new_cx = (w - iw * new_scale) / 2.0;
                let new_cy = (h - ih * new_scale) / 2.0;
                *px.borrow_mut() = cx - new_cx - ix * new_scale;
                *py.borrow_mut() = cy - new_cy - iy * new_scale;
                *z.borrow_mut() = new_zoom;
                cv.queue_draw();
            });
            canvas.add_controller(pinch);
        }

        // ---- motion ----
        {
            let last_on_motion = last_mouse.clone();
            let last_on_leave = last_mouse.clone();
            let widget_ref = canvas.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let zoom_c = zoom.clone();
            let orig = original.clone();
            let res = result.clone();
            let pxc = pixels.clone();
            let ebc = eyedropper.clone();
            let hcb_motion = on_hover_hex.clone();
            let hcb_leave = on_hover_hex.clone();
            let cur_motion = on_cursor.clone();
            let cur_leave = on_cursor.clone();

            let motion = gtk::EventControllerMotion::new();
            motion.connect_motion(move |_, x, y| {
                *last_on_motion.borrow_mut() = Some((x, y));
                if let Some(cb) = cur_motion.borrow().as_ref() {
                    let pos = widget_to_image(
                        &widget_ref,
                        *zoom_c.borrow(),
                        *px.borrow(),
                        *py.borrow(),
                        &orig,
                        &res,
                        x,
                        y,
                    )
                    .map(|(ix, iy)| (ix as i32, iy as i32));
                    cb(pos);
                }
                if ebc.get() {
                    if let Some((ix, iy)) = widget_to_image(
                        &widget_ref,
                        *zoom_c.borrow(),
                        *px.borrow(),
                        *py.borrow(),
                        &orig,
                        &res,
                        x,
                        y,
                    ) {
                        if let Some(buf) = pxc.borrow().as_ref() {
                            if ix >= 0.0
                                && iy >= 0.0
                                && (ix as u32) < buf.width()
                                && (iy as u32) < buf.height()
                            {
                                let p = buf.get_pixel(ix as u32, iy as u32);
                                let hex = format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]);
                                if let Some(cb) = hcb_motion.borrow().as_ref() {
                                    cb(Some(hex));
                                }
                            }
                        }
                    }
                }
                widget_ref.queue_draw();
            });
            motion.connect_leave(move |_| {
                if let Some(cb) = cur_leave.borrow().as_ref() {
                    cb(None);
                }
                *last_on_leave.borrow_mut() = None;
                if let Some(cb) = hcb_leave.borrow().as_ref() {
                    cb(None);
                }
            });
            canvas.add_controller(motion);
        }

        // ---- left click eyedropper ----
        {
            let orig = original.clone();
            let res = result.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let zoom_c = zoom.clone();
            let pxc = pixels.clone();
            let ebc = eyedropper.clone();
            let ed_cb = eyedropper_cb.clone();
            let widget_ref = canvas.clone();
            let click = gtk::GestureClick::new();
            click.set_button(1);
            click.connect_released(move |_, _, x, y| {
                if !ebc.get() {
                    return;
                }
                if let Some((ix, iy)) = widget_to_image(
                    &widget_ref,
                    *zoom_c.borrow(),
                    *px.borrow(),
                    *py.borrow(),
                    &orig,
                    &res,
                    x,
                    y,
                ) {
                    if let Some(buf) = pxc.borrow().as_ref() {
                        if ix >= 0.0
                            && iy >= 0.0
                            && (ix as u32) < buf.width()
                            && (iy as u32) < buf.height()
                        {
                            let p = buf.get_pixel(ix as u32, iy as u32);
                            let hex = format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]);
                            if let Some(cb) = ed_cb.borrow().as_ref() {
                                cb(hex);
                            }
                        }
                    }
                }
                ebc.set(false);
                widget_ref.set_cursor_from_name(Some("default"));
                widget_ref.queue_draw();
            });
            canvas.add_controller(click);
        }

        // ---- left drag crop ----
        {
            let orig = original.clone();
            let res = result.clone();
            let zoom_c = zoom.clone();
            let cm = crop_mode.clone();
            let cds = crop_drag_start.clone();
            let cdl = crop_drag_live.clone();
            let ccb = crop_cb.clone();
            let co = crop_overlay.clone();
            let widget_ref = canvas.clone();

            let drag = gtk::GestureDrag::new();
            drag.set_button(1);

            {
                let orig = orig.clone();
                let res = res.clone();
                let px = pan_x.clone();
                let py = pan_y.clone();
                let zoom_c = zoom_c.clone();
                let cm = cm.clone();
                let cds = cds.clone();
                let cdl = cdl.clone();
                let widget_ref = widget_ref.clone();
                drag.connect_drag_begin(move |_, x, y| {
                    if !cm.get() {
                        return;
                    }
                    if let Some((ix, iy)) = widget_to_image(
                        &widget_ref,
                        *zoom_c.borrow(),
                        *px.borrow(),
                        *py.borrow(),
                        &orig,
                        &res,
                        x,
                        y,
                    ) {
                        *cds.borrow_mut() = Some((ix, iy));
                        *cdl.borrow_mut() = Some((ix as u32, iy as u32, 0, 0));
                        widget_ref.queue_draw();
                    }
                });
            }

            {
                let orig = orig.clone();
                let res = res.clone();
                let zoom_c = zoom_c.clone();
                let cds = cds.clone();
                let cdl = cdl.clone();
                let widget_ref = widget_ref.clone();
                drag.connect_drag_update(move |_, ox, oy| {
                    let Some((sx, sy)) = *cds.borrow() else {
                        return;
                    };
                    let w = widget_ref.width() as f64;
                    let h = widget_ref.height() as f64;
                    if w <= 0.0 || h <= 0.0 {
                        return;
                    }
                    let dims = res
                        .borrow()
                        .as_ref()
                        .map(|s| (s.width() as f64, s.height() as f64))
                        .or_else(|| {
                            orig.borrow()
                                .as_ref()
                                .map(|s| (s.width() as f64, s.height() as f64))
                        });
                    let Some((iw, ih)) = dims else { return };
                    let base = (w / iw).min(h / ih);
                    let scale = base * *zoom_c.borrow();
                    if scale <= 0.0 {
                        return;
                    }
                    let cur_x = (sx + ox / scale).clamp(0.0, iw);
                    let cur_y = (sy + oy / scale).clamp(0.0, ih);
                    let x0 = sx.min(cur_x);
                    let y0 = sy.min(cur_y);
                    let x1 = sx.max(cur_x);
                    let y1 = sy.max(cur_y);
                    *cdl.borrow_mut() = Some((
                        x0 as u32,
                        y0 as u32,
                        (x1 - x0).max(0.0) as u32,
                        (y1 - y0).max(0.0) as u32,
                    ));
                    widget_ref.queue_draw();
                });
            }

            {
                let cdl = cdl.clone();
                let ccb = ccb.clone();
                let co = co.clone();
                let widget_ref = widget_ref.clone();
                drag.connect_drag_end(move |_, _, _| {
                    let live = cdl.borrow_mut().take();
                    if let Some(rect) = live {
                        if rect.2 >= 8 && rect.3 >= 8 {
                            *co.borrow_mut() = Some(rect);
                            if let Some(cb) = ccb.borrow().as_ref() {
                                cb(Some(rect));
                            }
                        }
                    }
                    widget_ref.queue_draw();
                });
            }

            canvas.add_controller(drag);
        }

        // ---- right drag pan ----
        {
            let px_begin = pan_x.clone();
            let py_begin = pan_y.clone();
            let px_update = pan_x.clone();
            let py_update = pan_y.clone();
            let ds_begin = drag_start.clone();
            let ds_update = drag_start.clone();
            let ds_end = drag_start.clone();
            let cv = canvas.clone();
            let drag = gtk::GestureDrag::new();
            drag.set_button(3);
            drag.connect_drag_begin(move |_, _, _| {
                *ds_begin.borrow_mut() = Some((*px_begin.borrow(), *py_begin.borrow()));
            });
            drag.connect_drag_update(move |_, ox, oy| {
                let Some((spx, spy)) = *ds_update.borrow() else {
                    return;
                };
                *px_update.borrow_mut() = spx + ox;
                *py_update.borrow_mut() = spy + oy;
                cv.queue_draw();
            });
            drag.connect_drag_end(move |_, _, _| {
                *ds_end.borrow_mut() = None;
            });
            canvas.add_controller(drag);
        }

        // ---- zoom buttons ----
        {
            let z = zoom.clone();
            let lbl = zoom_lbl.clone();
            let cv = canvas.clone();
            zoom_out.connect_clicked(move |_| {
                let mut zz = z.borrow_mut();
                *zz = (*zz / 1.25).clamp(0.01, 100.0);
                lbl.set_text(&format!("{}%", (*zz * 100.0) as i32));
                cv.queue_draw();
            });
        }
        {
            let z = zoom.clone();
            let lbl = zoom_lbl.clone();
            let cv = canvas.clone();
            zoom_in.connect_clicked(move |_| {
                let mut zz = z.borrow_mut();
                *zz = (*zz * 1.25).clamp(0.01, 100.0);
                lbl.set_text(&format!("{}%", (*zz * 100.0) as i32));
                cv.queue_draw();
            });
        }
        {
            let z = zoom.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let lbl = zoom_lbl.clone();
            let cv = canvas.clone();
            fit_btn.connect_clicked(move |_| {
                *z.borrow_mut() = 1.0;
                *px.borrow_mut() = 0.0;
                *py.borrow_mut() = 0.0;
                lbl.set_text("100%");
                cv.queue_draw();
            });
        }
        {
            let z = zoom.clone();
            let px = pan_x.clone();
            let py = pan_y.clone();
            let lbl = zoom_lbl.clone();
            let cv = canvas.clone();
            let o = original.clone();
            let r = result.clone();
            one_btn.connect_clicked(move |_| {
                let dims = r
                    .borrow()
                    .as_ref()
                    .map(|s| (s.width() as f64, s.height() as f64))
                    .or_else(|| {
                        o.borrow()
                            .as_ref()
                            .map(|s| (s.width() as f64, s.height() as f64))
                    });
                let Some((iw, ih)) = dims else { return };
                let w = cv.width() as f64;
                let h = cv.height() as f64;
                if w <= 0.0 || h <= 0.0 {
                    return;
                }
                let base = (w / iw).min(h / ih);
                if base <= 0.0 {
                    return;
                }
                let new_zoom = 1.0 / base;
                *z.borrow_mut() = new_zoom;
                *px.borrow_mut() = 0.0;
                *py.borrow_mut() = 0.0;
                lbl.set_text(&format!("{}%", (new_zoom * 100.0) as i32));
                cv.queue_draw();
            });
        }

        Self {
            root,
            canvas,
            original,
            result,
            pixels,
            fade,
            zoom,
            pan_x,
            pan_y,
            last_mouse,
            drag_start,
            eyedropper,
            eyedropper_cb,
            on_hover_hex,
            crop_mode,
            crop_overlay,
            crop_drag_live,
            crop_drag_start,
            crop_cb,
            bg_color,
            on_cursor,
            zoom_lbl,
            zoom_in_btn: zoom_in,
            zoom_out_btn: zoom_out,
            fit_btn,
            one_btn,
        }
    }

    /// Called with the image pixel under the pointer (None when outside the image / left the canvas).
    pub fn set_on_cursor(&self, cb: Rc<dyn Fn(Option<(i32, i32)>)>) {
        *self.on_cursor.borrow_mut() = Some(cb);
    }

    pub fn set_original(&self, path: &Path) {
        let (surf, px) = load_surface(path);
        *self.original.borrow_mut() = surf;
        if let Some(p) = px {
            *self.pixels.borrow_mut() = Some(p);
        }
        *self.zoom.borrow_mut() = 1.0;
        *self.pan_x.borrow_mut() = 0.0;
        *self.pan_y.borrow_mut() = 0.0;
        self.canvas.queue_draw();
    }

    pub fn set_result(&self, path: &Path) {
        let (surf, px) = load_surface(path);
        *self.result.borrow_mut() = surf;
        if let Some(p) = px {
            *self.pixels.borrow_mut() = Some(p);
        }
        *self.fade.borrow_mut() = 1.0;
        *self.zoom.borrow_mut() = 1.0;
        *self.pan_x.borrow_mut() = 0.0;
        *self.pan_y.borrow_mut() = 0.0;
        self.canvas.queue_draw();
    }

    pub fn clear_result(&self) {
        *self.result.borrow_mut() = None;
        *self.fade.borrow_mut() = 0.0;
        self.canvas.queue_draw();
    }

    pub fn set_fade(&self, v: f64) {
        *self.fade.borrow_mut() = v;
        self.canvas.queue_draw();
    }

    pub fn is_mouse_inside(&self) -> bool {
        self.last_mouse.borrow().is_some()
    }

    pub fn set_eyedropper_mode(&self, active: bool, cb: Option<Rc<dyn Fn(String)>>) {
        self.eyedropper.set(active);
        *self.eyedropper_cb.borrow_mut() = cb;
        if active {
            self.crop_mode.set(false);
            self.canvas.set_cursor_from_name(Some("crosshair"));
        } else {
            self.canvas.set_cursor_from_name(Some("default"));
        }
    }

    pub fn set_on_hover_hex(&self, cb: Rc<dyn Fn(Option<String>)>) {
        *self.on_hover_hex.borrow_mut() = Some(cb);
    }

    pub fn is_eyedropper_active(&self) -> bool {
        self.eyedropper.get()
    }

    pub fn set_crop_mode(&self, enabled: bool) {
        self.crop_mode.set(enabled);
        if enabled {
            self.eyedropper.set(false);
            self.canvas.set_cursor_from_name(Some("crosshair"));
        } else {
            self.canvas.set_cursor_from_name(Some("default"));
        }
        self.canvas.queue_draw();
    }

    pub fn set_crop_overlay(&self, rect: Option<CropRect>) {
        *self.crop_overlay.borrow_mut() = rect;
        self.canvas.queue_draw();
    }

    pub fn get_crop_overlay(&self) -> Option<CropRect> {
        *self.crop_overlay.borrow()
    }

    pub fn set_on_crop_changed(&self, cb: Rc<dyn Fn(Option<CropRect>)>) {
        *self.crop_cb.borrow_mut() = Some(cb);
    }

    pub fn set_bg_color(&self, rgb: [f64; 3]) {
        *self.bg_color.borrow_mut() = rgb;
        self.canvas.queue_draw();
    }
}

fn widget_to_image(
    widget: &gtk::DrawingArea,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    original: &Rc<RefCell<Option<cairo::ImageSurface>>>,
    result: &Rc<RefCell<Option<cairo::ImageSurface>>>,
    x: f64,
    y: f64,
) -> Option<(f64, f64)> {
    let w = widget.width() as f64;
    let h = widget.height() as f64;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let dims = result
        .borrow()
        .as_ref()
        .map(|s| (s.width() as f64, s.height() as f64))
        .or_else(|| {
            original
                .borrow()
                .as_ref()
                .map(|s| (s.width() as f64, s.height() as f64))
        });
    let Some((iw, ih)) = dims else { return None };
    let base = (w / iw).min(h / ih);
    let scale = base * zoom;
    let dw = iw * scale;
    let dh = ih * scale;
    let dx = (w - dw) / 2.0 + pan_x;
    let dy = (h - dh) / 2.0 + pan_y;
    let ix = (x - dx) / scale;
    let iy = (y - dy) / scale;
    if ix >= 0.0 && iy >= 0.0 && ix < iw && iy < ih {
        Some((ix, iy))
    } else {
        None
    }
}

/// Build a Cairo surface straight from RGBA pixels (premultiplied BGRA on little-endian).
#[cfg(target_endian = "little")]
pub(crate) fn surface_from_rgba(img: &image::RgbaImage) -> Option<cairo::ImageSurface> {
    let (w, h) = img.dimensions();
    let (w, h) = (w as usize, h as usize);
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).ok()?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface.data().ok()?;
        let src = img.as_raw();
        for y in 0..h {
            let row_src = &src[y * w * 4..(y + 1) * w * 4];
            let row_dst = &mut data[y * stride..y * stride + w * 4];
            for (d, s) in row_dst.chunks_exact_mut(4).zip(row_src.chunks_exact(4)) {
                let a = s[3] as u32;
                if a == 255 {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                } else {
                    d[0] = ((s[2] as u32 * a + 127) / 255) as u8;
                    d[1] = ((s[1] as u32 * a + 127) / 255) as u8;
                    d[2] = ((s[0] as u32 * a + 127) / 255) as u8;
                }
                d[3] = s[3];
            }
        }
    }
    surface.mark_dirty();
    Some(surface)
}

fn load_surface(path: &Path) -> (Option<cairo::ImageSurface>, Option<image::RgbaImage>) {
    // Fast path: decode once in-process. No `magick` subprocess, no temp PNG,
    // no second decode. Falls back to magick for formats the `image` crate can't read.
    #[cfg(target_endian = "little")]
    if let Ok(img) = image::open(path) {
        let rgba = img.to_rgba8();
        if let Some(surface) = surface_from_rgba(&rgba) {
            return (Some(surface), Some(rgba));
        }
    }

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
        return (None, None);
    }

    let surface = std::fs::File::open(&tmp)
        .ok()
        .and_then(|mut f| cairo::ImageSurface::create_from_png(&mut f).ok());

    let pixels = image::open(&tmp).ok().map(|i| i.to_rgba8());

    (surface, pixels)
}

#[allow(clippy::too_many_arguments)]
fn draw_layers(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    original: Option<&cairo::ImageSurface>,
    result: Option<&cairo::ImageSurface>,
    fade: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    eyedropper: bool,
    crop_mode: bool,
    crop_overlay: Option<CropRect>,
    crop_drag_live: Option<CropRect>,
    bg: [f64; 3],
) {
    // Canvas background
    cr.set_source_rgb(bg[0], bg[1], bg[2]);
    cr.rectangle(0.0, 0.0, w as f64, h as f64);
    let _ = cr.fill();

    // Faint grid, tinted to stay visible on every theme (incl. pure-black OLED and light).
    let lum = 0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2];
    if lum < 0.02 {
        cr.set_source_rgba(0.60, 0.62, 0.70, 0.17);
    } else if lum < 0.5 {
        cr.set_source_rgba(0.30, 0.34, 0.50, 0.15);
    } else {
        cr.set_source_rgba(0.15, 0.18, 0.30, 0.10);
    }
    cr.set_line_width(1.0);
    let step = 20.0_f64;
    let mut gx = 0.0_f64;
    while gx <= w as f64 {
        cr.move_to(gx, 0.0);
        cr.line_to(gx, h as f64);
        gx += step;
    }
    let mut gy = 0.0_f64;
    while gy <= h as f64 {
        cr.move_to(0.0, gy);
        cr.line_to(w as f64, gy);
        gy += step;
    }
    let _ = cr.stroke();

    if original.is_none() && result.is_none() {
        cr.set_source_rgb(0.55, 0.55, 0.55);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(14.0);
        cr.move_to(16.0, 28.0);
        let _ = cr.show_text("No image loaded");
        return;
    }

    let (iw, ih) = {
        let s = result.or(original).unwrap();
        (s.width() as f64, s.height() as f64)
    };
    let base = (w as f64 / iw).min(h as f64 / ih);
    let scale = base * zoom;
    let dw = iw * scale;
    let dh = ih * scale;
    let dx = (w as f64 - dw) / 2.0 + pan_x;
    let dy = (h as f64 - dh) / 2.0 + pan_y;

    // Checkerboard behind the image so transparent areas are always visible
    // (e.g. black halftone dots on a transparent background over a black canvas).
    {
        let lum = 0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2];
        let (a, b) = if lum < 0.5 { (0.21, 0.31) } else { (0.78, 0.92) };
        let cell = 8i32;
        if let Ok(tile) = cairo::ImageSurface::create(cairo::Format::Rgb24, cell * 2, cell * 2) {
            if let Ok(tcr) = cairo::Context::new(&tile) {
                tcr.set_source_rgb(a, a, a);
                let _ = tcr.paint();
                tcr.set_source_rgb(b, b, b);
                tcr.rectangle(0.0, 0.0, cell as f64, cell as f64);
                tcr.rectangle(cell as f64, cell as f64, cell as f64, cell as f64);
                let _ = tcr.fill();
            }
            let pat = cairo::SurfacePattern::create(&tile);
            pat.set_extend(cairo::Extend::Repeat);
            pat.set_filter(cairo::Filter::Nearest);
            pat.set_matrix(cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -dx, -dy));
            let _ = cr.save();
            cr.rectangle(dx, dy, dw, dh);
            cr.clip();
            let _ = cr.set_source(&pat);
            let _ = cr.paint();
            let _ = cr.restore();
        }
    }

    let draw_original = !(result.is_some() && fade >= 0.99);
    if draw_original {
        if let Some(o) = original {
            let _ = cr.save();
            cr.translate(dx, dy);
            cr.scale(scale, scale);
            let _ = cr.set_source_surface(o, 0.0, 0.0);
            let _ = cr.paint();
            let _ = cr.restore();
        }
    }
    if let Some(r) = result {
        if fade > 0.0 {
            let _ = cr.save();
            cr.translate(dx, dy);
            cr.scale(scale, scale);
            let _ = cr.set_source_surface(r, 0.0, 0.0);
            let _ = cr.paint_with_alpha(fade);
            let _ = cr.restore();
        }
    }

    if eyedropper {
        cr.set_source_rgba(1.0, 0.9, 0.4, 0.06);
        cr.rectangle(0.0, 0.0, w as f64, h as f64);
        let _ = cr.fill();
    }

    if crop_mode {
        let active = crop_drag_live.or(crop_overlay);
        if let Some((rx, ry, rw, rh)) = active {
            let sx = dx + rx as f64 * scale;
            let sy = dy + ry as f64 * scale;
            let sw = rw as f64 * scale;
            let sh = rh as f64 * scale;

            if sw > 1.0 && sh > 1.0 {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
                cr.rectangle(0.0, 0.0, w as f64, sy);
                let _ = cr.fill();
                cr.rectangle(0.0, sy + sh, w as f64, h as f64 - sy - sh);
                let _ = cr.fill();
                cr.rectangle(0.0, sy, sx, sh);
                let _ = cr.fill();
                cr.rectangle(sx + sw, sy, w as f64 - sx - sw, sh);
                let _ = cr.fill();

                cr.set_source_rgba(1.0, 0.62, 0.15, 1.0);
                cr.set_line_width(2.0);
                cr.rectangle(sx, sy, sw, sh);
                let _ = cr.stroke();

                let h_len = 14.0;
                let h_w = 3.0;
                cr.rectangle(sx - h_w / 2.0, sy - h_w / 2.0, h_len, h_w);
                let _ = cr.fill();
                cr.rectangle(sx - h_w / 2.0, sy - h_w / 2.0, h_w, h_len);
                let _ = cr.fill();
                cr.rectangle(sx + sw - h_len + h_w / 2.0, sy - h_w / 2.0, h_len, h_w);
                let _ = cr.fill();
                cr.rectangle(sx + sw - h_w / 2.0, sy - h_w / 2.0, h_w, h_len);
                let _ = cr.fill();
                cr.rectangle(sx - h_w / 2.0, sy + sh - h_w / 2.0, h_len, h_w);
                let _ = cr.fill();
                cr.rectangle(sx - h_w / 2.0, sy + sh - h_len + h_w / 2.0, h_w, h_len);
                let _ = cr.fill();
                cr.rectangle(sx + sw - h_len + h_w / 2.0, sy + sh - h_w / 2.0, h_len, h_w);
                let _ = cr.fill();
                cr.rectangle(sx + sw - h_w / 2.0, sy + sh - h_len + h_w / 2.0, h_w, h_len);
                let _ = cr.fill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_from_rgba_is_premultiplied_bgra() {
        let mut img = image::RgbaImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([200, 100, 50, 255]));
        img.put_pixel(1, 0, image::Rgba([200, 100, 50, 128]));
        let mut s = surface_from_rgba(&img).unwrap();
        s.flush();
        let d = s.data().unwrap();
        // opaque pixel: BGRA order, unchanged values
        assert_eq!(&d[0..4], &[50, 100, 200, 255]);
        // half-transparent pixel: colour channels premultiplied by alpha
        assert_eq!(d[4], ((50u32 * 128 + 127) / 255) as u8);
        assert_eq!(d[5], ((100u32 * 128 + 127) / 255) as u8);
        assert_eq!(d[6], ((200u32 * 128 + 127) / 255) as u8);
        assert_eq!(d[7], 128);
    }
}
