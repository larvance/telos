//! Repository release fetcher, installation registry, and version checker.
//!
//! Queries repository releases and commits (remote via GitHub API/curl or local via git),
//! detects local development workspaces, and tracks installed/configured launch profiles.

use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

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
        .args(["log", &format!("-n{count}"), "--date=short", &format!("--pretty=format:{format}")])
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
        .args(["-s", "--connect-timeout", "2", "-H", "User-Agent: Telos-Launcher", gh_api])
        .output();

    if let Ok(output) = curl_res {
        if output.status.success() {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(sha) = val.get("sha").and_then(|s| s.as_str()) {
                    let short_sha = if sha.len() >= 7 { &sha[..7] } else { sha };
                    info.latest_commit_hash = short_sha.to_string();
                }
                if let Some(msg) = val.get("commit").and_then(|c| c.get("message")).and_then(|m| m.as_str()) {
                    info.latest_commit_msg = msg.lines().next().unwrap_or("Latest commit").to_string();
                }
            }
        }
    }

    info
}

/// Generates the standard installation set for the launcher based on detected environment.
pub fn get_default_installations(repo_dir: &Path, remote: &RemoteVersionInfo) -> Vec<Installation> {
    let mut installs = Vec::new();

    // Always offer Latest Release
    installs.push(Installation::new_release(&remote.latest_release_tag));

    // Always offer Latest Snapshot (commit hash)
    installs.push(Installation::new_snapshot(format!("snapshot-{}", remote.latest_commit_hash)));

    // If local dev environment is present, provide local dev build
    if detect_local_dev_repo(repo_dir) {
        installs.push(Installation::new_local_dev(
            repo_dir.to_path_buf(),
            &remote.latest_commit_hash,
        ));
    }

    installs
}
