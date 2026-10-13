use gtk::prelude::*;
use gtk4 as gtk;

use crate::app::AppState;
use crate::config;
use crate::tabs::base::BaseTab;

pub struct PixelateTab {
    pub base: BaseTab,
    pub block: gtk::Scale,
}

impl PixelateTab {
    pub fn new(app: AppState) -> Self {
        let base = BaseTab::new("Pixelate", Some("Pixelate"), "Pixelate", app);
        let block = base.tracked_slider("Block size", 1.0, 99.0, 10.0);
        base.register_slider("block", &block);
        Self { base, block }
    }

    pub fn run(&self, app: &AppState) {
        let Some(input) = self.base.get_input() else {
            self.base.set_status("Load an image first", true);
            return;
        };
        let out = config::tmp_file(".png");
        let pct = self.block.value() as i32;
        let cmds = vec![vec![
            "magick".to_string(),
            input.to_string_lossy().to_string(),
            "-scale".to_string(),
            format!("{}%", pct),
            "-scale".to_string(),
            "1000%".to_string(),
            "-filter".to_string(),
            "point".to_string(),
            out.to_string_lossy().to_string(),
        ]];
        self.base
            .clone()
            .spawn_processing(app.clone(), input, out, cmds);
    }
}
