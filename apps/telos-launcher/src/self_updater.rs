//! Telos Launcher Self-Update Coordinator.
//!
//! Provides background update checking, non-blocking background downloads to a staged
//! directory (`~/.telos/updater/`), and atomic, seamless replacement upon next launch.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Staged launcher update metadata persisted to `~/.telos/updater/pending_update.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedUpdateInfo {
    /// New version tag string (e.g. "v0.1.1").
    pub version: String,
    /// Release title or description.
    pub release_name: String,
    /// Absolute path to the downloaded staged executable.
    pub staged_binary_path: PathBuf,
    /// Target binary path to be replaced on the next launch.
    pub target_binary_path: PathBuf,
    /// Unix timestamp when the download completed.
    pub download_timestamp: u64,
    /// Whether the staged update binary is verified and ready for installation.
    pub ready: bool,
}

/// Comprehensive update status shared with UI and telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherUpdateInfo {
    /// Currently installed version string.
    pub current_version: String,
    /// Latest available version string.
    pub latest_version: String,
    /// Human-readable release name.
    pub release_name: String,
    /// Whether a newer version is available.
    pub update_available: bool,
    /// Whether a background download is currently active.
    pub is_downloading: bool,
    /// Whether a staged update is downloaded and waiting to install.
    pub is_ready_to_install: bool,
    /// Background download progress percentage (0 to 100).
    pub download_progress: u32,
    /// Summary of release notes.
    pub release_notes: String,
}

impl Default for LauncherUpdateInfo {
    fn default() -> Self {
        let cur = format!("v{}", env!("CARGO_PKG_VERSION"));
        Self {
            current_version: cur.clone(),
            latest_version: cur,
            release_name: "Telos Launcher".into(),
            update_available: false,
            is_downloading: false,
            is_ready_to_install: false,
            download_progress: 0,
            release_notes: "Telos Launcher is up to date.".into(),
        }
    }
}

/// Returns the staging directory for updater downloads: `~/.telos/updater/`.
pub fn get_updater_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".telos").join("updater")
    } else {
        PathBuf::from(".telos").join("updater")
    }
}

/// Returns the path to the pending update metadata file: `~/.telos/updater/pending_update.json`.
pub fn get_pending_update_path() -> PathBuf {
    get_updater_dir().join("pending_update.json")
}

/// Returns the default path for the staged binary: `~/.telos/updater/staged_launcher`.
pub fn get_staged_binary_path() -> PathBuf {
    get_updater_dir().join("staged_launcher")
}

/// Reads any pending update info from disk, verifying the staged binary exists and is valid.
pub fn read_pending_update() -> Option<StagedUpdateInfo> {
    let path = get_pending_update_path();
    if !path.exists() {
        return None;
    }

    let data = fs::read_to_string(&path).ok()?;
    let info = serde_json::from_str::<StagedUpdateInfo>(&data).ok()?;

    if !info.ready {
        return None;
    }

    // Verify the staged executable actually exists on disk and is non-empty
    if let Ok(meta) = fs::metadata(&info.staged_binary_path) {
        if meta.len() > 0 {
            return Some(info);
        }
    }

    // If metadata exists but staged file was deleted or empty, clean up stale state
    let _ = fs::remove_file(&path);
    None
}

/// Returns true if a verified staged update is downloaded and ready to be installed.
pub fn has_pending_update() -> bool {
    read_pending_update().is_some()
}

/// Cleans up any staged binaries and pending update metadata.
pub fn clear_pending_update() {
    let pending_json = get_pending_update_path();
    let staged_bin = get_staged_binary_path();
    let _ = fs::remove_file(&pending_json);
    let _ = fs::remove_file(&staged_bin);
}

