//! Telos Game Launcher & Version Manager (Tauri 2 Edition).
//!
//! A rich desktop launcher providing authentication, repository release/snapshot fetching,
//! local development build coordination, installations management, and multiplayer server lists.

mod build_coordinator;
mod profile;
mod version_manager;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Window};

use build_coordinator::{resolve_repo_dir, start_build_worker, BuildEvent};
use profile::{load_profile, save_profile as persist_profile, LauncherProfile};
use version_manager::{
    fetch_remote_versions, get_default_installations, ChannelKind, Installation,
    RemoteVersionInfo,
};

/// Persisted multiplayer server entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    /// Unique identifier for the server entry.
    pub id: String,
    /// User-visible server name.
    pub name: String,
    /// Host address and port.
    pub address: String,
    /// Group category for collapsible sections.
    pub group: String,
    /// Ordering index within the group.
    pub order: usize,
    /// Last measured ping latency in milliseconds.
    pub ping_ms: Option<u32>,
    /// Whether the server was reachable on last check.
    pub online: bool,
}

impl Default for ServerEntry {
    fn default() -> Self {
        Self {
            id: "default-1".into(),
            name: "Official Telos Alpha Server".into(),
            address: "play.telos.voxel:25565".into(),
            group: "Official Servers".into(),
            order: 0,
            ping_ms: Some(24),
            online: true,
        }
    }
}

/// Path to store multiplayer servers.
fn get_servers_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("telos").join("servers.json")
    } else {
        PathBuf::from(".telos_servers.json")
    }
}

/// Loads multiplayer servers or returns default list.
fn load_servers() -> Vec<ServerEntry> {
    let path = get_servers_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(servers) = serde_json::from_str::<Vec<ServerEntry>>(&data) {
            return servers;
        }
    }
    vec![
        ServerEntry {
            id: "official-1".into(),
            name: "Official Telos Alpha Server".into(),
            address: "play.telos.voxel:25565".into(),
            group: "Official Servers".into(),
            order: 0,
            ping_ms: Some(22),
            online: true,
        },
        ServerEntry {
            id: "official-2".into(),
            name: "Survival Test Cluster".into(),
            address: "survival.telos.voxel:25565".into(),
            group: "Official Servers".into(),
            order: 1,
            ping_ms: Some(38),
            online: true,
        },
        ServerEntry {
            id: "local-1".into(),
            name: "Local Dev Test Server".into(),
            address: "127.0.0.1:25565".into(),
            group: "LAN / Development".into(),
            order: 2,
            ping_ms: Some(1),
            online: true,
        },
        ServerEntry {
            id: "community-1".into(),
            name: "Larvance Creative Sandbox".into(),
            address: "creative.larvance.net:25565".into(),
            group: "Community Servers".into(),
            order: 3,
            ping_ms: Some(45),
            online: true,
        },
    ]
}

/// Persists multiplayer servers.
fn persist_servers(servers: &[ServerEntry]) -> Result<(), String> {
    let path = get_servers_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(servers).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

/// Launch response result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Human-readable status message.
    pub message: String,
    /// Whether a background build process is currently running.
    pub is_building: bool,
}

/// Updater status summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterInfo {
    /// Currently installed version string.
    pub current_version: String,
    /// Latest available version string.
    pub latest_version: String,
    /// Whether a newer version is available.
    pub update_available: bool,
    /// Release notes for the new version.
    pub release_notes: String,
}

/// Managed application state shared across Tauri commands.
pub struct AppState {
    /// Repository root directory.
    pub repo_dir: PathBuf,
    /// Cached launcher profile.
    pub profile: Mutex<LauncherProfile>,
    /// Configured multiplayer servers list.
    pub servers: Mutex<Vec<ServerEntry>>,
    /// Configured game installation profiles.
    pub installations: Mutex<Vec<Installation>>,
    /// Remote release information.
    pub remote_versions: RemoteVersionInfo,
    /// Flag indicating if background build is in progress.
    pub is_building: Arc<AtomicBool>,
}

