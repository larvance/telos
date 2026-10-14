//! Telos Game Launcher & Version Manager (Tauri 2 Edition).
//!
//! A rich desktop launcher providing authentication, repository release/snapshot fetching,
//! local development build coordination, installations management, and multiplayer server lists.

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::collapsible_if,
    clippy::implicit_clone,
    clippy::cast_possible_wrap
)]

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
    ensure_version_cached, fetch_remote_versions, get_telos_base_dir,
    load_or_init_installations, persist_installations, ChannelKind, Installation,
    RemoteVersionInfo,
};

/// Persisted multiplayer server entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    /// Unique identifier for the server entry.
    pub id: String,
    /// User-visible server name or group title.
    pub name: String,
    /// Host address and port (empty for group containers).
    #[serde(default)]
    pub address: String,
    /// Whether this entry represents a group container.
    #[serde(default)]
    pub is_group: bool,
    /// Parent group ID if placed inside a group.
    #[serde(default)]
    pub group_id: Option<String>,
    /// Legacy group field for backwards compatibility.
    #[serde(default)]
    pub group: String,
    /// Ordering index within the server list.
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
            name: "Telos Official Survival".into(),
            address: "play.telos.voxel:25565".into(),
            is_group: false,
            group_id: None,
            group: String::new(),
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
            name: "Telos Official Survival".into(),
            address: "play.telos.voxel:25565".into(),
            is_group: false,
            group_id: None,
            group: String::new(),
            order: 0,
            ping_ms: Some(22),
            online: true,
        },
        ServerEntry {
            id: "local-1".into(),
            name: "Local Dev Test Server".into(),
            address: "127.0.0.1:25565".into(),
            is_group: false,
            group_id: None,
            group: String::new(),
            order: 1,
            ping_ms: Some(1),
            online: true,
        },
        ServerEntry {
            id: "community-1".into(),
            name: "Larvance Creative Sandbox".into(),
            address: "creative.larvance.net:25565".into(),
            is_group: false,
            group_id: None,
            group: String::new(),
            order: 2,
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
    persist_installations(&installations)?;
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

/// Spawns a game process, isolates its execution, and monitors for crashes or clean termination.
fn spawn_and_supervise(
    mut cmd: Command,
    app_handle: AppHandle,
    inst_name: String,
    auto_close: bool,
) -> Result<(), String> {
    if auto_close {
        if let Some(win) = app_handle.get_webview_window("main") {
            let _ = win.hide();
        }
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn game process: {e}"))?;

    std::thread::spawn(move || {
        let stderr_reader = child.stderr.take();
        let stdout_reader = child.stdout.take();

        let err_handle = std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut r) = stderr_reader {
                use std::io::Read;
                let _ = r.read_to_string(&mut buf);
            }
            buf
        });

        let out_handle = std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut r) = stdout_reader {
                use std::io::Read;
                let _ = r.read_to_string(&mut buf);
            }
            buf
        });

        let status = child.wait();
        let stderr_output = err_handle.join().unwrap_or_default();
        let stdout_output = out_handle.join().unwrap_or_default();

        // Always show the launcher back when game exits or crashes
        if let Some(win) = app_handle.get_webview_window("main") {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }

        match status {
            Ok(exit_status) if exit_status.success() => {
                let _ = app_handle.emit("game_exited", serde_json::json!({
                    "success": true,
                    "installation": inst_name,
                    "exit_code": 0,
                }));
            }
            Ok(exit_status) => {
                let code = exit_status.code().unwrap_or(-1);
                let mut combined = String::new();
                if !stderr_output.trim().is_empty() {
                    combined.push_str("STDERR:\n");
                    combined.push_str(stderr_output.trim());
                }
                if !stdout_output.trim().is_empty() {
                    if !combined.is_empty() {
                        combined.push_str("\n\nSTDOUT:\n");
                    } else {
                        combined.push_str("STDOUT:\n");
                    }
                    combined.push_str(stdout_output.trim());
                }
                if combined.is_empty() {
                    combined = format!("Game process terminated unexpectedly with exit code {code}.");
                }

                let _ = app_handle.emit("game_crash", serde_json::json!({
                    "success": false,
                    "installation": inst_name,
                    "exit_code": code,
                    "error_log": combined,
                }));
            }
            Err(e) => {
                let _ = app_handle.emit("game_crash", serde_json::json!({
                    "success": false,
                    "installation": inst_name,
                    "exit_code": -1,
                    "error_log": format!("Process supervision failed: {e}"),
                }));
            }
        }
    });

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
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
        .map_or(true, |i| i.channel == ChannelKind::LocalDev);

    let profile = state.profile.lock().unwrap().clone();
    let repo_dir = state.repo_dir.clone();
    let is_building_flag = state.is_building.clone();

    // Assemble launch arguments and environment overrides from the selected installation
    let mut cmd_args: Vec<String> = Vec::new();
    let mut max_memory: Option<u32> = None;

    if let Some(ref inst) = selected_inst {
        if let Some(ref dir) = inst.custom_dir {
            cmd_args.push("--data-dir".into());
            cmd_args.push(dir.to_string_lossy().into());
        }
        if let Some(mb) = inst.max_memory_mb {
            cmd_args.push("--max-memory-mb".into());
            cmd_args.push(mb.to_string());
            max_memory = Some(mb);
        }
        if let Some(ref res) = inst.resolution {
            if res != "Auto" && res.contains('x') {
                let parts: Vec<&str> = res.split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        cmd_args.push("--width".into());
                        cmd_args.push(w.to_string());
                        cmd_args.push("--height".into());
                        cmd_args.push(h.to_string());
                    }
                }
            }
        }
        if let Some(ref extra) = inst.custom_args {
            for arg in extra.split_whitespace() {
                cmd_args.push(arg.to_string());
            }
        }
    }

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

        let is_debug = std::env::var("TELOS_DEBUG").map_or(false, |v| v == "1");
        start_build_worker(repo_dir.clone(), is_debug, tx, SystemTime::now());

        // Spawn listener thread that relays BuildEvents to Tauri frontend
        let app_handle = app.clone();
        let repo_dir_clone = repo_dir.clone();
        let auto_close = profile.auto_close_on_play;
        let is_building_for_thread = is_building_flag.clone();
        let cmd_args_dev = cmd_args.clone();
        let max_memory_dev = max_memory;

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
                            let mut cmd = Command::new(&target_bin);
                            cmd.args(&cmd_args_dev);
                            cmd.current_dir(&repo_dir_clone);
                            if let Some(mb) = max_memory_dev {
                                cmd.env("TELOS_MAX_MEMORY_MB", mb.to_string());
                            }
                            let _ = spawn_and_supervise(
                                cmd,
                                app_handle.clone(),
                                "Dev Environment (Local Build)".to_string(),
                                auto_close,
                            );
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
        // Cached Release / Snapshot / Custom installation
        let version_tag = selected_inst
            .as_ref()
            .map_or("0.1.0", |i| i.version_tag.as_str());

        // Resolve binary from ~/.telos/versions/<version_tag>/ or fallback to target/release/telos
        let bin_path = ensure_version_cached(&repo_dir, version_tag)
            .or_else(|_| {
                let rel = repo_dir.join("target/release/telos");
                if rel.exists() {
                    Ok(rel)
                } else {
                    Err(format!("Binary for version '{version_tag}' not found in cache."))
                }
            })
            .map_err(|e| e.to_string())?;

        let working_dir = selected_inst
            .as_ref()
            .and_then(|i| i.custom_dir.clone())
            .unwrap_or_else(get_telos_base_dir);
        let _ = std::fs::create_dir_all(&working_dir);

        let mut cmd = Command::new(&bin_path);
        cmd.args(&cmd_args);
        cmd.current_dir(&working_dir);
        if let Some(mb) = max_memory {
            cmd.env("TELOS_MAX_MEMORY_MB", mb.to_string());
        }

        let inst_name = selected_inst
            .as_ref()
            .map_or("Installation", |i| i.name.as_str())
            .to_string();

        spawn_and_supervise(cmd, app.clone(), inst_name.clone(), profile.auto_close_on_play)?;

        Ok(LaunchResult {
            success: true,
            message: format!("Launched {inst_name} ({version_tag})"),
            is_building: false,
        })
    }
}

