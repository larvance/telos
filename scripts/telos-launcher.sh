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

BUILD_TYPE="release"
DEV_BIN="$REPO_DIR/target/release/telos-launcher"
TARGET_BIN="$REPO_DIR/target/release/telos"
if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
    BUILD_TYPE="debug"
    DEV_BIN="$REPO_DIR/target/debug/telos-launcher"
    TARGET_BIN="$REPO_DIR/target/debug/telos"
fi
LAUNCHER_HASH_FILE="$REPO_DIR/target/.telos-launcher-${BUILD_TYPE}.codehash"
APP_HASH_FILE="$REPO_DIR/target/.telos-app-${BUILD_TYPE}.codehash"

compute_launcher_hash() {
    (
        cd "$REPO_DIR" && \
        find apps/telos-launcher/src \
             apps/telos-launcher/ui \
             apps/telos-launcher/icons \
             apps/telos-launcher/Cargo.toml \
             apps/telos-launcher/tauri.conf.json \
             apps/telos-launcher/build.rs \
             Cargo.lock \
             -type f -not -path "*/node_modules/*" -not -path "*/target/*" -not -path "*/.git/*" -print0 2>/dev/null \
            | sort -z \
            | xargs -0 sha256sum 2>/dev/null \
            | sha256sum \
            | awk '{print $1}'
    )
}

