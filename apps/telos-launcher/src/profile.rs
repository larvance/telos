//! User profile, authentication state, and launcher settings persistence.

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Persisted profile and authentication settings for Telos Launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherProfile {
    pub logged_in: bool,
    pub username: String,
    pub account_type: String,
    pub selected_installation_id: String,
    pub language: String,
    pub auto_close_on_play: bool,
    pub game_dir: Option<PathBuf>,
}

impl Default for LauncherProfile {
    fn default() -> Self {
        Self {
            logged_in: false,
            username: "Larvance".to_string(),
            account_type: "Telos Account".to_string(),
            selected_installation_id: "local-dev".to_string(),
            language: "English - United States".to_string(),
            auto_close_on_play: false,
            game_dir: None,
        }
    }
}

/// Returns the configuration file path for the launcher profile.
pub fn get_profile_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("telos").join("launcher_profile.json")
    } else {
        PathBuf::from(".telos_launcher_profile.json")
    }
}

/// Loads the profile from disk or returns default.
pub fn load_profile() -> LauncherProfile {
    let path = get_profile_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(profile) = serde_json::from_str::<LauncherProfile>(&data) {
            return profile;
        }
    }
    LauncherProfile::default()
}

/// Saves the profile to disk.
pub fn save_profile(profile: &LauncherProfile) -> anyhow::Result<()> {
    let path = get_profile_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(profile)?;
    fs::write(path, data)?;
    Ok(())
}
