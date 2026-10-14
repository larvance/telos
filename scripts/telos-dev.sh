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
            echo "# $initial_text"
            echo "10"
            "$@" > "$build_log" 2>&1 &
            build_pid=$!
            local pct=15

            while kill -0 "$build_pid" 2>/dev/null; do
                local last_line
                last_line="$(tail -n 1 "$build_log" 2>/dev/null || true)"
                if [[ "$last_line" =~ Compiling[[:space:]]+([a-zA-Z0-9_-]+) ]]; then
                    echo "# Compiling ${BASH_REMATCH[1]}..."
                    pct=$(( (pct + 4) < 92 ? (pct + 4) : 92 ))
                    echo "$pct"
                elif [[ "$last_line" =~ Finished ]]; then
                    echo "# Finalizing binary..."
                    echo "96"
                fi
                sleep 0.2
            done

            wait "$build_pid"
            local exit_code=$?

            if [[ "$exit_code" -eq 0 ]]; then
                echo "100"
                echo "# Launching $title..."
                sleep 0.25
            else
                exit "$exit_code"
            fi
        } | zenity --progress \
            --title="$title" \
            --text="$initial_text" \
            --pulsate \
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

if [[ "$NEED_BUILD" == "1" ]] && command -v cargo >/dev/null 2>&1; then
    if [[ "${TELOS_DEBUG:-0}" == "1" ]]; then
        build_with_os_progress "Telos Dev" "Compiling game binary..." cargo build --bin telos
    else
        build_with_os_progress "Telos Dev" "Compiling release game binary..." cargo build --release --bin telos
    fi
fi

# Immediately open the game
exec "$TARGET_BIN" "$@"