// Window management commands
#[tauri::command]
fn window_minimize(window: Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: Window) -> Result<bool, String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
fn window_drag_start(window: Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        let cursor = window.cursor_position().ok();
        window.unmaximize().map_err(|e| e.to_string())?;
        if let (Some(cursor), Ok(size)) = (cursor, window.outer_size()) {
            let target_x = (cursor.x as i32) - (size.width as i32 / 2);
            let target_y = (cursor.y as i32) - 18;
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                target_x.max(0),
                target_y.max(0),
            )));
        }
    }
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
fn is_window_maximized(window: Window) -> Result<bool, String> {
    window.is_maximized().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_close(window: Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cli_args() -> Result<Vec<String>, String> {
    Ok(std::env::args().collect())
}

/// Synchronizes system desktop entries:
/// - Ensures the raw game ("Telos") does not appear as a standalone app in the system app list.
/// - If local development is set up (repo with Cargo.toml), installs "Telos Dev" (`telos-dev.desktop`)
///   which immediately opens the local game binary.
/// - If dev is not set up, ensures "Telos Dev" is not present in the app list.
fn sync_desktop_integration(repo_dir: &std::path::Path) {
    let Some(data_dir) = dirs::data_dir() else { return; };
    let apps_dir = data_dir.join("applications");
    if !apps_dir.exists() {
        let _ = fs::create_dir_all(&apps_dir);
    }

    // 1. Never show raw "Telos" standalone in the desktop app list
    let old_telos_desktop = apps_dir.join("telos.desktop");
    if old_telos_desktop.exists() {
        let _ = fs::remove_file(&old_telos_desktop);
    }

    // 2. Manage "Telos Launcher" and "Telos Dev" desktop integrations
    let is_dev_setup = repo_dir.join("Cargo.toml").exists();
    let dev_desktop = apps_dir.join("telos-dev.desktop");
    let launcher_desktop = apps_dir.join("telos-launcher.desktop");

    if let Some(home) = dirs::home_dir() {
        let bin_dir = home.join(".local/bin");
        let _ = fs::create_dir_all(&bin_dir);

        if is_dev_setup {
            // Launcher auto-build wrapper symlink
            let launcher_script = repo_dir.join("scripts/telos-launcher.sh");
            let launcher_symlink = bin_dir.join("telos-launcher");
            if launcher_script.exists() {
                if launcher_symlink.is_symlink() || launcher_symlink.exists() {
                    let _ = fs::remove_file(&launcher_symlink);
                }
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&launcher_script, &launcher_symlink);
            }

            // Dev direct game wrapper symlink
            let dev_script = repo_dir.join("scripts/telos-dev.sh");
            let dev_symlink = bin_dir.join("telos-dev");
            if dev_script.exists() {
                if dev_symlink.is_symlink() || dev_symlink.exists() {
                    let _ = fs::remove_file(&dev_symlink);
                }
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(&dev_script, &dev_symlink);
            }
        }
    }

    // Configure Telos Launcher desktop entry (points to dev auto-building wrapper if in dev repo)
    let launcher_exec = if is_dev_setup {
        dirs::home_dir()
            .map(|h| h.join(".local/bin/telos-launcher").to_string_lossy().to_string())
            .unwrap_or_else(|| "telos-launcher".into())
    } else {
        "telos-launcher".to_string()
    };

    let launcher_desktop_content = format!(
        "[Desktop Entry]\n\
        Name=Telos Launcher\n\
        GenericName=Game Launcher\n\
        Comment=Launcher, version manager, and profile manager for Telos Voxel Engine\n\
        Exec={} %U\n\
        Icon=telos-launcher\n\
        Path={}\n\
        Terminal=false\n\
        Type=Application\n\
        Categories=Game;Simulation;\n\
        Keywords=telos;voxel;launcher;game;manager;\n\
        StartupNotify=true\n\
        StartupWMClass=telos-launcher\n",
        launcher_exec,
        repo_dir.to_string_lossy()
    );
    let _ = fs::write(&launcher_desktop, launcher_desktop_content);

    // 3. Manage "Telos Dev"
    if is_dev_setup {
        let dev_exec = dirs::home_dir()
            .map(|h| h.join(".local/bin/telos-dev").to_string_lossy().to_string())
            .unwrap_or_else(|| "telos-dev".into());

        let icon_path = dirs::data_dir()
            .map(|d| d.join("icons/hicolor/256x256/apps/telos.png").to_string_lossy().to_string())
            .unwrap_or_else(|| "telos".into());

        let dev_desktop_content = format!(
            "[Desktop Entry]\n\
            Name=Telos Dev\n\
            GenericName=Voxel Game (Dev)\n\
            Comment=Telos Voxel Engine - Local Development Build\n\
            Exec={} %U\n\
            Icon={}\n\
            Path={}\n\
            Terminal=false\n\
            Type=Application\n\
            Categories=Game;\n\
            Keywords=telos;voxel;dev;game;sandbox;3d;vulkan;\n\
            StartupNotify=true\n\
            StartupWMClass=telos\n",
            dev_exec,
            icon_path,
            repo_dir.to_string_lossy()
        );
        let _ = fs::write(&dev_desktop, dev_desktop_content);
    } else if dev_desktop.exists() {
        let _ = fs::remove_file(&dev_desktop);
    }

    // Update system desktop database if tool is installed
    let _ = Command::new("update-desktop-database").arg(&apps_dir).status();
}

fn main() {
    let repo_dir = resolve_repo_dir();
    sync_desktop_integration(&repo_dir);

    let profile = load_profile();
    let remote_versions = fetch_remote_versions(&repo_dir);
    let installations = load_or_init_installations(&repo_dir, &remote_versions);
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
        .setup(|app| {
            if let Some(win) = app.get_webview_window("main") {
                let icon = tauri::include_image!("icons/128x128@2x.png");
                let _ = win.set_icon(icon);
            }
            Ok(())
        })
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
            window_drag_start,
            is_window_maximized,
            window_close,
            get_cli_args,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Telos Launcher");
}