#[tauri::command]
fn get_profile(state: State<'_, AppState>) -> LauncherProfile {
    state.profile.lock().unwrap().clone()
}

#[tauri::command]
fn save_profile(state: State<'_, AppState>, profile: LauncherProfile) -> Result<(), String> {
    persist_profile(&profile).map_err(|e| e.to_string())?;
    *state.profile.lock().unwrap() = profile;
    Ok(())
}

#[tauri::command]
fn login(state: State<'_, AppState>, username: String) -> Result<LauncherProfile, String> {
    let mut profile = state.profile.lock().unwrap();
    profile.logged_in = true;
    if !username.trim().is_empty() {
        profile.username = username.trim().to_string();
    }
    persist_profile(&profile).map_err(|e| e.to_string())?;
    Ok(profile.clone())
}

#[tauri::command]
fn logout(state: State<'_, AppState>) -> Result<LauncherProfile, String> {
    let mut profile = state.profile.lock().unwrap();
    profile.logged_in = false;
    persist_profile(&profile).map_err(|e| e.to_string())?;
    Ok(profile.clone())
}

#[tauri::command]
fn get_installations(state: State<'_, AppState>) -> Vec<Installation> {
    state.installations.lock().unwrap().clone()
}

#[tauri::command]
fn save_installations(
    state: State<'_, AppState>,
    installations: Vec<Installation>,
) -> Result<(), String> {
    *state.installations.lock().unwrap() = installations;
    Ok(())
}

#[tauri::command]
fn get_remote_versions(state: State<'_, AppState>) -> RemoteVersionInfo {
    state.remote_versions.clone()
}

#[tauri::command]
fn get_servers(state: State<'_, AppState>) -> Vec<ServerEntry> {
    state.servers.lock().unwrap().clone()
}

#[tauri::command]
fn save_servers(state: State<'_, AppState>, servers: Vec<ServerEntry>) -> Result<(), String> {
    persist_servers(&servers)?;
    *state.servers.lock().unwrap() = servers;
    Ok(())
}

#[tauri::command]
fn check_updater_status(state: State<'_, AppState>) -> UpdaterInfo {
    let current = "v0.1.0".to_string();
    let latest = state.remote_versions.latest_release_tag.clone();
    let update_available = current != latest;
    UpdaterInfo {
        current_version: current,
        latest_version: latest,
        update_available,
        release_notes: state.remote_versions.latest_commit_msg.clone(),
    }
}

