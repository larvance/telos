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
    #[serde(default)]
    pub news_url: Option<String>,
    #[serde(default)]
    pub graphics_backend: Option<String>,
    #[serde(default)]
    pub ram_gb: Option<u32>,
    #[serde(default)]
    pub last_used_installation: Option<String>,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub successful_news_urls: Vec<String>,
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
            news_url: Some("https://news.telos.larvance.com/news.json".to_string()),
            graphics_backend: Some("vulkan".to_string()),
            ram_gb: Some(8),
            last_used_installation: Some("local-dev".to_string()),
            auth_token: None,
            successful_news_urls: vec![
                "https://news.telos.larvance.com/news.json".to_string(),
                "https://raw.githubusercontent.com/larvance/telos/main/apps/telos-launcher/ui/assets/news/news.json".to_string(),
            ],
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
        if let Ok(mut profile) = serde_json::from_str::<LauncherProfile>(&data) {
            let old_default = "https://raw.githubusercontent.com/larvance/telos/main/apps/telos-launcher/ui/assets/news/news.json";
            let new_default = "https://news.telos.larvance.com/news.json";
            if profile.news_url.as_deref() == Some(old_default) {
                profile.news_url = Some(new_default.to_string());
            }
            if profile.successful_news_urls.is_empty() {
                profile.successful_news_urls = vec![
                    new_default.to_string(),
                    old_default.to_string(),
                ];
            } else if !profile.successful_news_urls.contains(&new_default.to_string()) {
                profile.successful_news_urls.insert(0, new_default.to_string());
            }
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
