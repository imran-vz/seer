//! What the app remembers between launches.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub folder: Option<PathBuf>,
    pub sidebar_width: f32,
    pub inspector_width: f32,
    pub sidebar_visible: bool,
    pub inspector_visible: bool,
    pub show_hidden: bool,
    /// "name", "size" or "modified".
    pub sort: String,
    pub ascending: bool,
    /// x, y, width, height of the window in points.
    pub window: Option<[f32; 4]>,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            folder: None,
            sidebar_width: f32::from(crate::theme::size::SIDEBAR.1),
            inspector_width: f32::from(crate::theme::size::INSPECTOR.1),
            sidebar_visible: true,
            inspector_visible: true,
            show_hidden: false,
            sort: "name".into(),
            ascending: true,
            window: None,
        }
    }
}

fn file() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("Seer").join("state.json"))
}

impl Saved {
    pub fn load() -> Saved {
        file()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(file) = file() else { return };
        let write = || -> anyhow::Result<()> {
            std::fs::create_dir_all(file.parent().unwrap_or(&file))?;
            std::fs::write(&file, serde_json::to_vec_pretty(self)?)?;
            Ok(())
        };
        if let Err(e) = write() {
            log::warn!("could not save state: {e:#}");
        }
    }
}
