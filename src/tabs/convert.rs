use gtk::prelude::*;
use gtk4 as gtk;
use std::path::PathBuf;
use std::process::Command;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct ConvertTab {
    pub base: BaseTab,
    pub fmt_dd: gtk::DropDown,
    pub quality: gtk::Scale,
}

impl ConvertTab {
    pub fn new(app: AppState) -> Self {
        let base = BaseTab::new("Convert", Some("Converted"), "Convert Image", app);

        let fmt_lbl = gtk::Label::new(Some("Output format"));
        fmt_lbl.set_xalign(0.0);
        base.controls.append(&fmt_lbl);

        let fmt_dd = gtk::DropDown::from_strings(&[
            "png", "jpg", "webp", "gif", "bmp", "tiff", "avif", "ico",
        ]);
        base.controls.append(&fmt_dd);

        let q_lbl = gtk::Label::new(Some("Quality: 90"));
        q_lbl.set_xalign(0.0);
        base.controls.append(&q_lbl);

        let quality = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, 100.0, 1.0);
        quality.set_value(90.0);
        base.controls.append(&quality);

        base.register_dropdown("fmt", &fmt_dd);
        base.register_slider("quality", &quality);

        Self {
            base,
            fmt_dd,
            quality,
        }
    }

    pub fn root(&self) -> &gtk::Box {
        &self.base.inspector
    }

    fn build_cmds(&self, input: &PathBuf, out: &PathBuf) -> Vec<Vec<String>> {
        let fmts = ["png", "jpg", "webp", "gif", "bmp", "tiff", "avif", "ico"];
        let idx = self.fmt_dd.selected() as usize;
        let fmt = fmts.get(idx).copied().unwrap_or("png");
        let mut cmd: Vec<String> = vec!["magick".into(), input.to_string_lossy().into_owned()];
        if matches!(fmt, "jpg" | "webp" | "avif") {
            cmd.push("-quality".into());
            cmd.push((self.quality.value() as i32).to_string());
        }
        cmd.push(out.to_string_lossy().into_owned());
        vec![cmd]
    }

    pub fn run(self: &std::rc::Rc<Self>, app: &AppState) {
        let input = match self.base.get_input() {
            Some(p) => p,
            None => {
                self.base.set_status("Load an image first", true);
                return;
            }
        };
        let out_tmp = config::tmp_file(".png");
        let cmds = self.build_cmds(&input, &out_tmp);

        self.base.set_status("Processing…", false);
        self.base.go.set_sensitive(false);

        let base = self.base.clone();
        let app = app.clone();

        gtk::glib::MainContext::default().spawn_local(async move {
            let result: Result<PathBuf, String> = gtk::gio::spawn_blocking(move || {
                for cmd in cmds.iter() {
                    let output = Command::new(&cmd[0]).args(&cmd[1..]).output();
                    match output {
                        Ok(o) if o.status.success() => {}
                        Ok(o) => {
                            return Err(String::from_utf8_lossy(&o.stderr).to_string());
                        }
                        Err(e) => {
                            return Err(format!("spawn error: {}", e));
                        }
                    }
                }
                Ok(out_tmp)
            })
            .await
            .unwrap_or_else(|_| Err("task panicked".to_string()));

            base.go.set_sensitive(true);
            match result {
                Ok(out_path) => {
                    base.set_status("Done.", false);
                    app.commit_history("Convert", out_path);
                }
                Err(e) => {
                    base.set_status(&e, true);
                }
            }
        });
    }
}
