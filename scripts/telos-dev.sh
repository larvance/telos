#!/usr/bin/env bash
# ==============================================================================
# Telos Dev - Instant Local Development Game Launcher
# Directly launches the local compiled Telos game binary.
# ==============================================================================
set -e

# Resolve repository directory (symlink-aware)
SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SOURCE" ]; do
    DIR="$(cd -P "$(dirname "$SOURCE")" >/dev/null 2>&1 && pwd)"
    SOURCE="$(readlink "$SOURCE")"
    [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$(cd -P "$(dirname "$SOURCE")" >/dev/null 2>&1 && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." >/dev/null 2>&1 && pwd)"

# Fallback if invoked detached
if [[ ! -f "$REPO_DIR/Cargo.toml" ]]; then
    if [[ -f "/home/usr/Projects/voxel/Cargo.toml" ]]; then
        REPO_DIR="/home/usr/Projects/voxel"
    elif [[ -f "/home/usr/Projects/larvance/telos/Cargo.toml" ]]; then
        REPO_DIR="/home/usr/Projects/larvance/telos"
    fi
fi

cd "$REPO_DIR"

TARGET_BIN="$REPO_DIR/target/release/telos"
if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
    TARGET_BIN="$REPO_DIR/target/debug/telos"
fi

# If target binary does not exist, compile it first
if [[ ! -x "$TARGET_BIN" ]]; then
    if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
        cargo build --bin telos
    else
        cargo build --release --bin telos
    fi
fi

# Immediately open the game
exec "$TARGET_BIN" "$@"
