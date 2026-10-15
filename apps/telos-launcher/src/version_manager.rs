//! Repository release fetcher, installation registry, and version checker.
//!
//! Queries repository releases and commits (remote via GitHub API/curl or local via git),
//! detects local development workspaces, and tracks installed/configured launch profiles.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Version channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    /// Latest stable release version.
    LatestRelease,
    /// Latest snapshot / dev build (commit hash).
    Snapshot,
    /// Local workspace development build (builds from source).
    LocalDev,
    /// Custom user installation.
    Custom,
}

/// A configured installation profile selectable in the launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    pub id: String,
    pub name: String,
    #[serde(default = "default_channel_kind")]
    pub channel: ChannelKind,
    #[serde(default)]
    pub version_tag: String,
    #[serde(default)]
    pub custom_dir: Option<PathBuf>,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub custom_args: Option<String>,
    #[serde(default)]
    pub is_dev: bool,
    #[serde(default)]
    pub icon_type: Option<String>,
}

fn default_channel_kind() -> ChannelKind {
    ChannelKind::Custom
}

impl Installation {
    pub fn new_release(version: impl Into<String>) -> Self {
        let ver = version.into();
        Self {
            id: "latest-release".into(),
            name: "Latest release".into(),
            channel: ChannelKind::LatestRelease,
            version_tag: ver,
            custom_dir: None,
            max_memory_mb: Some(4096),
            resolution: Some("Auto".into()),
            custom_args: None,
            is_dev: false,
            icon_type: Some("voxel".into()),
        }
    }

    pub fn new_snapshot(commit: impl Into<String>) -> Self {
        let commit_str = commit.into();
        Self {
            id: "latest-snapshot".into(),
            name: "Latest snapshot".into(),
            channel: ChannelKind::Snapshot,
            version_tag: commit_str,
            custom_dir: None,
            max_memory_mb: Some(4096),
            resolution: Some("Auto".into()),
            custom_args: None,
            is_dev: false,
            icon_type: Some("package".into()),
        }
    }

    pub fn new_local_dev(path: PathBuf, commit: &str) -> Self {
        Self {
            id: "local-dev".into(),
            name: "Dev Environment (Local Build)".into(),
            channel: ChannelKind::LocalDev,
            version_tag: format!("local @ {commit}"),
            custom_dir: Some(path),
            max_memory_mb: Some(8192),
            resolution: Some("Auto".into()),
            custom_args: None,
            is_dev: true,
            icon_type: Some("wrench".into()),
        }
    }
}

/// Remote release and repository state fetched from GitHub or local git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteVersionInfo {
    pub latest_release_tag: String,
    pub latest_release_name: String,
    pub latest_commit_hash: String,
    pub latest_commit_msg: String,
    pub last_checked: String,
    pub patch_notes: Vec<PatchNoteEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchNoteEntry {
    pub title: String,
    pub version: String,
    pub date: String,
    pub summary: String,
}

impl Default for RemoteVersionInfo {
    fn default() -> Self {
        Self {
            latest_release_tag: "v0.1.0".into(),
            latest_release_name: "Telos Foundation Release".into(),
            latest_commit_hash: "3326973".into(),
            latest_commit_msg: "feat(ui): add main menu logo banner, button hover animations, and multiplayer server groups".into(),
            last_checked: "Just now".into(),
            patch_notes: vec![
                PatchNoteEntry {
                    title: "Wilderness Bound & Multiplayer Overhaul".into(),
                    version: "v0.1.0".into(),
                    date: "October 2026".into(),
                    summary: "High-performance Vulkan 1.3 renderer, 3D cubic chunk architecture, multi-noise world generation, safe spawn solver, and collapsible server browser with drag reordering.".into(),
                },
                PatchNoteEntry {
                    title: "Vulkan 1.3 & Texture Arrays".into(),
                    version: "v0.0.9".into(),
                    date: "October 2026".into(),
                    summary: "Bindless texture arrays, buffer device addresses, and seamless 360-degree panorama background cubemaps.".into(),
                },
                PatchNoteEntry {
                    title: "Procedural Caves & Aquifers".into(),
                    version: "v0.0.8".into(),
                    date: "October 2026".into(),
                    summary: "3D coarse density trilinear interpolation, cheese caves, 3D Voronoi aquifers, and multi-biome Nether generation.".into(),
                },
            ],
        }
    }
}

/// Detects if current working environment is inside a local Telos dev repository.
pub fn detect_local_dev_repo(repo_dir: &Path) -> bool {
    repo_dir.join("Cargo.toml").exists() && repo_dir.join("crates/telos-core").exists()
}

/// Retrieves the current git commit hash from a local repository.
pub fn get_local_git_hash(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .ok()?;
    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !hash.is_empty() {
            return Some(hash);
        }
    }
    None
}