/// Determines the target executable path that should be updated.
pub fn determine_target_launcher_path(repo_dir: &Path) -> PathBuf {
    // 1. Explicit override via environment variable
    if let Ok(target) = std::env::var("TELOS_LAUNCHER_TARGET_BIN") {
        let p = PathBuf::from(target);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }

    // 2. Check currently running executable
    if let Ok(current_exe) = std::env::current_exe() {
        let file_name = current_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        // Never target shell scripts (.sh)
        if !file_name.ends_with(".sh") && file_name.contains("telos-launcher") {
            return current_exe;
        }
    }

    // 3. User local bin (~/.local/bin/telos-launcher)
    if let Some(home) = dirs::home_dir() {
        let local_bin = home.join(".local/bin/telos-launcher");
        if local_bin.is_file() {
            // If it's a symlink to a shell script, don't overwrite the script
            if let Ok(target) = fs::read_link(&local_bin) {
                if !target.to_string_lossy().ends_with(".sh") {
                    return local_bin;
                }
            } else {
                return local_bin;
            }
        }
    }

    // 4. Fallback to workspace release build if in development repository
    let rel_bin = repo_dir.join("target/release/telos-launcher");
    if rel_bin.exists() {
        return rel_bin;
    }

    // 5. Final fallback: standard executable path in home
    if let Some(home) = dirs::home_dir() {
        home.join(".local/bin/telos-launcher")
    } else {
        PathBuf::from("telos-launcher")
    }
}

/// Applies a staged update to the target binary location atomically.
pub fn apply_staged_update(info: &StagedUpdateInfo) -> Result<(), String> {
    if !info.staged_binary_path.exists() {
        return Err(format!(
            "Staged binary does not exist at {:?}",
            info.staged_binary_path
        ));
    }

    let target = &info.target_binary_path;
    let target_dir = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;

    // Copy to temporary file within the same directory first to ensure atomic rename
    // without crossing filesystem boundaries or hitting ETXTBSY on running binaries.
    let temp_target = target_dir.join(format!(".telos-launcher-new.{}", std::process::id()));

    fs::copy(&info.staged_binary_path, &temp_target)
        .map_err(|e| format!("Failed to copy staged binary to temp target: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&temp_target) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&temp_target, perms);
        }
    }

    // Atomic replacement of target binary
    fs::rename(&temp_target, target)
        .map_err(|e| format!("Failed to atomically rename updated binary to {target:?}: {e}"))?;

    // Clean up staging files
    clear_pending_update();
    Ok(())
}

/// Stages an existing binary file as a pending update (used for testing and local distribution).
pub fn stage_existing_binary(
    source_bin: &Path,
    target_bin: &Path,
    version: &str,
    release_name: &str,
) -> Result<StagedUpdateInfo, String> {
    let updater_dir = get_updater_dir();
    stage_existing_binary_to_dir(&updater_dir, source_bin, target_bin, version, release_name)
}

