# Jasper UI iOS / iPadOS reference host

This directory is the iOS/iPadOS reference host for the Jasper UI Rust engine: a UIKit window
created by winit, a Metal surface, UIKit lifecycle handling and a small reference scene. One
binary serves both iPhone and iPad; the bundle declares `UIDeviceFamily` `[1, 2]`.

本目录是 Jasper UI Rust 引擎的 iOS/iPadOS 参考宿主：由 winit 创建的 UIKit 窗口、Metal Surface、
UIKit 生命周期处理和一个最小参考场景。iPhone 与 iPad 共用同一二进制，bundle 声明
`UIDeviceFamily` `[1, 2]`。

## Architecture

- `winit`'s iOS backend calls `UIApplicationMain` from `EventLoop::run_app`; the host creates the
  window in `resumed` (`applicationDidBecomeActive`).
- `suspended` (`applicationWillResignActive`) calls `Engine::suspend` and releases the drawable,
  device, queue and window; the saved successor generation is reused on the next activation.
- `ios/src/model.rs` holds the platform-independent rules: lifecycle state machine, viewport
  arithmetic with safe-area insets, and primary-contact handoff. They are unit-tested on the build
  host so the rules are checked without a Simulator runtime.
- Touch input arrives as winit `Touch` events and is normalized by `DesktopInput`; the host tracks
  the stable primary contact and forwards Down/Move/Up/Cancel to `Engine::pointer`.
- The host owns the `CAMetalLayer` surface, the wgpu Metal device/queue and `present`; the engine
  only paints into the host texture.
- iPad safe area: `Viewport` accepts physical size, scale factor and UIKit insets and derives the
  logical content rectangle used for the root scene node. The current adapter has no UIKit
  inset query yet, so the host passes the insets given by `--insets` (zero by default).
- Fonts: the same ordered stack as Android/Windows and the macOS host — `DejaVuSans` then
  `DroidSansFallbackFull` — so the Chinese title, line and `激活 Activate` button render with real
  glyphs. `--cjk-tofu` drops the fallback face as a control.

## Build

Requirements: macOS 11+, Xcode with the iOS SDK, Rust 1.89+ with the Apple targets:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

Bundles:

```bash
bash ios/build-app.sh simulator     # ad-hoc signed, installable in a Simulator runtime
bash ios/build-app.sh device        # signed only when a code-signing identity exists
bash ios/build-app.sh device --sign "Apple Development: ..."
```

Outputs:

```text
ios/target/app/simulator/JasperUI.app
ios/target/app/device/JasperUI.app
```

The build script substitutes `Info.plist.in` (`CFBundleSupportedPlatforms`, `DTPlatformName`,
`DTSDKName`), links a modern `LC_BUILD_VERSION` platform load command (`IOS` /
`IOSSIMULATOR`, minimum 15.0) and verifies the signature it applied.

Install and launch (requires an installed Simulator runtime):

```bash
xcrun simctl boot <UDID>
xcrun simctl install <UDID> ios/target/app/simulator/JasperUI.app
xcrun simctl launch --console-pty <UDID> org.jasper.ui.ios --args "" --frames 2
```

On a device, the bundle must be signed by a development identity with a matching provisioning
profile; an unsigned or ad-hoc device bundle installs nowhere.

Command line (arguments reach the app through `simctl launch --args` on Simulator, or through the
scheme/Xcode arguments on device):

```text
jasper-ui FONT [--frames N] [--readback out.ppm] [--insets top,right,bottom,left]
               [--size W,H] [--tap X,Y] [--timeout-ms N] [--cjk-font PATH] [--cjk-tofu]
```

## Verification

```bash
bash scripts/verify-ios-app.sh
```

| Layer | What runs |
|---|---|
| Static bundle checks | `plutil` declarations, `UIDeviceFamily [1,2]`, `vtool` platform, `UIKit`/`Metal` linkage, `codesign --verify --deep --strict`, SHA-256 of binary and plist |
| Portable rules | `cargo test --manifest-path ios/Cargo.toml` (7 tests: lifecycle, duplicate notifications, termination, background resume, iPad safe area, invalid insets, primary contact handoff) |
| Adapter exercise | `--features host-on-macos` compiles the identical host module into a macOS build and runs the iPhone and iPad viewport profiles, including a tapped activation |
| Runtime | Simulator install + launch, only when a Simulator runtime is installed |

`host-on-macos` is a development aid: it exercises the host's window, surface, viewport, scene and
input code on macOS. It is not an iOS runtime and does not cover UIKit lifecycle delivery, device
security, or real device metrics.

## Known limits

- The verification host has no Simulator runtime installed and no code-signing identity, so iOS
  runtime launch is **not** verified here; only build, link, bundle, signature and the portable
  rules are.
- The adapter does not query `UIView.safeAreaInsets` yet; insets are host-supplied.
- No iOS accessibility bridge (AccessKit has no iOS adapter in the pinned version).
- No live software keyboard / IME composition round trip.
- No Metal device-loss handling and no thermal/power device matrix.
- No App Store packaging, icons, storyboards or provisioning automation.
