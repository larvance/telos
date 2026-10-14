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

# If target binary does not exist or sources are newer, compile it first
NEED_BUILD=0
if [[ ! -x "$TARGET_BIN" ]]; then
    NEED_BUILD=1
elif [[ -n "$(find "$REPO_DIR/crates" "$REPO_DIR/Cargo.toml" -newer "$TARGET_BIN" 2>/dev/null | head -n 1)" ]]; then
    NEED_BUILD=1
fi

if [[ "$NEED_BUILD" == "1" ]] && command -v cargo >/dev/null 2>&1; then
    if ! [ -t 1 ] && command -v notify-send >/dev/null 2>&1; then
        notify-send -i telos "Telos Dev" "Building game binary..." -t 4000 2>/dev/null || true
    fi
    if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
        cargo build --bin telos
    else
        cargo build --release --bin telos
    fi
fi

# Immediately open the game
exec "$TARGET_BIN" "$@"
