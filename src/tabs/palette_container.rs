use gtk::prelude::*;
use gtk4 as gtk;
use std::path::PathBuf;
use std::rc::Rc;

use crate::app::AppState;
use crate::tabs::gradient::GradientTab;
use crate::tabs::palette::PaletteTab;

pub struct PaletteContainer {
    pub inspector: gtk::Box,
    pub nb: gtk::Notebook,
    pub colors: Rc<PaletteTab>,
    pub gradient: Rc<GradientTab>,
    pub go_proxy: gtk::Button,
    app: AppState,
}

impl PaletteContainer {
    pub fn new(app: AppState) -> Rc<Self> {
        let inspector = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let nb = gtk::Notebook::new();
        nb.set_scrollable(true);

        let colors = PaletteTab::new(app.clone());
        let gradient = GradientTab::new(app.clone());

        gradient.base.set_live_preview(true, 400);

        nb.append_page(
            &colors.base.inspector,
            Some(&gtk::Label::new(Some("Colors"))),
        );
        nb.append_page(
            &gradient.base.inspector,
            Some(&gtk::Label::new(Some("Gradient"))),
        );
        inspector.append(&nb);

        let go_proxy = gtk::Button::new();

        let container = Rc::new(Self {
            inspector,
            nb,
            colors,
            gradient,
            go_proxy: go_proxy.clone(),
            app,
        });

        {
            let cc = container.clone();
            container.colors.base.go.connect_clicked(move |_| {
                cc.colors.extract();
            });
        }
        {
            let cc = container.clone();
            container.gradient.base.go.connect_clicked(move |_| {
                cc.gradient.run(&cc.app);
            });
        }
        {
            let gc = container.gradient.clone();
            let ac = container.app.clone();
            container
                .gradient
                .base
                .set_live_trigger(Rc::new(move || gc.run(&ac)));
        }
        {
            let cc = container.clone();
            go_proxy.connect_clicked(move |_| match cc.nb.current_page() {
                Some(0) => cc.colors.extract(),
                Some(1) => cc.gradient.run(&cc.app),
                _ => {}
            });
        }

        container
    }

    pub fn set_input(&self, path: Option<PathBuf>, display: Option<String>) {
        self.colors.base.set_input(path.clone(), display.clone());
        self.gradient.base.set_input(path, display);
    }

    pub fn current_meta(&self) -> (PathBuf, String) {
        match self.nb.current_page() {
            Some(1) => (
                self.gradient.base.save_dir(),
                self.gradient.base.action_name.clone(),
            ),
            _ => (
                self.colors.base.save_dir(),
                self.colors.base.action_name.clone(),
            ),
        }
    }
}