/// Stages an existing binary file to an explicit staging directory.
pub fn stage_existing_binary_to_dir(
    updater_dir: &Path,
    source_bin: &Path,
    target_bin: &Path,
    version: &str,
    release_name: &str,
) -> Result<StagedUpdateInfo, String> {
    fs::create_dir_all(updater_dir).map_err(|e| e.to_string())?;
    let staged_final = updater_dir.join("staged_launcher");

    fs::copy(source_bin, &staged_final)
        .map_err(|e| format!("Failed to copy source binary to staging: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&staged_final) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&staged_final, perms);
        }
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let info = StagedUpdateInfo {
        version: version.to_string(),
        release_name: release_name.to_string(),
        staged_binary_path: staged_final,
        target_binary_path: target_bin.to_path_buf(),
        download_timestamp: now,
        ready: true,
    };

    let info_json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
    fs::write(updater_dir.join("pending_update.json"), info_json).map_err(|e| e.to_string())?;

    Ok(info)
}

/// Locates the `telos-updater` executable across standard installation and dev locations.
pub fn find_updater_binary(repo_dir: &Path) -> Option<PathBuf> {
    // 1. Same directory as current executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join("telos-updater");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 2. Local dev repository release binary
    let repo_rel = repo_dir.join("target/release/telos-updater");
    if repo_rel.is_file() {
        return Some(repo_rel);
    }

    // 3. Local dev repository debug binary
    let repo_dbg = repo_dir.join("target/debug/telos-updater");
    if repo_dbg.is_file() {
        return Some(repo_dbg);
    }

    // 4. ~/.local/bin/telos-updater
    if let Some(home) = dirs::home_dir() {
        let local_updater = home.join(".local/bin/telos-updater");
        if local_updater.is_file() {
            return Some(local_updater);
        }
    }

    // 5. Check PATH via `which telos-updater`
    if let Ok(output) = Command::new("which").arg("telos-updater").output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(p);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Queries remote releases for the latest launcher version without blocking startup.
pub fn check_for_launcher_update(current_version: &str) -> LauncherUpdateInfo {
    let mut status = LauncherUpdateInfo {
        current_version: current_version.to_string(),
        latest_version: current_version.to_string(),
        release_name: "Telos Launcher".into(),
        update_available: false,
        is_downloading: false,
        is_ready_to_install: false,
        download_progress: 0,
        release_notes: "Telos Launcher is up to date.".into(),
    };

    // If an update is already staged, reflect ready status
    if let Some(pending) = read_pending_update() {
        status.latest_version = pending.version.clone();
        status.release_name = pending.release_name.clone();
        status.update_available = true;
        status.is_ready_to_install = true;
        status.download_progress = 100;
        status.release_notes = format!(
            "Update {} is ready to install on next launch.",
            pending.version
        );
        return status;
    }

    // Query endpoint: check custom URL or official GitHub releases
    let api_url = std::env::var("TELOS_UPDATE_URL")
        .unwrap_or_else(|_| "https://api.github.com/repos/larvance/telos/releases/latest".into());

    let curl_res = Command::new("curl")
        .args([
            "-s",
            "--connect-timeout",
            "3",
            "-H",
            "User-Agent: Telos-Launcher",
            &api_url,
        ])
        .output();

    if let Ok(output) = curl_res {
        if output.status.success() {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(tag) = val.get("tag_name").and_then(|t| t.as_str()) {
                    status.latest_version = tag.to_string();
                    if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                        status.release_name = name.to_string();
                    }
                    if let Some(body) = val.get("body").and_then(|b| b.as_str()) {
                        status.release_notes = body.to_string();
                    }

                    let cur_clean = current_version.trim_start_matches('v');
                    let tag_clean = tag.trim_start_matches('v');
                    if cur_clean != tag_clean {
                        status.update_available = true;
                    }
                }
            }
        }
    }

    status
}