/// Retrieves recent git commits from local repository for patch notes.
pub fn get_local_git_commits(repo_dir: &Path, count: usize) -> Vec<PatchNoteEntry> {
    let mut notes = Vec::new();
    let format = "%h%x09%cd%x09%s";
    if let Ok(output) = Command::new("git")
        .args([
            "log",
            &format!("-n{count}"),
            "--date=short",
            &format!("--pretty=format:{format}"),
        ])
        .current_dir(repo_dir)
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 3 {
                    notes.push(PatchNoteEntry {
                        title: parts[2].to_string(),
                        version: parts[0].to_string(),
                        date: parts[1].to_string(),
                        summary: format!("Commit {} in main branch: {}", parts[0], parts[2]),
                    });
                }
            }
        }
    }
    notes
}

/// Returns the path to the cached remote versions file: `~/.telos/versions_cache.json`.
pub fn get_remote_versions_cache_path() -> PathBuf {
    get_telos_base_dir().join("versions_cache.json")
}

/// Retrieves cached remote version info immediately from disk or local git without blocking on network queries.
pub fn get_cached_remote_versions(repo_dir: &Path) -> RemoteVersionInfo {
    let cache_file = get_remote_versions_cache_path();
    if let Ok(data) = std::fs::read_to_string(&cache_file) {
        if let Ok(info) = serde_json::from_str::<RemoteVersionInfo>(&data) {
            return info;
        }
    }

    let mut info = RemoteVersionInfo::default();
    if let Some(hash) = get_local_git_hash(repo_dir) {
        info.latest_commit_hash = hash;
    }
    let local_commits = get_local_git_commits(repo_dir, 5);
    if !local_commits.is_empty() {
        info.latest_commit_msg = local_commits[0].title.clone();
        info.patch_notes = local_commits;
    }
    info
}

/// Fetches remote repository version info via GitHub API using curl or local fallback.
pub fn fetch_remote_versions(repo_dir: &Path) -> RemoteVersionInfo {
    let mut info = RemoteVersionInfo::default();

    // 1. If local git exists, read real HEAD commit
    if let Some(hash) = get_local_git_hash(repo_dir) {
        info.latest_commit_hash = hash;
    }
    let local_commits = get_local_git_commits(repo_dir, 5);
    if !local_commits.is_empty() {
        info.latest_commit_msg = local_commits[0].title.clone();
        info.patch_notes = local_commits;
    }

    // 2. Attempt lightweight remote GitHub query via curl
    let gh_api = "https://api.github.com/repos/larvance/telos/commits/main";
    let curl_res = Command::new("curl")
        .args([
            "-s",
            "--connect-timeout",
            "2",
            "-H",
            "User-Agent: Telos-Launcher",
            gh_api,
        ])
        .output();

    if let Ok(output) = curl_res {
        if output.status.success() {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(sha) = val.get("sha").and_then(|s| s.as_str()) {
                    let short_sha = if sha.len() >= 7 { &sha[..7] } else { sha };
                    info.latest_commit_hash = short_sha.to_string();
                }
                if let Some(msg) = val
                    .get("commit")
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.as_str())
                {
                    info.latest_commit_msg =
                        msg.lines().next().unwrap_or("Latest commit").to_string();
                }
            }
        }
    }

    // Persist to local cache for instant future launches
    let cache_file = get_remote_versions_cache_path();
    if let Some(parent) = cache_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(&info) {
        let _ = std::fs::write(&cache_file, data);
    }

    info
}

/// Generates the standard installation set for the launcher based on detected environment.
pub fn get_default_installations(repo_dir: &Path, remote: &RemoteVersionInfo) -> Vec<Installation> {
    let mut installs = Vec::new();

    // Always offer Latest Release
    installs.push(Installation::new_release(&remote.latest_release_tag));

    // Always offer Latest Snapshot (commit hash)
    installs.push(Installation::new_snapshot(format!(
        "snapshot-{}",
        remote.latest_commit_hash
    )));

    // If local dev environment is present, provide local dev build
    if detect_local_dev_repo(repo_dir) {
        installs.push(Installation::new_local_dev(
            repo_dir.to_path_buf(),
            &remote.latest_commit_hash,
        ));
    }

    installs
}

/// Returns the path to the launcher installations JSON file.
pub fn get_installations_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("telos").join("installations.json")
    } else {
        PathBuf::from(".telos_installations.json")
    }
}

