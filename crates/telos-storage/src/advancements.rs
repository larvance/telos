//! Persistent player advancements storage in the world directory.
//!
//! Stores per-player advancement progress as TOML documents in
//! `<world_dir>/advancements/<player_id>.toml` using atomic write
//! semantics (`.tmp` + rename) to guarantee crash safety.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Result, StorageError};

/// Resolves the canonical file path for a player's advancement TOML document.
#[must_use]
pub fn player_advancements_path(world_dir: &Path, player_id: &str) -> PathBuf {
    // Sanitize player_id to avoid path traversal vulnerabilities
    let sanitized: String = player_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    let safe_id = if sanitized.is_empty() {
        "default"
    } else {
        &sanitized
    };

    world_dir
        .join("advancements")
        .join(format!("{safe_id}.toml"))
}

/// Loads a player's advancement progress from `<world_dir>/advancements/<player_id>.toml`
/// (with automatic fallback and migration from legacy `.json` files).
///
/// Returns `Ok(None)` if the advancement file does not exist yet.
pub fn load_player_advancements<T: serde::de::DeserializeOwned + serde::Serialize>(
    world_dir: &Path,
    player_id: &str,
) -> Result<Option<T>> {
    let toml_path = player_advancements_path(world_dir, player_id);
    if toml_path.exists() {
        let text = fs::read_to_string(&toml_path).map_err(StorageError::Io)?;
        let data: T = toml::from_str(&text).map_err(|e| {
            StorageError::CorruptPayload(format!("Failed to parse advancements TOML: {e}"))
        })?;
        return Ok(Some(data));
    }

    // Backwards-compatible migration from legacy .json
    let legacy_json = toml_path.with_extension("json");
    if legacy_json.exists() {
        let bytes = fs::read(&legacy_json).map_err(StorageError::Io)?;
        let data: T = serde_json::from_slice(&bytes).map_err(|e| {
            StorageError::CorruptPayload(format!("Failed to parse legacy advancements JSON: {e}"))
        })?;
        let _ = save_player_advancements(world_dir, player_id, &data);
        let _ = fs::remove_file(&legacy_json);
        return Ok(Some(data));
    }

    Ok(None)
}

/// Atomically saves a player's advancement progress to `<world_dir>/advancements/<player_id>.toml`.
///
/// Writes to a temporary `.tmp` file and flushes before performing an atomic filesystem rename.
pub fn save_player_advancements<T: serde::Serialize>(
    world_dir: &Path,
    player_id: &str,
    advancements: &T,
) -> Result<()> {
    let final_path = player_advancements_path(world_dir, player_id);
    let dir = final_path.parent().expect("advancements dir has parent");

    fs::create_dir_all(dir).map_err(StorageError::Io)?;

    let tmp_path = final_path.with_extension("toml.tmp");

    let toml_str = toml::to_string_pretty(advancements).map_err(|e| {
        StorageError::CorruptPayload(format!("Failed to serialize advancements TOML: {e}"))
    })?;

    {
        let mut file = fs::File::create(&tmp_path).map_err(StorageError::Io)?;
        file.write_all(toml_str.as_bytes())
            .map_err(StorageError::Io)?;
        file.sync_data().map_err(StorageError::Io)?;
    }

    fs::rename(&tmp_path, &final_path).map_err(StorageError::Io)?;

    // Remove legacy .json if present
    let legacy_json = final_path.with_extension("json");
    if legacy_json.exists() {
        let _ = fs::remove_file(&legacy_json);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct MockProgress {
        completed: BTreeMap<String, u64>,
    }

    #[test]
    fn test_save_and_load_player_advancements_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let world_path = dir.path();

        let mut progress = MockProgress {
            completed: BTreeMap::new(),
        };
        progress
            .completed
            .insert("telos:story/root".to_string(), 12345);
        progress
            .completed
            .insert("telos:story/mine_wood".to_string(), 12350);

        // Non-existent load returns Ok(None)
        let loaded = load_player_advancements::<MockProgress>(world_path, "PlayerOne")
            .expect("load non-existent");
        assert_eq!(loaded, None);

        // Save progress
        save_player_advancements(world_path, "PlayerOne", &progress).expect("save advancements");

        // Load progress and verify exact equality
        let loaded = load_player_advancements::<MockProgress>(world_path, "PlayerOne")
            .expect("load saved")
            .expect("progress exists");
        assert_eq!(loaded, progress);

        // Path traversal sanitization check
        let path = player_advancements_path(world_path, "../../etc/passwd");
        assert_eq!(path, world_path.join("advancements").join("etcpasswd.toml"));
    }

    #[test]
    fn test_advancements_legacy_json_migration() {
        let dir = tempdir().expect("tempdir");
        let world_path = dir.path();
        let adv_dir = world_path.join("advancements");
        fs::create_dir_all(&adv_dir).unwrap();

        let legacy_json = adv_dir.join("OldPlayer.json");
        fs::write(&legacy_json, r#"{"completed":{"telos:story/root":9999}}"#).unwrap();

        let loaded: MockProgress = load_player_advancements(world_path, "OldPlayer")
            .unwrap()
            .expect("should load from legacy json");
        assert_eq!(loaded.completed.get("telos:story/root"), Some(&9999));

        // Verify .toml was created and legacy .json was deleted
        assert!(adv_dir.join("OldPlayer.toml").exists());
        assert!(!legacy_json.exists());
    }
}