#[tauri::command]
fn launch_updater(state: State<'_, AppState>) -> Result<(), String> {
    let updater_bin = state.repo_dir.join("target/debug/telos-updater");
    if updater_bin.exists() {
        Command::new(&updater_bin)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        // Fallback: spawn cargo run -p telos-launcher --bin telos-updater
        Command::new("cargo")
            .args(["run", "-p", "telos-launcher", "--bin", "telos-updater"])
            .current_dir(&state.repo_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
fn trigger_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    installation_id: String,
) -> Result<LaunchResult, String> {
    let installations = state.installations.lock().unwrap().clone();
    let selected_inst = installations
        .iter()
        .find(|i| i.id == installation_id)
        .cloned();

    let is_dev = selected_inst
        .as_ref()
        .map_or(false, |i| i.channel == ChannelKind::LocalDev);

    let profile = state.profile.lock().unwrap().clone();
    let repo_dir = state.repo_dir.clone();
    let is_building_flag = state.is_building.clone();

    if is_dev {
        if is_building_flag.load(Ordering::SeqCst) {
            return Ok(LaunchResult {
                success: true,
                message: "Build is already in progress...".into(),
                is_building: true,
            });
        }

        is_building_flag.store(true, Ordering::SeqCst);
        let (tx, rx) = channel::<BuildEvent>();

        let is_debug = std::env::var("TELOS_DEBUG").map_or(true, |v| v == "1");
        start_build_worker(repo_dir.clone(), is_debug, tx, SystemTime::now());

        // Spawn listener thread that relays BuildEvents to Tauri frontend
        let app_handle = app.clone();
        let repo_dir_clone = repo_dir.clone();
        let auto_close = profile.auto_close_on_play;
        let is_building_for_thread = is_building_flag.clone();

        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let _ = app_handle.emit("build_event", &event);
                match &event {
                    BuildEvent::Success { .. } => {
                        is_building_for_thread.store(false, Ordering::SeqCst);
                        // Launch client binary
                        let bin_subpath = if is_debug {
                            "target/debug/telos"
                        } else {
                            "target/release/telos"
                        };
                        let target_bin = repo_dir_clone.join(bin_subpath);
                        if target_bin.exists() {
                            let _ = Command::new(&target_bin)
                                .current_dir(&repo_dir_clone)
                                .spawn();
                            if auto_close {
                                if let Some(win) = app_handle.get_webview_window("main") {
                                    let _ = win.close();
                                }
                            }
                        }
                        break;
                    }
                    BuildEvent::Failed(_) => {
                        is_building_for_thread.store(false, Ordering::SeqCst);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(LaunchResult {
            success: true,
            message: "Starting compilation worker...".into(),
            is_building: true,
        })
    } else {
        // Release channel
        let release_bin = repo_dir.join("target/release/telos");
        if release_bin.exists() {
            let _ = Command::new(&release_bin)
                .current_dir(&repo_dir)
                .spawn()
                .map_err(|e| e.to_string())?;

            if profile.auto_close_on_play {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.close();
                }
            }
            Ok(LaunchResult {
                success: true,
                message: "Launched release executable.".into(),
                is_building: false,
            })
        } else {
            // Build release first
            if is_building_flag.load(Ordering::SeqCst) {
                return Ok(LaunchResult {
                    success: true,
                    message: "Release build is currently in progress...".into(),
                    is_building: true,
                });
            }

            is_building_flag.store(true, Ordering::SeqCst);
            let (tx, rx) = channel::<BuildEvent>();
            start_build_worker(repo_dir.clone(), false, tx, SystemTime::now());

            let app_handle = app.clone();
            let repo_dir_clone = repo_dir.clone();
            let auto_close = profile.auto_close_on_play;
            let is_building_for_thread = is_building_flag.clone();

            std::thread::spawn(move || {
                while let Ok(event) = rx.recv() {
                    let _ = app_handle.emit("build_event", &event);
                    match &event {
                        BuildEvent::Success { .. } => {
                            is_building_for_thread.store(false, Ordering::SeqCst);
                            let target_bin = repo_dir_clone.join("target/release/telos");
                            if target_bin.exists() {
                                let _ = Command::new(&target_bin)
                                    .current_dir(&repo_dir_clone)
                                    .spawn();
                                if auto_close {
                                    if let Some(win) = app_handle.get_webview_window("main") {
                                        let _ = win.close();
                                    }
                                }
                            }
                            break;
                        }
                        BuildEvent::Failed(_) => {
                            is_building_for_thread.store(false, Ordering::SeqCst);
                            break;
                        }
                        _ => {}
                    }
                }
            });

            Ok(LaunchResult {
                success: true,
                message: "Compiling release binary before launch...".into(),
                is_building: true,
            })
        }
    }
}

// Window management commands
#[tauri::command]
fn window_minimize(window: Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(window: Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

fn main() {
    let repo_dir = resolve_repo_dir();
    let profile = load_profile();
    let remote_versions = fetch_remote_versions(&repo_dir);
    let installations = get_default_installations(&repo_dir, &remote_versions);
    let servers = load_servers();

    let app_state = AppState {
        repo_dir,
        profile: Mutex::new(profile),
        servers: Mutex::new(servers),
        installations: Mutex::new(installations),
        remote_versions,
        is_building: Arc::new(AtomicBool::new(false)),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_profile,
            save_profile,
            login,
            logout,
            get_installations,
            save_installations,
            get_remote_versions,
            get_servers,
            save_servers,
            check_updater_status,
            launch_updater,
            trigger_launch,
            window_minimize,
            window_toggle_maximize,
            window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Telos Launcher");
}