/// Loads configured installations from disk, migrating or inserting defaults if needed.
pub fn load_or_init_installations(
    repo_dir: &Path,
    remote: &RemoteVersionInfo,
) -> Vec<Installation> {
    let path = get_installations_path();
    let is_dev_repo = detect_local_dev_repo(repo_dir);

    let mut saved_installs: Vec<Installation> = if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    };

    if saved_installs.is_empty() {
        let defaults = get_default_installations(repo_dir, remote);
        let _ = persist_installations(&defaults);
        return defaults;
    }

    // Ensure dev environment profile is present if in dev repo, or removed if not in dev repo
    if is_dev_repo {
        if !saved_installs.iter().any(|i| i.id == "local-dev") {
            saved_installs.insert(
                0,
                Installation::new_local_dev(repo_dir.to_path_buf(), &remote.latest_commit_hash),
            );
        } else {
            // Keep local-dev up-to-date with current commit & dir
            if let Some(dev_inst) = saved_installs.iter_mut().find(|i| i.id == "local-dev") {
                dev_inst.version_tag = format!("local @ {}", remote.latest_commit_hash);
                dev_inst.custom_dir = Some(repo_dir.to_path_buf());
                dev_inst.is_dev = true;
            }
        }
    } else {
        saved_installs.retain(|i| i.id != "local-dev");
    }

    // Ensure default latest-release exists
    if !saved_installs
        .iter()
        .any(|i| i.id == "latest-release" || i.channel == ChannelKind::LatestRelease)
    {
        saved_installs.push(Installation::new_release(&remote.latest_release_tag));
    }

    // Ensure default latest-snapshot exists
    if !saved_installs
        .iter()
        .any(|i| i.id == "latest-snapshot" || i.channel == ChannelKind::Snapshot)
    {
        saved_installs.push(Installation::new_snapshot(format!(
            "snapshot-{}",
            remote.latest_commit_hash
        )));
    }

    let _ = persist_installations(&saved_installs);
    saved_installs
}

/// Persists installations list to disk.
pub fn persist_installations(installations: &[Installation]) -> Result<(), String> {
    let path = get_installations_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(installations).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns the root Telos base directory (~/.telos).
pub fn get_telos_base_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".telos")
    } else {
        PathBuf::from(".telos")
    }
}

/// Returns the version cache directory for a given version tag: `~/.telos/versions/<version_tag>`.
pub fn get_version_cache_dir(version_tag: &str) -> PathBuf {
    get_telos_base_dir().join("versions").join(version_tag)
}

/// Returns the path to the executable inside the version cache.
pub fn get_version_binary_path(version_tag: &str) -> PathBuf {
    get_version_cache_dir(version_tag).join("telos")
}

/// Recursively copy a directory and its contents.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Ensures the given version tag is extracted in the version cache (`~/.telos/versions/<version_tag>/`).
/// Multiple installations referencing the same version tag share this single cached extraction!
pub fn ensure_version_cached(repo_dir: &Path, version_tag: &str) -> Result<PathBuf, String> {
    let bin_path = get_version_binary_path(version_tag);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    let version_dir = get_version_cache_dir(version_tag);
    let _ = std::fs::create_dir_all(&version_dir);

    // 1. Check if a prebuilt .tar.zst archive exists in repo dist/ (e.g. dist/telos-0.1.0-linux-x86_64.tar.zst)
    let candidate_archives = [
        repo_dir.join(format!("dist/telos-{version_tag}-linux-x86_64.tar.zst")),
        repo_dir.join(format!("dist/telos-{version_tag}.tar.zst")),
        repo_dir.join("dist/telos-0.1.0-linux-x86_64.tar.zst"),
    ];

    for archive in &candidate_archives {
        if archive.exists() {
            let status = Command::new("tar")
                .args([
                    "--zstd",
                    "-xf",
                    archive.to_str().unwrap(),
                    "-C",
                    version_dir.to_str().unwrap(),
                ])
                .status();
            if let Ok(s) = status {
                if s.success() && bin_path.exists() {
                    return Ok(bin_path);
                }
            }
        }
    }

    // 2. Local dev environment fallback: copy from target/release/telos + assets
    let target_release = repo_dir.join("target/release/telos");
    if target_release.exists() {
        std::fs::copy(&target_release, &bin_path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&bin_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&bin_path, perms);
            }
        }
        let assets_src = repo_dir.join("assets");
        if assets_src.exists() {
            let _ = copy_dir_all(&assets_src, &version_dir.join("assets"));
        }
        return Ok(bin_path);
    }

    // 3. Fallback to target/debug/telos if release isn't built yet
    let target_debug = repo_dir.join("target/debug/telos");
    if target_debug.exists() {
        std::fs::copy(&target_debug, &bin_path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&bin_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&bin_path, perms);
            }
        }
        let assets_src = repo_dir.join("assets");
        if assets_src.exists() {
            let _ = copy_dir_all(&assets_src, &version_dir.join("assets"));
        }
        return Ok(bin_path);
    }

    Err(format!(
        "Version '{}' binary is not present in cache ({:?}) and no prebuilt release archive was found.",
        version_tag, bin_path
    ))
}
