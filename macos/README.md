# Jasper UI macOS reference host

This directory is the macOS reference host for the Jasper UI Rust engine: a Cocoa window, a
Metal surface, the AccessKit Cocoa accessibility adapter and a small reference scene. It is a
platform adapter, not a macOS application framework, and it is not part of the default engine
core.

本目录是 Jasper UI Rust 引擎的 macOS 参考宿主：Cocoa 窗口、Metal Surface、AccessKit Cocoa
无障碍适配器和一个最小参考场景。它是平台适配器，不是 macOS 应用框架，也不进入默认引擎核心。

## Architecture

- `winit` provides the Cocoa window, Retina scale factor and normalized input events.
- The host owns the `CAMetalLayer` surface, the wgpu Metal device/queue and `Queue::present`.
- The engine receives only owned `Scene`/`ScenePatch` values, normalized pointer/keyboard events
  and surface generations through `HostRenderer` and `GpuRenderer`.
- `Renderer::render_gpu_to_host_view` paints into the host's swapchain texture; the host confirms
  presentation with `gpu_presented` after `Queue::present`.
- `accesskit_winit` selects the macOS (Cocoa) backend. The host creates the adapter before the
  window becomes visible and passes physical-pixel bounds to AccessKit.
- The viewport is physical (`inner_size`) with the logical scale factor forwarded to the engine;
  pointer samples are divided by the scale factor before routing.
- The scene is a dark card with a title line, one Chinese line and one `激活 Activate` text button;
  activation applies a versioned style patch exactly like the Android and desktop reference hosts.
- Fonts: the host injects the same ordered stack as Android/Windows — `DejaVuSans` first,
  `DroidSansFallbackFull` second — so CJK text renders with real glyphs instead of tofu.

## Build

Requirements: macOS 11+, Xcode command line tools, Rust 1.89+.

```bash
bash macos/build-app.sh            # native arm64 release bundle, ad-hoc signed
bash macos/build-app.sh --universal # arm64 + x86_64 merged with lipo
bash macos/build-app.sh --debug     # debug bundle
```

Output:

```text
macos/target/app/Jasper UI.app
```

Run:

```bash
"macos/target/app/Jasper UI.app/Contents/MacOS/jasper-ui" macos/assets/DejaVuSans.ttf
open "macos/target/app/Jasper UI.app" --args "$PWD/macos/assets/DejaVuSans.ttf"
```

Command line:

```text
jasper-ui FONT [--once] [--frames N] [--readback out.ppm] [--immediate]
               [--tap] [--timeout-ms N] [--cjk-font PATH] [--cjk-tofu]
```

- `--once` / `--frames N` exit after the requested number of presented frames.
- `--readback PATH` copies the presented drawable into a PPM and reports exact pixel counts,
  byte size and SHA-256.
- `--tap` injects one host-level activation after the first frame. It is an engine-level pointer
  sequence, not an OS click; `scripts/verify-macos-host.sh` additionally posts a real
  WindowServer click through `CGEvent`.
- `--cjk-font PATH` replaces the bundled CJK fallback face; `--cjk-tofu` drops the fallback face so
  the difference between real glyphs and missing-glyph boxes is measurable.

## Verification

```bash
bash scripts/verify-macos-host.sh
```

The script builds the host, runs it on the real desktop session and checks:

| Check | Evidence |
|---|---|
| Metal frame presented and read back | `stage=frame ... format=Bgra8UnormSrgb`, exact pixel counts |
| AccessKit Cocoa tree | `AXButton "Activate"` with screen position and size |
| AX activation | `AXPress` → `ActionRequested` → `stage=activate` → repaint |
| Real OS pointer input | `CGEvent` click at the AX frame center → `stage=activate` |
| CJK rendering | Chinese line renders real glyphs with an injected CJK font and differs from the tofu control |
| Lifecycle | `retired=true` and a clean process exit |

The verification host is an Apple M2 (8 GB) running macOS 15.7.5 with Xcode 26.3 and the iOS
26.2 SDK. `scripts/verify-macos-host.sh` records the exact session in
`evidence/macos-reference-host/`.

## Known limits

- No Metal device-loss handling: the engine reports device-loss recovery as `Unsupported`.
- No measured vsync deadline, compositor frame pacing or hardware-independent timing claim.
- No live system IME composition round trip; IME events are forwarded when macOS delivers them.
- No automated window resize/occlusion/multi-display exercise; those are host-owned paths.
- No notarization, hardened runtime or App Store distribution; the bundle is ad-hoc signed.
- No x86_64 runtime acceptance (the universal build is compile-time only until run on Intel).
