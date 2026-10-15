//! Build coordinator and multi-instance concurrency lock manager for Telos.
//!
//! Provides advisory file locking to coordinate multiple processes attempting to build
//! or launch the game client, preventing duplicate compilation while streaming real-time
//! crate compilation progress.

#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// Event sent from the background compilation/lock worker to the launcher UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildEvent {
    /// Update status message with current activity.
    Status(String),
    /// A crate has started compiling.
    CrateStarted(String),
    /// A crate artifact has completed.
    ArtifactFinished {
        crate_name: String,
        is_target_bin: bool,
    },
    /// Build lock is currently held by another process; waiting.
    LockHeld { message: String },
    /// Compilation or lock wait succeeded.
    Success { already_built: bool },
    /// Build failed with compiler error output.
    Failed(String),
}

/// Attempts to acquire an exclusive lock non-blockingly using advisory file locking.
pub fn try_lock_exclusive(file: &std::fs::File) -> bool {
    rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_ok()
}

/// Resolves the root workspace repository directory.
pub fn resolve_repo_dir() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = Path::new(&manifest_dir);
        if let Some(parent) = p.parent().and_then(Path::parent) {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
        }
    }
    let default_path = PathBuf::from("/home/usr/Projects/larvance/telos");
    if default_path.join("Cargo.toml").exists() {
        return default_path;
    }
    PathBuf::from("/home/usr/Projects/voxel")
}

/// Computes a deterministic SHA-256 hash of all source files for the Telos game app.
#[must_use]
pub fn compute_app_code_hash(repo_dir: &Path) -> Option<String> {
    let script = r#"find apps/telos crates shaders Cargo.toml Cargo.lock -type f -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/node_modules/*" -print0 2>/dev/null | sort -z | xargs -0 sha256sum 2>/dev/null | sha256sum | awk '{print $1}'"#;
    let output = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.len() == 64 {
            return Some(hash);
        }
    }
    None
}

/// Computes a deterministic SHA-256 hash of all source files for the Telos launcher.
#[must_use]
pub fn compute_launcher_code_hash(repo_dir: &Path) -> Option<String> {
    let script = r#"find apps/telos-launcher/src apps/telos-launcher/ui apps/telos-launcher/icons apps/telos-launcher/Cargo.toml apps/telos-launcher/tauri.conf.json apps/telos-launcher/build.rs Cargo.lock -type f -not -path "*/node_modules/*" -not -path "*/target/*" -not -path "*/.git/*" -print0 2>/dev/null | sort -z | xargs -0 sha256sum 2>/dev/null | sha256sum | awk '{print $1}'"#;
    let output = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.len() == 64 {
            return Some(hash);
        }
    }
    None
}

