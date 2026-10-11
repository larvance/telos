#!/usr/bin/env bash
# ==============================================================================
# Telos Local Development Launcher
# Directly connected to local workspace at /home/usr/Projects/voxel
# Automatically ensures the latest build is executed with splash & lock coordination.
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
    REPO_DIR="/home/usr/Projects/larvance/telos"
fi

cd "$REPO_DIR"

DEV_BIN="$REPO_DIR/target/release/telos-dev"
TARGET_BIN="$REPO_DIR/target/release/telos"
if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
    DEV_BIN="$REPO_DIR/target/debug/telos-dev"
    TARGET_BIN="$REPO_DIR/target/debug/telos"
fi

# Ensure dev launcher binary exists if cargo is available
if [[ ! -x "$DEV_BIN" && -z "${TELOS_NO_DEV:-}" && -z "${TELOS_NO_BUILD:-}" ]] && command -v cargo >/dev/null 2>&1; then
    if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
        cargo build -p telos-dev --quiet || true
    else
        cargo build --release -p telos-dev --quiet || true
    fi
fi

# If dev launcher is available and enabled, delegate build progress & lock coordination to it
if [[ -z "${TELOS_NO_DEV:-}" && -x "$DEV_BIN" && -z "${TELOS_NO_BUILD:-}" ]]; then
    exec "$DEV_BIN" "$@"
fi

# Direct build fallback if TELOS_NO_DEV=1
if [[ -z "${TELOS_NO_BUILD:-}" ]] && command -v cargo >/dev/null 2>&1; then
    if [ -t 1 ]; then
        if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
            cargo build --bin telos
        else
            cargo build --release --bin telos
        fi
    else
        if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
            cargo build --bin telos --quiet 2>/dev/null || true
        else
            cargo build --release --bin telos --quiet 2>/dev/null || true
        fi
    fi
fi

# Execute the latest binary with all passed arguments
if [[ -x "$TARGET_BIN" ]]; then
    exec "$TARGET_BIN" "$@"
elif [[ -x "$REPO_DIR/target/release/telos" ]]; then
    exec "$REPO_DIR/target/release/telos" "$@"
elif [[ -x "$REPO_DIR/target/debug/telos" ]]; then
    exec "$REPO_DIR/target/debug/telos" "$@"
else
    exec cargo run --release --bin telos -- "$@"
fi