/// Downloads and stages a remote launcher update in the background.
pub fn download_and_stage_update<F>(
    download_url: &str,
    version: &str,
    release_name: &str,
    target_bin: &Path,
    on_progress: F,
) -> Result<StagedUpdateInfo, String>
where
    F: Fn(u32) + Send + 'static,
{
    let updater_dir = get_updater_dir();
    fs::create_dir_all(&updater_dir).map_err(|e| e.to_string())?;

    let staged_tmp = updater_dir.join("staged_launcher.tmp");
    let staged_final = get_staged_binary_path();
    let _ = fs::remove_file(&staged_tmp);

    on_progress(15);

    // Download update payload via curl
    let status = Command::new("curl")
        .args([
            "-L",
            "--fail",
            "--connect-timeout",
            "8",
            "-o",
            staged_tmp.to_str().unwrap(),
            download_url,
        ])
        .status()
        .map_err(|e| format!("Failed to download update: {e}"))?;

    if !status.success() {
        let _ = fs::remove_file(&staged_tmp);
        return Err(format!(
            "Download exited with error status {:?}",
            status.code()
        ));
    }

    on_progress(60);

    // If download is a tarball (.tar.gz, .tar.zst, .tar.xz), extract the telos-launcher binary
    let is_archive = download_url.ends_with(".tar.gz")
        || download_url.ends_with(".tgz")
        || download_url.ends_with(".tar.zst")
        || download_url.ends_with(".tar.xz");

    if is_archive {
        let extract_dir = updater_dir.join("extract_tmp");
        let _ = fs::create_dir_all(&extract_dir);
        let tar_status = Command::new("tar")
            .args([
                "-xf",
                staged_tmp.to_str().unwrap(),
                "-C",
                extract_dir.to_str().unwrap(),
            ])
            .status();

        if tar_status.map_or(false, |s| s.success()) {
            let candidate_bin = extract_dir.join("telos-launcher");
            if candidate_bin.is_file() {
                let _ = fs::rename(&candidate_bin, &staged_tmp);
            }
        }
        let _ = fs::remove_dir_all(&extract_dir);
    }

    on_progress(85);

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&staged_tmp) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&staged_tmp, perms);
        }
    }

    // Verify binary is non-empty
    let len = fs::metadata(&staged_tmp).map(|m| m.len()).unwrap_or(0);
    if len < 512 {
        let _ = fs::remove_file(&staged_tmp);
        return Err("Downloaded update file is invalid or too small.".into());
    }

    // Atomic move to staged location
    fs::rename(&staged_tmp, &staged_final)
        .map_err(|e| format!("Failed to move staged binary: {e}"))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let info = StagedUpdateInfo {
        version: version.to_string(),
        release_name: release_name.to_string(),
        staged_binary_path: staged_final,
        target_binary_path: target_bin.to_path_buf(),
        download_timestamp: now,
        ready: true,
    };

    let info_json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
    fs::write(get_pending_update_path(), info_json).map_err(|e| e.to_string())?;

    on_progress(100);
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_staged_update_serialization() {
        let info = StagedUpdateInfo {
            version: "v0.1.1".into(),
            release_name: "Telos Foundation v0.1.1".into(),
            staged_binary_path: PathBuf::from("/tmp/staged_launcher"),
            target_binary_path: PathBuf::from("/usr/bin/telos-launcher"),
            download_timestamp: 1729000000,
            ready: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: StagedUpdateInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    #[test]
    fn test_apply_staged_update_workflow() {
        let temp = tempdir().unwrap();
        let staged_bin = temp.path().join("staged_launcher");
        let target_bin = temp.path().join("telos-launcher");

        // Create mock staged binary
        fs::write(&staged_bin, b"#!/bin/sh\necho updated\n").unwrap();
        // Create initial target binary
        fs::write(&target_bin, b"#!/bin/sh\necho original\n").unwrap();

        let info = StagedUpdateInfo {
            version: "v0.1.1".into(),
            release_name: "Test Update".into(),
            staged_binary_path: staged_bin,
            target_binary_path: target_bin.clone(),
            download_timestamp: 100,
            ready: true,
        };

        let res = apply_staged_update(&info);
        assert!(res.is_ok());

        let target_content = fs::read(&target_bin).unwrap();
        assert_eq!(target_content, b"#!/bin/sh\necho updated\n");
    }

    #[test]
    fn test_stage_existing_binary_workflow() {
        let temp = tempdir().unwrap();
        let updater_dir = temp.path().join("updater");
        let source_bin = temp.path().join("source_launcher");
        let target_bin = temp.path().join("target_launcher");

        fs::write(&source_bin, b"binary_data_here_12345").unwrap();

        // Staging creates metadata and moves staged copy to isolated temp updater dir
        let info = stage_existing_binary_to_dir(
            &updater_dir,
            &source_bin,
            &target_bin,
            "v0.1.2",
            "Test 0.1.2",
        );
        assert!(info.is_ok());
        let info = info.unwrap();

        assert_eq!(info.version, "v0.1.2");
        assert!(info.ready);
        assert!(info.staged_binary_path.exists());
        assert_eq!(info.target_binary_path, target_bin);

        // Verify JSON was written
        let pending_json = updater_dir.join("pending_update.json");
        assert!(pending_json.exists());
        let data = fs::read_to_string(&pending_json).unwrap();
        let read_info: StagedUpdateInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(read_info.version, "v0.1.2");
    }

    #[test]
    fn test_target_launcher_path_resolution() {
        let temp = tempdir().unwrap();
        let repo_dir = temp.path();

        let target = determine_target_launcher_path(repo_dir);
        assert!(!target.as_os_str().is_empty());
        // Verify it doesn't end with a shell script
        assert!(!target.to_string_lossy().ends_with(".sh"));
    }
}
