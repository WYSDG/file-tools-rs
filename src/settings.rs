use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Serialize, Deserialize)]
pub struct Layout {
    pub tabs: HashMap<String, bool>,
}

impl Layout {
    pub fn load() -> Self {
        let path = crate::config::config_dir().join("layout.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(l) = serde_json::from_str::<Layout>(&data) {
                return l;
            }
        }
        Layout::default()
    }

    pub fn save(&self) {
        let path = crate::config::config_dir().join("layout.json");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        *self.tabs.get(name).unwrap_or(&true)
    }

    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.tabs.insert(name.to_string(), enabled);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct TabPresets {
    pub tabs: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl TabPresets {
    pub fn load() -> Self {
        let path = crate::config::config_dir().join("tab_presets.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(p) = serde_json::from_str::<TabPresets>(&data) {
                return p;
            }
        }
        TabPresets::default()
    }

    pub fn save(&self) {
        let path = crate::config::config_dir().join("tab_presets.json");
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn get(&self, tab: &str, preset: &str) -> Option<&serde_json::Value> {
        self.tabs.get(tab).and_then(|t| t.get(preset))
    }

    pub fn set(&mut self, tab: &str, preset: &str, data: serde_json::Value) {
        self.tabs
            .entry(tab.to_string())
            .or_default()
            .insert(preset.to_string(), data);
    }

    pub fn delete(&mut self, tab: &str, preset: &str) {
        if let Some(t) = self.tabs.get_mut(tab) {
            t.remove(preset);
        }
    }

    pub fn list(&self, tab: &str) -> Vec<String> {
        self.tabs
            .get(tab)
            .map(|t| {
                let mut names: Vec<_> = t.keys().cloned().collect();
                names.sort();
                names
            })
            .unwrap_or_default()
    }
}
