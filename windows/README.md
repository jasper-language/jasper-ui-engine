# Windows reference-host acceptance

This directory records the reproducible Windows host acceptance setup. The engine and the
`examples/window` caller stay platform-neutral; the Windows VM supplies native winit windows,
DWM, UIA and the VirtualBox display adapter.

## Virtual machine

```text
VirtualBox VM: JasperUIWin10
Windows: Windows 10 Enterprise LTSC 2021 Evaluation, build 19044
CPU: 4 vCPU
Memory: 6144 MB
Disk: 24 GB dynamically allocated VDI
User: jasper
```

The unattended template is `win_nt6_unattended.xml`. It removes the product-key requirement that
blocked evaluation-media installation and preserves the standard Windows evaluation notice.

## Build and transfer

The Windows GNU binary is built in the Linux build VM:

```bash
cargo build --release --locked --target x86_64-pc-windows-gnu \
  --features desktop-accessibility --example window
```

The build VM uses `gcc-mingw-w64-x86-64` for `dlltool` and linking. Copy the resulting
`window.exe` into `C:\Users\jasper\`, together with the test resources. The Windows host accepts
an ordered font list:

```powershell
C:\Users\jasper\window-uia.exe `
  C:\Users\jasper\DejaVuSans.ttf `
  C:\Windows\Fonts\msyh.ttc
```

The first font supplies Latin text; Microsoft YaHei supplies CJK fallback. The scene label is
`激活 Activate`, so the screenshot proves both scripts reach the same shaped text run. Chinese is
UTF-8 in Rust and never crosses a GBK conversion boundary.

## Acceptance harness

- `show-window.ps1` restores, positions and focuses a reference window.
- `uia-inspect.ps1` enumerates the native UI Automation tree and optionally invokes a named
  button.
- `click-at.ps1` sends a physical pointer click to a desktop coordinate.

Run the UIA assertion with:

```powershell
powershell -ExecutionPolicy Bypass -File C:\Users\jasper\uia-inspect.ps1 `
  -ProcessName window-uia -InvokeActivate
```

`uia-inspect.ps1` sets PowerShell input, output and console encodings to UTF-8 and prints the
button name's UTF-16 codepoints. This separates the scene's actual Unicode text from console-code
page rendering. In particular, `激活` is verified as `U+6FC0 U+6D3B`, not as replacement
characters.

The acceptance covers single-window CPU presentation, two independent windows, suspend/resume,
window resize, 100% and 150% DPI, UIA enumeration/invocation, and CJK fallback. VirtualBox's
adapter is not treated as a physical-GPU/vsync claim; it selected the DX12
`Microsoft Basic Render Driver`, and the Ubuntu RTX 3060 run remains the hardware GPU evidence.
