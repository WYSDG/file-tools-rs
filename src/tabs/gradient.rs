use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct GradientTab {
    pub base: BaseTab,
    pub kind: gtk::DropDown,
    pub mode: gtk::DropDown,
    pub c1: gtk::ColorDialogButton,
    pub c2: gtk::ColorDialogButton,
    pub angle: gtk::Scale,
    pub width: gtk::Scale,
    pub height: gtk::Scale,
}

impl GradientTab {
    pub fn new(app: AppState) -> Rc<Self> {
        let base = BaseTab::new(
            "Gradient",
            Some("Gradients"),
            "Gradient Generator",
            app.clone(),
        );

        let k_lbl = gtk::Label::new(Some("Type"));
        k_lbl.set_xalign(0.0);
        base.controls.append(&k_lbl);
        let kind = gtk::DropDown::from_strings(&["Linear", "Radial"]);
        base.controls.append(&kind);

        let m_lbl = gtk::Label::new(Some("Mode"));
        m_lbl.set_xalign(0.0);
        base.controls.append(&m_lbl);
        let mode =
            gtk::DropDown::from_strings(&["Standalone gradient", "Background for current image"]);
        base.controls.append(&mode);

        let c1_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        c1_row.append(&gtk::Label::new(Some("Color 1")));
        let d1 = gtk::ColorDialog::new();
        let c1 = gtk::ColorDialogButton::new(Some(d1));
        c1.set_rgba(&gtk::gdk::RGBA::new(0.23, 0.43, 0.65, 1.0));
        c1_row.append(&c1);
        c1_row.append(&crate::util::make_eyedropper_button(
            c1.clone(),
            app.clone(),
        ));
        base.controls.append(&c1_row);
        {
            let b = base.clone();
            c1.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let c2_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        c2_row.append(&gtk::Label::new(Some("Color 2")));
        let d2 = gtk::ColorDialog::new();
        let c2 = gtk::ColorDialogButton::new(Some(d2));
        c2.set_rgba(&gtk::gdk::RGBA::new(0.91, 0.57, 0.18, 1.0));
        c2_row.append(&c2);
        c2_row.append(&crate::util::make_eyedropper_button(
            c2.clone(),
            app.clone(),
        ));
        base.controls.append(&c2_row);
        {
            let b = base.clone();
            c2.connect_rgba_notify(move |_| b.schedule_live_process());
        }

        let swap_btn = gtk::Button::with_label("⇅ Swap");
        {
            let c1c = c1.clone();
            let c2c = c2.clone();
            swap_btn.connect_clicked(move |_| {
                let a = c1c.rgba();
                let b = c2c.rgba();
                c1c.set_rgba(&b);
                c2c.set_rgba(&a);
            });
        }
        base.controls.append(&swap_btn);

        let angle = base.tracked_slider("Angle", 0.0, 360.0, 0.0);
        let width = base.tracked_slider("Width", 100.0, 4000.0, 1200.0);
        let height = base.tracked_slider("Height", 100.0, 4000.0, 1200.0);

        base.register_dropdown("kind", &kind);
        base.register_dropdown("mode", &mode);
        base.register_color("c1", &c1);
        base.register_color("c2", &c2);
        base.register_slider("angle", &angle);
        base.register_slider("width", &width);
        base.register_slider("height", &height);

        Rc::new(Self {
            base,
            kind,
            mode,
            c1,
            c2,
            angle,
            width,
            height,
        })
    }

    fn hex_of(btn: &gtk::ColorDialogButton) -> String {
        let r = btn.rgba();
        format!(
            "#{:02X}{:02X}{:02X}",
            (r.red() * 255.0) as u8,
            (r.green() * 255.0) as u8,
            (r.blue() * 255.0) as u8,
        )
    }

    pub fn run(self: &Rc<Self>, app: &AppState) {
        let input_opt = self.base.get_input();
        let w = self.width.value() as i32;
        let h = self.height.value() as i32;
        let c1 = Self::hex_of(&self.c1);
        let c2 = Self::hex_of(&self.c2);
        let out = config::tmp_file(".png");

        let mut cmd: Vec<String> = vec!["magick".into()];

        if self.kind.selected() == 0 {
            let ang = self.angle.value() as i32;
            cmd.extend([
                "-size".into(),
                format!("{}x{}", w, h),
                format!("gradient:{}-{}", c1, c2),
                "-rotate".into(),
                ang.to_string(),
                "-gravity".into(),
                "center".into(),
                "-extent".into(),
                format!("{}x{}", w, h),
            ]);
        } else {
            cmd.extend([
                "-size".into(),
                format!("{}x{}", w, h),
                format!("radial-gradient:{}-{}", c1, c2),
            ]);
        }

        // Composite with input if requested
        let cmds = if self.mode.selected() == 1 {
            let Some(input) = input_opt.clone() else {
                self.base
                    .set_status("Load an image first for background mode", true);
                return;
            };
            // magick input ( gradient ) +swap -compose Over -composite -resize WxH out
            let mut composite: Vec<String> = vec![
                "magick".into(),
                input.to_string_lossy().into_owned(),
                "(".into(),
            ];
            composite.extend(cmd[1..].to_vec()); // skip the "magick" at start
            composite.extend([
                ")".into(),
                "+swap".into(),
                "-compose".into(),
                "Over".into(),
                "-composite".into(),
                "-resize".into(),
                format!("{}x{}", w, h),
                out.to_string_lossy().into_owned(),
            ]);
            vec![composite]
        } else {
            cmd.push(out.to_string_lossy().into_owned());
            vec![cmd]
        };

        let input = input_opt.unwrap_or_else(|| config::tmp_file(".png"));
        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, cmds);
    }
}