/// Spawns the background build worker that coordinates locking and builds.
pub fn start_build_worker(
    repo_dir: PathBuf,
    is_debug: bool,
    tx: Sender<BuildEvent>,
    start_time: SystemTime,
) {
    std::thread::Builder::new()
        .name("telos-build-worker".into())
        .spawn(move || {
            let target_bin = if is_debug {
                repo_dir.join("target/debug/telos")
            } else {
                repo_dir.join("target/release/telos")
            };

            let mode = if is_debug { "debug" } else { "release" };
            let hash_file = repo_dir.join("target").join(format!(".telos-app-{mode}.codehash"));
            let current_hash = compute_app_code_hash(&repo_dir);

            // Fast path: if target binary already exists, verify if the code hash is unchanged.
            // If the code hash hasn't changed, do not compile the app at all!
            if target_bin.exists() {
                if let Some(ref cur_hash) = current_hash {
                    let mut up_to_date = false;
                    if let Ok(saved_hash) = std::fs::read_to_string(&hash_file) {
                        if saved_hash.trim() == cur_hash {
                            up_to_date = true;
                        }
                    }

                    if up_to_date {
                        let _ = tx.send(BuildEvent::Success {
                            already_built: true,
                        });
                        return;
                    }
                }
            }

            let lock_dir = repo_dir.join("target");
            let _ = std::fs::create_dir_all(&lock_dir);
            let lock_path = lock_dir.join(".telos-dev.lock");

            let lock_file = match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
            {
                Ok(f) => f,
                Err(err) => {
                    let _ = tx.send(BuildEvent::Failed(format!(
                        "Failed to open lockfile: {err}"
                    )));
                    return;
                }
            };

            // 1. Check if another instance is already building
            let initial_lock = try_lock_exclusive(&lock_file);
            let mut waited_for_lock = false;

            if !initial_lock {
                waited_for_lock = true;
                let _ = tx.send(BuildEvent::LockHeld {
                    message: "Another Telos build is in progress. Auto-launching when finished."
                        .into(),
                });

                // Wait for the active build to complete and release the lock
                loop {
                    std::thread::sleep(Duration::from_millis(120));
                    if try_lock_exclusive(&lock_file) {
                        break;
                    }
                }
            }

            // Lock is now held exclusively by this process!
            // 2. Multi-instance rule: If we waited for the lock, check if the other instance built it!
            if waited_for_lock && target_bin.exists() {
                let mut fresh_or_matching = false;
                if let (Some(cur_hash), Ok(saved_hash)) =
                    (&current_hash, std::fs::read_to_string(&hash_file))
                {
                    if saved_hash.trim() == cur_hash {
                        fresh_or_matching = true;
                    }
                }
                if !fresh_or_matching {
                    fresh_or_matching = target_bin
                        .metadata()
                        .and_then(|m| m.modified())
                        .map_or(false, |mtime| {
                            mtime
                                >= start_time
                                    .checked_sub(Duration::from_secs(5))
                                    .unwrap_or(start_time)
                        });
                }

                if fresh_or_matching {
                    let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);
                    let _ = tx.send(BuildEvent::Success {
                        already_built: true,
                    });
                    return;
                }
            }

            // 3. We are the builder or binary is missing/outdated: execute `cargo build`
            let _ = tx.send(BuildEvent::Status("Starting Cargo build...".into()));

            let mut cmd = Command::new("cargo");
            cmd.current_dir(&repo_dir);
            cmd.arg("build");
            if !is_debug {
                cmd.arg("--release");
            }
            cmd.args(["--bin", "telos", "--message-format=json"]);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(err) => {
                    let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);
                    let _ = tx.send(BuildEvent::Failed(format!("Failed to spawn cargo: {err}")));
                    return;
                }
            };

            let stdout = child.stdout.take().expect("piped stdout");
            let stderr = child.stderr.take().expect("piped stderr");

            let tx_err = tx.clone();
            let stderr_handle = std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                let mut accumulated_errors = Vec::new();
                for line in reader.lines().map_while(Result::ok) {
                    if line.trim().starts_with("Compiling") {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.len() >= 2 {
                            let _ = tx_err.send(BuildEvent::CrateStarted(parts[1].to_string()));
                        }
                    } else if line.contains("error:") || line.contains("Error") {
                        accumulated_errors.push(line);
                    }
                }
                accumulated_errors.join("\n")
            });

            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if val.get("reason").and_then(|r| r.as_str()) == Some("compiler-artifact") {
                        let target_name = val
                            .get("target")
                            .and_then(|t| t.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("crate");
                        let is_bin = val
                            .get("target")
                            .and_then(|t| t.get("kind"))
                            .and_then(|k| k.as_array())
                            .map_or(false, |kinds| {
                                kinds.iter().any(|k| k.as_str() == Some("bin"))
                            });

                        let _ = tx.send(BuildEvent::ArtifactFinished {
                            crate_name: target_name.to_string(),
                            is_target_bin: is_bin && target_name == "telos",
                        });
                    }
                }
            }

            let status = child.wait();
            let accumulated_err = stderr_handle.join().unwrap_or_default();

            // Release file lock immediately upon build completion
            let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);

            match status {
                Ok(exit) if exit.success() => {
                    if let Some(ref cur_hash) = current_hash {
                        let _ = std::fs::create_dir_all(repo_dir.join("target"));
                        let _ = std::fs::write(&hash_file, cur_hash);
                    }
                    let _ = tx.send(BuildEvent::Success {
                        already_built: false,
                    });
                }
                Ok(exit) => {
                    let _ = tx.send(BuildEvent::Failed(format!(
                        "Cargo exited with code {}\n{}",
                        exit.code().unwrap_or(1),
                        accumulated_err
                    )));
                }
                Err(err) => {
                    let _ = tx.send(BuildEvent::Failed(format!("Cargo process error: {err}")));
                }
            }
        })
        .expect("spawn build worker thread");
}
