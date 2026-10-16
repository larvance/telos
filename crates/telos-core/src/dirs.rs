//! Platform-native application data layout and directory resolution.
//!
//! Provides standards-compliant path discovery across Linux (XDG Base Directory specification),
//! Windows (%APPDATA%, %LOCALAPPDATA%), and macOS (~/Library/Application Support, ~/Library/Caches).
//!
//! Supports portable mode (`--portable`), custom data directory overrides (`--data-dir`),
//! and transparent fallback to local workspace directories (`./worlds`, `./server.toml`)
//! or legacy `~/.voxel`.

use std::path::{Path, PathBuf};

/// Application name used for subdirectories in platform-native data roots.
pub const APP_NAME: &str = "telos";

/// Resolved application directories for settings, persistent world saves,
/// transient caches, and rolling logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    data_dir: PathBuf,
    config_dir: PathBuf,
    cache_dir: PathBuf,
    logs_dir: PathBuf,
    is_portable: bool,
}

impl AppDirs {
    /// Resolves standard platform directories with default fallbacks.
    #[must_use]
    pub fn standard() -> Self {
        resolve_standard_dirs()
    }

    /// Resolves directories checking for local server configuration fallback first.
    ///
    /// If `./server.toml` exists in the current working directory,
    /// returns an instance rooted at the current directory. Otherwise resolves
    /// standard platform directories.
    #[must_use]
    pub fn standard_with_local_fallback() -> Self {
        if let Ok(cwd) = std::env::current_dir()
            && cwd.join("server.toml").is_file()
        {
            return Self {
                data_dir: cwd.clone(),
                config_dir: cwd.clone(),
                cache_dir: cwd.join(".cache"),
                logs_dir: cwd.join("logs"),
                is_portable: false,
            };
        }
        Self::standard()
    }

    /// Resolves directories in portable mode inside the specified base path.
    ///
    /// All configuration, world data, caches, and logs are isolated within subdirectories
    /// of `base_dir`.
    pub fn portable(base_dir: impl Into<PathBuf>) -> Self {
        let base = base_dir.into();
        Self {
            data_dir: base.join("data"),
            config_dir: base.join("config"),
            cache_dir: base.join("cache"),
            logs_dir: base.join("logs"),
            is_portable: true,
        }
    }

    /// Resolves directories rooted at a custom data directory override.
    ///
    /// The `config_dir`, `cache_dir`, and `logs_dir` are derived from subdirectories of `data_dir`.
    pub fn from_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        let data = data_dir.into();
        Self {
            config_dir: data.join("config"),
            cache_dir: data.join("cache"),
            logs_dir: data.join("logs"),
            data_dir: data,
            is_portable: false,
        }
    }

    /// Overrides the persistent data directory.
    #[must_use]
    pub fn with_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = path.into();
        self
    }

    /// Overrides the configuration directory.
    #[must_use]
    pub fn with_config_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_dir = path.into();
        self
    }

    /// Overrides the transient cache directory.
    #[must_use]
    pub fn with_cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = path.into();
        self
    }

    /// Overrides the logging directory.
    #[must_use]
    pub fn with_logs_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.logs_dir = path.into();
        self
    }

    /// Returns the root data directory (containing saves, assets, and mods).
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Returns the configuration directory (containing settings files).
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Returns the transient disk cache directory (containing LOD clipmaps, shaders).
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Returns the rolling log output directory.
    #[must_use]
    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    /// Returns true if running in portable mode.
    #[must_use]
    pub fn is_portable(&self) -> bool {
        self.is_portable
    }

    /// Returns the directory where persistent world saves live: `<data_dir>/worlds`.
    /// On standard Linux/Unix installations, this defaults to `~/.telos/worlds`.
    #[must_use]
    pub fn saves_dir(&self) -> PathBuf {
        self.data_dir.join("worlds")
    }

    /// Returns the specific world save directory: `<data_dir>/worlds/<world_name>`.
    #[must_use]
    pub fn world_save_dir(&self, world_name: &str) -> PathBuf {
        self.saves_dir().join(world_name)
    }

    /// Returns the path to a specific configuration file: `<config_dir>/<filename>`.
    #[must_use]
    pub fn config_file(&self, filename: &str) -> PathBuf {
        self.config_dir.join(filename)
    }

    /// Returns the transient LOD disk cache directory for a given server:
    /// `<cache_dir>/lod/<server_id>`.
    #[must_use]
    pub fn lod_cache_dir(&self, server_id: &str) -> PathBuf {
        self.cache_dir.join("lod").join(server_id)
    }

    /// Returns the transient compiled shader cache directory:
    /// `<cache_dir>/shaders`.
    #[must_use]
    pub fn shader_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("shaders")
    }

    /// Returns the custom asset pack storage directory:
    /// `<data_dir>/assets`.
    #[must_use]
    pub fn assets_dir(&self) -> PathBuf {
        self.data_dir.join("assets")
    }

    /// Returns the mod storage directory:
    /// `<data_dir>/mods`.
    #[must_use]
    pub fn mods_dir(&self) -> PathBuf {
        self.data_dir.join("mods")
    }

    /// Seamlessly migrates legacy world saves found in `~/.local/share/telos/worlds`
    /// or local working directory `./worlds` into the target saves directory (e.g. `~/.telos/worlds`).
    pub fn migrate_legacy_worlds(target_saves_dir: &Path) {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return,
        };

        let mut legacy_candidates = vec![
            home.join(".local/share").join(APP_NAME).join("worlds"),
            home.join(".local/share").join("voxel").join("worlds"),
        ];

        if let Ok(cwd) = std::env::current_dir() {
            let cwd_worlds = cwd.join("worlds");
            if cwd_worlds.is_dir() && &cwd_worlds != target_saves_dir {
                legacy_candidates.push(cwd_worlds);
            }
        }

        for legacy_dir in &legacy_candidates {
            if !legacy_dir.is_dir() || legacy_dir == target_saves_dir {
                continue;
            }

            let entries = match std::fs::read_dir(legacy_dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let file_name = entry.file_name();
                let dest_path = target_saves_dir.join(&file_name);

                let should_migrate = if !dest_path.exists() {
                    true
                } else {
                    let src_time = path.metadata().and_then(|m| m.modified()).ok();
                    let dst_time = dest_path.metadata().and_then(|m| m.modified()).ok();
                    match (src_time, dst_time) {
                        (Some(st), Some(dt)) => st > dt,
                        _ => false,
                    }
                };

                if should_migrate {
                    let _ = std::fs::create_dir_all(target_saves_dir);
                    if dest_path.exists() {
                        let _ = std::fs::remove_dir_all(&dest_path);
                    }
                    if std::fs::rename(&path, &dest_path).is_err() {
                        if let Ok(()) = copy_dir_recursive(&path, &dest_path) {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    }
                }
            }

            // If legacy directory is now empty, clean it up
            if let Ok(mut remaining) = std::fs::read_dir(legacy_dir) {
                if remaining.next().is_none() {
                    let _ = std::fs::remove_dir(legacy_dir);
                }
            }
        }
    }

    /// Ensures that the primary directories (`data_dir`, `config_dir`, `cache_dir`, `logs_dir`, `saves_dir`)
    /// exist on disk, creating them if necessary.
    ///
    /// Also migrates any legacy world saves into `saves_dir`.
    ///
    /// # Errors
    /// Returns an `std::io::Error` if directory creation fails.
    pub fn ensure_dirs_exist(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        std::fs::create_dir_all(self.saves_dir())?;
        Self::migrate_legacy_worlds(&self.saves_dir());
        Ok(())
    }
}

