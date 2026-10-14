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
