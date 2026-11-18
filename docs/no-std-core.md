# `no-std-core`

状态日期：2026-11-18。

`no-std-core` 是 Rust UI Engine 的 `no_std + alloc` 最小核心构建。它用于不提供系统
文件、时钟、字体目录、窗口或 GPU 的嵌入式/OS 宿主；不代表 Blitz Native 适配器、
Vello 渲染器或桌面窗口已在无操作系统环境中运行。

## 启用

```toml
jasper-ui-engine = { git = "...", rev = "<commit>", default-features = false, features = ["no-std-core"] }
```

验证目标：

```bash
scripts/check-no-std-core.sh
```

脚本默认锁定 `nightly-2026-10-17` 与 `x86_64-unknown-none`，也可用
`JASPER_NO_STD_TOOLCHAIN` / `JASPER_NO_STD_TARGET` 显式覆盖；运行前要求目标已安装，
不会联网修改用户工具链。输出包含实际 rustc 版本及 `Cargo.toml` / `Cargo.lock`
的 SHA-256。

`no-std-core` 是模式特性。它与 `desktop`、`gpu*`、`native-*`、`accesskit-pending`、
`real-text`、`canvas-tiny-skia`、`ui-plugin-*`、Parley/Vello text/render 等 std 宿主特性
组合时，构建脚本选择普通 std 路径，以便 `--all-features` 仍可编译和测试；bare-metal
集成必须使用 `--no-default-features --features no-std-core`。

## Core 能力

- `UiBatch`、`Scene`、`ScenePatch`、原子批事务、命中、布局、滚动、语义与焦点基础；
- `UiKey`、输入绑定票据、ResourceGate、Surface 生命周期、overlay 纯状态；
- Typed UI IR v0.1 的严格 JSON/Schema Rust consumer、identity ledger、事件准入和
  Rust Native Scene 映射数据结构；
- 无状态 CPU raster/Canvas、RGBA Surface 行检查、输入规范化和 UI plugin gate；
- `alloc` 字符串、容器和 serde JSON；浮点 `floor/ceil/round/sqrt/pow/fract/trig`
  通过 `libm`，不依赖 libc/libm 系统库。

## 不在 Core

- `HostRenderer`、Blitz/Parley、AccessKit 平台适配、JUI 2 宿主传输、winit/softbuffer；
- GPU/wgpu/Vello、图片解码/SVG 光栅、文件保存和系统字体发现；
- 系统时钟、IME 服务、平台输入、窗口 Surface/present、unreachable host IO；
- 正式 ABI、Jasper-OS syscall 适配或跨目标运行验收。

宿主需要自己实现存储、时钟、字体字节注入、资源读取和呈现。当前 core 是纯 Rust
数据/状态与最小 RGBA 软件路径，不直接 syscall。

## 验证状态

- bare-metal compile：通过；
- std 默认库测试：`180 passed`；
- std `--all-features` 编译、运行时测试与既有宿主证据继续保留；
- 未在 Jasper-OS 用户态/内核中链接运行，未证明目标端 allocator、panic handler、
  vector/float ABI、CPU 特性、中断时钟或真实 Surface 呈现。