/// Helper to recursively copy directories when cross-device moves fail.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Resolves standard platform directories according to OS conventions.
fn resolve_standard_dirs() -> AppDirs {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "windows")]
    {
        let data_dir = std::env::var_os("TELOS_HOME")
            .or_else(|| std::env::var_os("TELOS_DIR"))
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .map(|p| p.join("Telos"))
                    .unwrap_or_else(|| home.join("AppData").join("Roaming").join("Telos"))
            });
        let config_dir = data_dir.join("config");
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("Telos").join("cache"))
            .unwrap_or_else(|| {
                home.join("AppData")
                    .join("Local")
                    .join("Telos")
                    .join("cache")
            });
        let logs_dir = dirs::data_local_dir()
            .map(|p| p.join("Telos").join("logs"))
            .unwrap_or_else(|| {
                home.join("AppData")
                    .join("Local")
                    .join("Telos")
                    .join("logs")
            });

        AppDirs {
            data_dir,
            config_dir,
            cache_dir,
            logs_dir,
            is_portable: false,
        }
    }

    #[cfg(target_os = "macos")]
    {
        let data_dir = std::env::var_os("TELOS_HOME")
            .or_else(|| std::env::var_os("TELOS_DIR"))
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if home.join(".telos").is_dir() {
                    home.join(".telos")
                } else {
                    dirs::data_dir()
                        .map(|p| p.join("Telos"))
                        .unwrap_or_else(|| home.join("Library/Application Support/Telos"))
                }
            });
        let config_dir = data_dir.join("config");
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("Telos"))
            .unwrap_or_else(|| home.join("Library/Caches/Telos"));
        let logs_dir = home.join("Library/Logs/Telos");

        AppDirs {
            data_dir,
            config_dir,
            cache_dir,
            logs_dir,
            is_portable: false,
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux / Unix / BSD: game data, worlds, assets, and mods live in ~/.telos
        let telos_home = std::env::var_os("TELOS_HOME")
            .or_else(|| std::env::var_os("TELOS_DIR"))
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".telos"));

        let data_dir = telos_home.clone();

        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(|v| PathBuf::from(v).join(APP_NAME))
            .or_else(|| dirs::config_dir().map(|p| p.join(APP_NAME)))
            .unwrap_or_else(|| home.join(".config").join(APP_NAME));

        let cache_dir = std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(|v| PathBuf::from(v).join(APP_NAME))
            .or_else(|| dirs::cache_dir().map(|p| p.join(APP_NAME)))
            .unwrap_or_else(|| telos_home.join("cache"));

        let logs_dir = telos_home.join("logs");

        AppDirs {
            data_dir,
            config_dir,
            cache_dir,
            logs_dir,
            is_portable: false,
        }
    }
}
