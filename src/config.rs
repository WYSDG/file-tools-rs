use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn save_dir() -> PathBuf {
    let d = home().join("Pictures").join("FileTools");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Flat output — everything goes into ~/Pictures/FileTools/
pub fn sub_dir(_name: &str) -> PathBuf {
    save_dir()
}

pub fn config_dir() -> PathBuf {
    let d = home().join(".config").join("file-tools");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn history_file() -> PathBuf {
    save_dir().join(".history.json")
}

pub fn slots_file() -> PathBuf {
    config_dir().join("color_slots.json")
}

pub fn presets_file() -> PathBuf {
    config_dir().join("presets.json")
}

pub fn tmp_dir() -> PathBuf {
    let d = std::env::temp_dir().join("file-tools-rs");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn tmp_file(suffix: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4().simple().to_string();
    tmp_dir().join(format!("{}{}", id, suffix))
}

pub fn bg_remover_bin() -> PathBuf {
    home()
        .join("bg-remover-env")
        .join("bin")
        .join("backgroundremover")
}
