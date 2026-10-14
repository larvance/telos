# Telos iOS Deployment & Packaging

This directory contains the application manifest, packaging scripts, and asset catalog configurations for building and running Telos on iOS devices (iPhone, iPad) and the iOS Simulator.

---

## 1. Architecture

Telos runs on iOS using native UIKit / Winit windowing and touch controllers:
- **Graphics Backend**:
  - **Metal via MoltenVK**: Translates Vulkan 1.3 calls into Apple Metal commands.
  - **OpenGL ES 3.2**: Secondary fallback path for testing.
- **Display Pacing & ProMotion**:
  - `CADisableMinimumFrameDurationOnPhone: true` enables full 120 Hz ProMotion variable refresh rate on compatible iPhones and iPads.
- **Input & Touch Controls**:
  - Multi-touch tracking via Winit's `WindowEvent::Touch` and [`touch_controls.rs`](file:///home/usr/Projects/voxel/apps/telos/src/touch_controls.rs).
  - Floating left-thumb virtual joystick for direction, sneaking, and sprinting.
  - Right-side swipe gestures for camera look and pitch/yaw adjustment.
  - On-screen touch buttons for Jump, Sneak, Hit, Use, Pause, and Inventory.
  - Direct hotbar slot selection with pixel-perfect physical snapping.
- **Application Lifecycle Management**:
  - `suspended`: Pauses gameplay, releases mouse/cursor lock, and coordinates world autosaves when the app moves to the background.
  - `resumed`: Re-activates display rendering and requests redraw upon foreground transition.
  - `memory_warning`: Responds to OS memory pressure by purging non-critical staging allocations.

---

## 2. Prerequisites

1. **Rust Apple Targets**:
   ```bash
   # Physical iOS devices (arm64):
   rustup target add aarch64-apple-ios

   # iOS Simulator on Apple Silicon (M1/M2/M3/M4):
   rustup target add aarch64-apple-ios-sim

   # iOS Simulator on Intel x86_64:
   rustup target add x86_64-apple-ios
   ```

2. **Xcode**: Version 14 or newer with command line tools (`xcode-select --install`).

3. **MoltenVK** (for Vulkan-on-Metal):
   - Download the latest Vulkan SDK for macOS from [LunarG](https://vulkan.lunarg.com/sdk/home) or build MoltenVK from source.
   - Set the environment variable:
     ```bash
     export MOLTENVK_DIR=/path/to/VulkanSDK/macOS/Frameworks/MoltenVK.framework
     ```

---

## 3. Building the iOS Application

To build and package the `.app` bundle and `.ipa`:

### A. Physical Device (ARM64)
```bash
./packaging/ios/build-ios.sh aarch64
```

### B. iOS Simulator (Apple Silicon)
```bash
./packaging/ios/build-ios.sh sim
```

### C. iOS Simulator (Intel)
```bash
./packaging/ios/build-ios.sh sim-x86_64
```

The resulting application bundle will be created at:
```
target/ios-build/Payload/Telos.app
target/ios-build/Telos.ipa
```

---

## 4. Running on iOS Simulator

To boot and test on an active simulator instance:

```bash
# 1. Boot simulator
xcrun simctl boot "iPhone 15 Pro"

# 2. Install application
xcrun simctl install booted target/ios-build/Payload/Telos.app

# 3. Launch application
xcrun simctl launch booted com.larvance.telos
```

---

## 5. Device Code Signing & Deployment

For deployment to physical iOS devices:
1. Open Xcode and configure an iOS Development Team signing profile.
2. Sign the bundle with your developer identity:
   ```bash
   codesign -f -s "Apple Development: Your Name (XXXXXXXXXX)" \
       --entitlements packaging/ios/Telos.entitlements \
       target/ios-build/Payload/Telos.app
   ```
3. Deploy via Xcode or `ios-deploy`:
   ```bash
   ios-deploy --bundle target/ios-build/Payload/Telos.app
   ```
