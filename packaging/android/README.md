# Telos Android Deployment & Packaging

This directory contains the manifest, build scripts, and resource configurations for building Telos on Android devices.

## Architecture

Telos targets Android NDK native activities:
- **Graphics Backend**: Vulkan 1.3 preferred with OpenGL ES 3.2 fallback.
- **Input System**: Multi-touch capacitive input via Winit's `WindowEvent::Touch` and virtual on-screen controls (`apps/telos/src/touch_controls.rs`):
  - Dynamic left-zone virtual joystick for strafe/forward/sprint movement.
  - Right-zone swipe gestures for camera look and pitch/yaw adjustment.
  - Dedicated virtual buttons for Jump (`JMP`), Sneak (`SNK`), Attack/Mine (`HIT`), Place/Use (`USE`), Pause (`||`), and Inventory (`INV`).
  - Hotbar direct tapping for quick item selection.
  - Automatic fallback translation to GUI mouse clicks across all menus and containers.

## Prerequisites

1. **Android NDK**: Version r25c or newer.
2. **Android SDK & Build-Tools**: API level 26 minimum (Android 8.0+), target API 34.
3. **Rust Android Targets**:
   ```bash
   rustup target add aarch64-linux-android x86_64-linux-android
   ```
4. **Cargo NDK / APK Helper**:
   ```bash
   cargo install cargo-ndk
   # or
   cargo install cargo-apk
   ```

## Building

To build an APK for ARM64 (`arm64-v8a`):
```bash
./packaging/android/build-apk.sh aarch64
```

To build for x86_64 emulator:
```bash
./packaging/android/build-apk.sh x86_64
```
