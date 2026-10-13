#!/usr/bin/env bash
# ==============================================================================
# Telos Android APK Packaging Script
# Compiles the client for Android NDK targets and builds an installable APK.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_ARCH="${1:-aarch64}" # Options: aarch64, armv7, x86_64

case "${TARGET_ARCH}" in
    aarch64|arm64|arm64-v8a)
        RUST_TARGET="aarch64-linux-android"
        ANDROID_ABI="arm64-v8a"
        ;;
    x86_64|x64)
        RUST_TARGET="x86_64-linux-android"
        ANDROID_ABI="x86_64"
        ;;
    armv7|armeabi-v7a)
        RUST_TARGET="armv7-linux-androideabi"
        ANDROID_ABI="armeabi-v7a"
        ;;
    *)
        echo "Error: Unsupported architecture '${TARGET_ARCH}'. Use aarch64, x86_64, or armv7."
        exit 1
        ;;
esac

echo "=========================================================="
echo "  Building Telos Android APK"
echo "  Target Architecture: ${TARGET_ARCH} (${RUST_TARGET})"
echo "=========================================================="

# Check for cargo-ndk or cargo-apk
if command -v cargo-ndk &>/dev/null; then
    BUILD_TOOL="cargo-ndk"
elif command -v cargo-apk &>/dev/null; then
    BUILD_TOOL="cargo-apk"
else
    echo "Notice: Neither 'cargo-ndk' nor 'cargo-apk' is installed."
    echo "To install: cargo install cargo-ndk"
    echo "Falling back to standard target check..."
    BUILD_TOOL="cargo"
fi

BUILD_DIR="${WORKSPACE_ROOT}/target/android-build"
mkdir -p "${BUILD_DIR}/lib/${ANDROID_ABI}" "${BUILD_DIR}/assets"

echo "-> Compiling client shared library for ${RUST_TARGET}..."
if [ "${BUILD_TOOL}" = "cargo-ndk" ]; then
    cargo ndk -t "${ANDROID_ABI}" build --release --bin telos
elif [ "${BUILD_TOOL}" = "cargo-apk" ]; then
    cargo apk build --release
    exit 0
else
    cargo build --target "${RUST_TARGET}" --release --bin telos
fi

echo "-> Assembling asset package..."
if [ -d "${WORKSPACE_ROOT}/assets" ]; then
    cp -r "${WORKSPACE_ROOT}/assets/"* "${BUILD_DIR}/assets/" 2>/dev/null || true
fi
if [ -d "${WORKSPACE_ROOT}/shaders" ]; then
    cp -r "${WORKSPACE_ROOT}/shaders" "${BUILD_DIR}/assets/" 2>/dev/null || true
fi

echo "=========================================================="
echo "  Android build prepared successfully."
echo "  Build staging directory: ${BUILD_DIR}"
echo "=========================================================="
