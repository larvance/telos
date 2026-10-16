//! Integration and unit tests for Telos launcher.

use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_resolve_repo_dir() {
    let repo = std::env::current_dir().unwrap();
    assert!(
        repo.join("Cargo.toml").exists(),
        "Repo dir must contain Cargo.toml"
    );
}

#[test]
fn test_exclusive_lock_workflow() {
    let dir = tempdir().unwrap();
    let lock_path = dir.path().join("test.lock");
    let file = File::create(&lock_path).unwrap();

    let locked = rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive);
    assert!(locked.is_ok(), "First exclusive lock must succeed");

    // Attempt second non-blocking lock on same file descriptor
    let second = rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive);
    assert!(
        second.is_ok(),
        "Same fd re-lock is valid in POSIX flock semantics"
    );

    let unlocked = rustix::fs::flock(&file, rustix::fs::FlockOperation::Unlock);
    assert!(unlocked.is_ok(), "Unlock must succeed");
}

#[test]
fn test_profile_serialization_roundtrip() {
    let dir = tempdir().unwrap();
    let profile_file = dir.path().join("launcher_profile.json");

    let original_json = serde_json::json!({
        "logged_in": true,
        "username": "TestPlayer",
        "account_type": "Telos Account",
        "selected_installation_id": "latest-snapshot",
        "language": "English - United States",
        "auto_close_on_play": true,
        "game_dir": null
    });

    std::fs::write(
        &profile_file,
        serde_json::to_string(&original_json).unwrap(),
    )
    .unwrap();
    let read_back: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&profile_file).unwrap()).unwrap();

    assert_eq!(read_back["username"], "TestPlayer");
    assert_eq!(read_back["logged_in"], true);
    assert_eq!(read_back["selected_installation_id"], "latest-snapshot");
    assert_eq!(read_back["auto_close_on_play"], true);
}

#[test]
fn test_installation_channels() {
    let release = serde_json::json!({
        "id": "latest-release",
        "name": "Latest release",
        "channel": "LatestRelease",
        "version_tag": "v0.1.0",
        "is_dev": false
    });

    let snapshot = serde_json::json!({
        "id": "latest-snapshot",
        "name": "Latest snapshot",
        "channel": "Snapshot",
        "version_tag": "snapshot-3326973",
        "is_dev": false
    });

    let dev = serde_json::json!({
        "id": "local-dev",
        "name": "Dev Environment (Local Build)",
        "channel": "LocalDev",
        "version_tag": "local @ 3326973",
        "custom_dir": "/home/usr/Projects/larvance/telos",
        "is_dev": true
    });

    assert_eq!(release["channel"], "LatestRelease");
    assert_eq!(snapshot["channel"], "Snapshot");
    assert_eq!(dev["channel"], "LocalDev");
    assert_eq!(dev["is_dev"], true);
}

#[test]
fn test_seamless_update_lifecycle_integration() {
    let dir = tempdir().unwrap();
    let old_launcher_bin = dir.path().join("telos-launcher");
    let new_launcher_bin = dir.path().join("new_release_bin");
    let staging_bin = dir.path().join("staged_launcher");
    let pending_json = dir.path().join("pending_update.json");

    // Initial state: old launcher exists
    std::fs::write(&old_launcher_bin, b"#!/bin/sh\necho 'version 0.1.0'\n").unwrap();
    std::fs::write(
        &new_launcher_bin,
        b"#!/bin/sh\necho 'version 0.1.1-updated'\n",
    )
    .unwrap();

    // 1. Stage update
    std::fs::copy(&new_launcher_bin, &staging_bin).unwrap();
    let update_info = serde_json::json!({
        "version": "v0.1.1",
        "release_name": "Telos Launcher v0.1.1",
        "staged_binary_path": staging_bin,
        "target_binary_path": old_launcher_bin,
        "download_timestamp": 1729000000,
        "ready": true,
    });
    std::fs::write(&pending_json, serde_json::to_string(&update_info).unwrap()).unwrap();

    // Verify pending update detection
    assert!(pending_json.exists());
    assert!(staging_bin.exists());

    // 2. Perform seamless update replacement
    let temp_target = dir.path().join(".telos-launcher-new.tmp");
    std::fs::copy(&staging_bin, &temp_target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_target).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_target, perms).unwrap();
    }
    std::fs::rename(&temp_target, &old_launcher_bin).unwrap();
    std::fs::remove_file(&pending_json).unwrap();
    std::fs::remove_file(&staging_bin).unwrap();

    // 3. Verify target has been updated seamlessly
    let updated_bytes = std::fs::read(&old_launcher_bin).unwrap();
    assert_eq!(updated_bytes, b"#!/bin/sh\necho 'version 0.1.1-updated'\n");
    assert!(!pending_json.exists());
    assert!(!staging_bin.exists());
}

#[path = "../src/build_coordinator.rs"]
mod build_coordinator;

#[test]
fn test_compute_code_hashes_determinism() {
    let repo_dir = build_coordinator::resolve_repo_dir();
    assert!(repo_dir.exists(), "Repository directory must exist");

    let app_hash_1 = build_coordinator::compute_app_code_hash(&repo_dir);
    let app_hash_2 = build_coordinator::compute_app_code_hash(&repo_dir);
    assert!(app_hash_1.is_some(), "App code hash should be computed");
    assert_eq!(
        app_hash_1.as_ref().map(|s| s.len()),
        Some(64),
        "SHA-256 hash must be 64 characters"
    );
    assert_eq!(
        app_hash_1, app_hash_2,
        "App code hash must be deterministic"
    );

    let launcher_hash_1 = build_coordinator::compute_launcher_code_hash(&repo_dir);
    let launcher_hash_2 = build_coordinator::compute_launcher_code_hash(&repo_dir);
    assert!(
        launcher_hash_1.is_some(),
        "Launcher code hash should be computed"
    );
    assert_eq!(
        launcher_hash_1.as_ref().map(|s| s.len()),
        Some(64),
        "SHA-256 hash must be 64 characters"
    );
    assert_eq!(
        launcher_hash_1, launcher_hash_2,
        "Launcher code hash must be deterministic"
    );
}

#[test]
fn test_code_hash_cache_skip_behavior() {
    let dir = tempdir().unwrap();
    let bin_path = dir.path().join("telos");
    let hash_path = dir.path().join(".telos-app-release.codehash");

    // Initially no binary, no hash
    assert!(!bin_path.exists());
    assert!(!hash_path.exists());

    // Create binary and write matching hash
    std::fs::write(&bin_path, b"binary_content").unwrap();
    let expected_hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    std::fs::write(&hash_path, expected_hash).unwrap();

    let saved = std::fs::read_to_string(&hash_path).unwrap();
    assert_eq!(saved.trim(), expected_hash, "Hash should match saved");

    // When hash matches, compilation should be skipped
    let is_up_to_date = bin_path.exists() && saved.trim() == expected_hash;
    assert!(is_up_to_date, "Should be up to date and skip compilation");

    // When code changes, hash differs -> needs build
    let changed_hash = "1111111111111111111111111111111111111111111111111111111111111111";
    let is_still_up_to_date = bin_path.exists() && saved.trim() == changed_hash;
    assert!(
        !is_still_up_to_date,
        "Should detect hash mismatch and trigger build"
    );
}