compute_app_hash() {
    (
        cd "$REPO_DIR" && \
        find apps/telos \
             crates \
             shaders \
             Cargo.toml \
             Cargo.lock \
             -type f -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/node_modules/*" -print0 2>/dev/null \
            | sort -z \
            | xargs -0 sha256sum 2>/dev/null \
            | sha256sum \
            | awk '{print $1}'
    )
}

# Fast path: If an existing launcher instance is already running, activate it immediately!
if [[ -x "$DEV_BIN" ]] && "$DEV_BIN" --activate >/dev/null 2>&1; then
    exit 0
fi

# Ensure launcher binary exists and is up-to-date by comparing code hash
NEED_LAUNCHER_BUILD=0
CURRENT_LAUNCHER_HASH=""
if [[ ! -x "$DEV_BIN" ]]; then
    NEED_LAUNCHER_BUILD=1
else
    CURRENT_LAUNCHER_HASH="$(compute_launcher_hash)"
    if [[ -f "$LAUNCHER_HASH_FILE" ]]; then
        SAVED_LAUNCHER_HASH="$(cat "$LAUNCHER_HASH_FILE" 2>/dev/null || true)"
        if [[ "$SAVED_LAUNCHER_HASH" != "$CURRENT_LAUNCHER_HASH" ]]; then
            NEED_LAUNCHER_BUILD=1
        fi
    else
        NEED_LAUNCHER_BUILD=1
    fi
fi

# Execute build with an OS-level progress window (never use desktop notifications)
build_with_os_progress() {
    local title="$1"
    local initial_text="$2"
    shift 2

    # If graphical display and zenity are available, show native OS-level progress window
    if command -v zenity >/dev/null 2>&1 && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
        local build_log
        build_log="$(mktemp /tmp/telos_build_XXXXXX.log)"

        {
            local build_pid
            trap 'kill "$build_pid" 2>/dev/null || true' EXIT INT TERM
            echo "5"
            echo "# $initial_text"
            "$@" > "$build_log" 2>&1 &
            build_pid=$!
            local pct=8
            local last_crate=""
            local idle_ticks=0

            while kill -0 "$build_pid" 2>/dev/null; do
                local last_line
                last_line="$(tail -n 1 "$build_log" 2>/dev/null || true)"
                if [[ "$last_line" =~ Compiling[[:space:]]+([a-zA-Z0-9_-]+) ]]; then
                    local cur_crate="${BASH_REMATCH[1]}"
                    if [[ "$cur_crate" != "$last_crate" ]]; then
                        last_crate="$cur_crate"
                        echo "# Compiling $cur_crate..."
                        pct=$(( (pct + 7) < 92 ? (pct + 7) : 92 ))
                        echo "$pct"
                        idle_ticks=0
                    fi
                elif [[ "$last_line" =~ Finished ]]; then
                    echo "# Finalizing binary..."
                    echo "96"
                fi

                idle_ticks=$((idle_ticks + 1))
                if [[ $idle_ticks -ge 5 ]]; then
                    idle_ticks=0
                    if [[ $pct -lt 92 ]]; then
                        pct=$((pct + 1))
                        echo "$pct"
                    fi
                fi
                sleep 0.15
            done

            wait "$build_pid"
            local exit_code=$?

            if [[ "$exit_code" -eq 0 ]]; then
                echo "100"
                echo "# Launching $title..."
                sleep 0.2
            else
                exit "$exit_code"
            fi
        } | zenity --progress \
            --title="$title" \
            --text="$initial_text" \
            --percentage=5 \
            --auto-close \
            --auto-kill \
            --width=460 \
            --height=120
        local status=$?

        if [[ "$status" -ne 0 && -f "$build_log" ]]; then
            local err_sample
            err_sample="$(tail -n 12 "$build_log" 2>/dev/null || echo "Unknown build failure")"
            zenity --error \
                --title="$title Build Error" \
                --text="Failed to build $title:\n\n$err_sample" \
                --width=500 2>/dev/null || true
        fi
        rm -f "$build_log"
        return $status
    elif [[ -x "$REPO_DIR/target/release/telos-updater" ]] && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
        "$REPO_DIR/target/release/telos-updater" &
        local up_pid=$!
        "$@" || true
        kill "$up_pid" 2>/dev/null || true
    else
        "$@"
    fi
}

if [[ "$NEED_LAUNCHER_BUILD" == "1" && -z "${TELOS_NO_DEV:-}" && -z "${TELOS_NO_BUILD:-}" ]] && command -v cargo >/dev/null 2>&1; then
    if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
        build_with_os_progress "Telos Launcher" "Building latest launcher updates..." cargo build -p telos-launcher --bin telos-launcher
    else
        build_with_os_progress "Telos Launcher" "Building latest launcher updates..." cargo build --release -p telos-launcher --bin telos-launcher
    fi
    mkdir -p "$REPO_DIR/target"
    if [[ -z "$CURRENT_LAUNCHER_HASH" ]]; then
        CURRENT_LAUNCHER_HASH="$(compute_launcher_hash)"
    fi
    echo "$CURRENT_LAUNCHER_HASH" > "$LAUNCHER_HASH_FILE"
fi

# If launcher is available and enabled, delegate to it
if [[ -z "${TELOS_NO_DEV:-}" && -x "$DEV_BIN" && -z "${TELOS_NO_BUILD:-}" ]]; then
    exec "$DEV_BIN" "$@"
fi

# Direct build fallback if TELOS_NO_DEV=1
if [[ -z "${TELOS_NO_BUILD:-}" ]] && command -v cargo >/dev/null 2>&1; then
    NEED_APP_BUILD=0
    CURRENT_APP_HASH=""
    if [[ ! -x "$TARGET_BIN" ]]; then
        NEED_APP_BUILD=1
    else
        CURRENT_APP_HASH="$(compute_app_hash)"
        if [[ -f "$APP_HASH_FILE" ]]; then
            SAVED_APP_HASH="$(cat "$APP_HASH_FILE" 2>/dev/null || true)"
            if [[ "$SAVED_APP_HASH" != "$CURRENT_APP_HASH" ]]; then
                NEED_APP_BUILD=1
            fi
        else
            NEED_APP_BUILD=1
        fi
    fi

    if [[ "$NEED_APP_BUILD" == "1" ]]; then
        if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
            build_with_os_progress "Telos" "Compiling local game binary..." cargo build --bin telos
        else
            build_with_os_progress "Telos" "Compiling release game binary..." cargo build --release --bin telos
        fi
        mkdir -p "$REPO_DIR/target"
        if [[ -z "$CURRENT_APP_HASH" ]]; then
            CURRENT_APP_HASH="$(compute_app_hash)"
        fi
        echo "$CURRENT_APP_HASH" > "$APP_HASH_FILE"
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
