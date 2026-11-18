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

## 显式宿主端口

`core_host` 只定义调用边界，不执行 I/O：

- `CoreClock::now_millis`：宿主单调时钟，单位毫秒；
- `CoreFontSource::font_bytes(id)`：宿主认证、持有并借用字体字节；
- `CoreImageSource::image_bytes(id)`：宿主认证并借用编码图片字节；
- `CoreFileSource::file_bytes(id)`：宿主认证并借用只读文件或内嵌资源字节。

返回 `None` 表示宿主不可用或拒绝，不产生路径、系统字体、IPC 或文件系统调用。
图片解码仍在 std image/SVG 特性中；bare-metal 宿主若需要图片，必须先自行解码，
再通过 `RenderBackend::submit_rgba8` 提交 RGBA8 帧。

CPU RGBA 呈现最小契约复用现有 `RenderBackend`：

- `submit_rgba8(width, height, Vec<u8>)`：接管恰好 `width * height * 4` 字节；
- `present()`：只在成功接管后由宿主呈现；
- `HostInbox<UiBatch>`：对宿主消息执行会话/epoch/顺序/容量准入；
- `Scene` / `ScenePatch`：语义场景和原子补丁输入。

`CoreRgbaRenderer::render(&Scene)` 提供最小 Scene → CPU RGBA8 路径：输出
`CoreRgbaFrame { width, height, pixels, scene_version }`，像素为紧密行优先 RGBA8。
它只使用布局、绘制和内存 Canvas，不创建窗口、Surface、GPU 设备或系统资源。宿主仍负责
把该帧复制/提交到自己的显示缓冲并调用 `present` 语义。

渲染前会拒绝非有限、非正、超过 65535 像素的单轴尺寸，以及超过默认 64 MiB 的紧凑 RGBA8
帧；宿主可通过 `CoreRgbaRenderer::with_max_frame_bytes` 设置自己的更严格额度。超限返回
`InvalidFrameSize` 或 `FrameBudgetExceeded`，不会进入 Canvas 分配。

`CoreRgbaRenderer::render_to_backend(&Scene, &mut RenderBackend)` 直接把帧交给宿主后端：
`submit_rgba8` 成功后才会调用 `present`；准入/接管失败保持无呈现。

这些是 Rust 调用边界，不是正式 ABI 或 Jasper-OS syscall 定义。

## 不在 Core

- `HostRenderer`、Blitz/Parley、AccessKit 平台适配、JUI 2 宿主传输、winit/softbuffer；
- GPU/wgpu/Vello、图片解码/SVG 光栅、文件保存和系统字体发现；
- 系统时钟、IME 服务、平台输入、窗口 Surface/present、unreachable host IO；
- 正式 ABI、Jasper-OS syscall 适配或跨目标运行验收。

宿主需要自己实现存储、时钟、字体字节注入、资源读取和呈现。当前 core 是纯 Rust
数据/状态与最小 RGBA 软件路径，不直接 syscall。

## 验证状态

- bare-metal compile：通过；
- std 默认库测试：`186 passed`；
- std `--all-features`：库测试 `296 passed, 1 ignored`，全部集成测试与 doc-test 通过；
- 帧尺寸/预算准入、`UiKey` generation 后继、事件权限分类和 authority 优先级已有
  Verus 实现级证明（2/0、3/0、3/0、3/0、2/0，负向控制均被拒），并有独立 Proptest、
  Miri 定向用例和 read-boundary fuzz 覆盖；
- 未在 Jasper-OS 用户态/内核中链接运行，未证明目标端 allocator、panic handler、
  vector/float ABI、CPU 特性、中断时钟或真实 Surface 呈现。
