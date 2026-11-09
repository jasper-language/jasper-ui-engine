# Jasper UI Android reference host

This directory is the Android `cdylib`/NativeActivity host for the Jasper UI Rust engine. It is a
reference platform adapter, not a Java application framework and not part of the default engine
core.

本目录是 Jasper UI Rust 引擎的 Android `cdylib`/NativeActivity 参考宿主；它不是 Java
应用框架，也不进入默认引擎核心。

## Architecture

- `winit` Android NativeActivity provides the window and input lifecycle.
- The host owns the wgpu Surface/device/queue and presents through wgpu.
- The engine receives only `Scene`, `ScenePatch`, and normalized pointer events.
- GPU rendering uses the engine's host-neutral TextureView path.
- The APK embeds DejaVu Sans plus Droid Sans Fallback as an explicit host-ordered font stack:
  Latin text uses DejaVu Sans, while CJK text falls back to Droid Sans Fallback.
- Android Background/Home calls `Engine::suspend`; foreground calls `Engine::resume` with the
  successor Surface generation.
- The Rust core remains independent of Android and can still build for Linux, Windows, macOS,
  Android, and iOS targets.

## Build requirements

- Android NDK 27.2.12479018 at `/usr/lib/android-sdk/ndk/27.2.12479018`
- Android SDK Platform 35
- Android Build Tools 35.0.0 (`aapt2`, `zipalign`, `apksigner`)
- Rust target `aarch64-linux-android`
- `zip` and Java `keytool`

The build script uses minSdk 24 and targetSdk 35. It generates a debug signing key under
`target/apk/`; no release key is created or committed.

```bash
rustup target add aarch64-linux-android
bash android/build-apk.sh
```

Output:

```text
android/target/apk/jasper-ui.apk
```

Install and launch:

```bash
adb install -r android/target/apk/jasper-ui.apk
adb shell am force-stop org.jasper.ui
adb shell am start -n org.jasper.ui/android.app.NativeActivity
adb logcat -s JasperUI:V
```

## Device acceptance

The current reference test device is:

```text
model: C66
Android: 11
ABI: arm64-v8a
surface: 720x1344 content area
```

Validated on this device:

- NativeActivity launch and full-screen GPU frame
- 70% width, 180 logical-pixel button rendered through Blink/Blitz layout
- UTF-8 Chinese button label `中文 激活` rendered through the CJK fallback font
- Touch Down/Move/Up routed through the public pointer facade
- Tap activation and versioned style patch
- Home → foreground suspend/resume with the returned successor Surface generation
- Portrait/landscape viewport relayout
- APK signature v2/v3 verification

The host currently does not claim a real external IME, Android accessibility service, process
isolation, mobile multi-window, or hardware-independent temperature/power validation. Those are
separate integrations or device matrices. Chinese here means host-owned CJK font fallback and
rendered glyph verification, not a complete locale pack or system font discovery service.
