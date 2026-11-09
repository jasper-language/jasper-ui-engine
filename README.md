# jasper-ui-engine

> Current direction: migrate the UI execution layer to pinned Blitz. The existing
> hand-assembled engine and optional Parley adapter are retained for migration
> comparison; they are not the target architecture. The root crate now offers
> `engine::Engine` behind the application-facing `native-engine` feature (CPU
> painting: `native-engine-cpu`). Those names forward to the unchanged
> `native-blitz-pending` implementation gate. Functional hardening and basic
> admission/integration evidence are delivered; the remaining capacity-limited
> and live-platform proofs are tracked for follow-up.
> `experiments/blitz-host` consumes that root runtime instead of compiling a
> second adapter.
> Native execution requires an explicit feature; old execution is available as LegacyEngine only. Canvas remains deferred.

Jasper UI 的 **宿主无关 Native 引擎底座**（Rust）。它只负责底层布局、文字测量、场景/绘制、UiBatch、软件光栅和后端接口；不包含 Jasper UI 组件、状态、主题、路由或 Web 运行时。

> 状态：**v0.1 原型底座**，用于宿主/图形界面集成实验；**API 会改，不是稳定 ABI**。
> 这是可运行的工程，不是完整引擎；未实现的能力会明确报错/标注，不用空实现或固定结果冒充完成。
> 详见 [docs/status.md](docs/status.md)。

技术文档索引：[docs/README.md](docs/README.md)：
设计 [design.md](docs/design.md)、API [api.md](docs/api.md)、ABI 草案 [abi.md](docs/abi.md)、
依赖 [dependencies.md](docs/dependencies.md)、路线图 [roadmap.md](docs/roadmap.md)、
宿主集成 [integration.md](docs/integration.md)、体积 [size.md](docs/size.md)。

## 为什么独立成仓库

- 引擎需要同时服务两类宿主：
  - **内置宿主**：由平台把引擎作为系统 UI 服务提供，应用不携带引擎。
  - **自带宿主**：应用像 Flutter 一样把引擎/框架一起打包。
- 引擎不能假设 POSIX、窗口系统或 GPU 一定存在；因此平台/窗口/GPU/文字/渲染都通过 trait 隔离。
- Jasper UI Framework、UI IR、Web 后端归 Jasper 侧；本仓库只做 Rust 原生执行层。

## v0.1 已实现

- 几何、颜色、PSS 子集（Flex/Block 语义、尺寸、间距、对齐、边框）
- 场景树（稳定 `NodeId`）与增量 `ScenePatch`
- **Taffy 布局适配**（Flexbox / Block（Grid 尚未映射到公开样式）；百分比、min/max、padding/margin/gap）
- 文字测量 trait + 参考实现（近似度量，真实排版后续接 Parley）
- 可选真实文本测量：`--features real-text` 使用 cosmic-text 的系统字体、shaping 与断行
- 绘制显示列表（Fill/Stroke/Text/Clip）
- `UiBatch` 二进制编码/解码（带版本、边界检查）
- 软件光栅（RGBA、填充、描边、裁剪、alpha 混合、PPM 输出）
- `RenderBackend` / `PlatformHost` trait + Headless 后端
- 统一指针事件模型：鼠标 / 触摸 / 触控笔（多点、压力、倾斜、扭转、接触面积、笔擦）
- **命中测试 + 指针路由**：布局树命中、root→target 路径、按 pointer 的 capture
- **语义树 + 焦点管理**：角色/标签/值/禁用/可聚焦，Tab/Shift+Tab 焦点遍历；可选
  AccessKit 转换与 winit 平台适配（Linux AT-SPI 已实测）
- **场景版本 + 原子批量补丁**：成功才推进版本；过期批次拒绝；失败不半应用
- **参考手势层**：tap / long-press / pan / pinch / cancel，接在指针路由/capture 上
- **溢出裁剪 + 滚动偏移**：`Overflow::Hidden/Scroll` 裁剪子树；`Node::scroll` 平移子节点
- **滚动控制**：滚轮 delta 沿祖先链逐级消费、残余拆分、边界 clamp、fling 惯性；滚动条几何/命中/拖动/绘制
- **Canvas 组件**：记录式绘制指令（矩形/线/文字/裁剪），参与布局与内容裁剪；CPU 渲染走 `tiny-skia`
- **GPU 选择策略**：自动/集成/独立/指定适配器/纯软件，支持软件回退与运行时切换规划
- **桌面输入**：`--features desktop` 把 winit 鼠标/键盘/滚轮/IME/Focus 规范化为引擎事件
- **桌面窗口**：`--features desktop,native-engine-cpu --example window -- FONT [--once]` 使用根原生 CPU 候选，经宿主持有的 winit/softbuffer 呈现；Xvfb 下已实测一帧、缩放和点击，硬件/平台生命周期仍待验收
- **宿主渲染生命周期**：`native::host::HostRenderer` 绑定尺寸/DPI/Surface 代数，协调暂停、恢复、退休和 GPU present 确认；见 [host-rendering.md](docs/host-rendering.md)
- **GPU 选择**：`--features gpu` 用 wgpu 枚举真实适配器，套用自动/集成/独立/指定/纯软件策略
- **macOS 参考宿主**：Cocoa winit 窗口 + Metal `CAMetalLayer` Surface + AccessKit Cocoa 适配器；
  Retina 2x、drawable 读回、`AXPress` 与真实 CGEvent 点击、CJK 字体注入已在 Apple M2 真机实测
- **iOS/iPadOS 参考宿主**：winit UIKit 窗口 + Metal Surface，同一二进制覆盖 iPhone 与 iPad
  （`UIDeviceFamily [1,2]`）；完成交叉链接、`.app` 打包/签名静态验证与可移植规则单测，
  运行验收仍缺模拟器运行时或签名身份
- 正式原生流水线：场景更新 → Blitz 树/样式/布局/事件 → Parley/Vello → 宿主后端或 Surface
- 旧兼容流水线：场景 → Taffy 布局 → PaintOp → UiBatch → 后端，仅由 `LegacyEngine` 显式调用
- 测试与可运行示例

## v0.1 明确未实现

- 真实文字光栅化（`DrawText` 在 Headless 后端返回 `Unsupported`；需要 Parley/字体后端）
- 移动端系统无障碍、Windows 物理 GPU/vsync、图片/SVG 解码；Ubuntu 已完成硬件 GPU、
  device-loss 恢复演练与 AT-SPI 桥实测，Windows 10 VM 已完成 UIA 与 wgpu 生命周期演练，
  macOS 已完成 Cocoa 无障碍与 Metal 呈现实测
- Jasper ABI、Jasper UI Framework、UI IR、状态/事件/动画/路由
- Web HTML/CSS/JS/WASM 输出
- 完整文本编辑与 IME 往返、过冲回弹、系统无障碍桥（文本/组合事件映射与焦点路由已有实现）
- 这些缺口在 [docs/status.md](docs/status.md) 里逐项标注，不写成“已完成”。

## 快速开始

```bash
cargo test
JASPER_UI_VM_RUN=/path/to/vm-entry.sh scripts/verify-in-vm.sh env CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=experiments/blitz-host/target JASPER_UI_TEST_FONT=/path/in/vm/font.ttc cargo +nightly run --locked --features native-engine-cpu --example hello -- /path/in/vm/font.ttc
# 字体由验证 VM 宿主注入（示例路径用占位符）；生成 hello.ppm，运行原生候选入口，整体验收仍未完成。
```

原生鼠标/视觉更新/停用/退休示例为 `--features native-engine --example pointer -- FONT`，字体同样由 VM 宿主注入。通用按钮的焦点由 Framework 显式请求；该示例不声称触摸或 OS capture。旧触摸/capture 演示仅以 `--example legacy_pointer` 保留迁移对照。

完整公共调用路径可通过
`cargo run --locked --features native-engine-cpu --example full_scene -- FONT OUTPUT`
重复运行。它覆盖声明、版本化视觉更新、布局/CPU 呈现、焦点与指针交互、外部显示快照及预编辑、无障碍树、资源失败和退休，并输出强制重绘的 CPU 中位/P95 耗时。该数据只用于同机回归，不是硬件显示或 GPU 基准。

窗口调用方
`cargo run --locked --features desktop,native-engine-cpu --example window -- FONT [--once|--once-after-activation]`
现将指针、滚轮、命令键、IME 预编辑/提交和失焦恢复送入同一公共入口；点击触发的视觉补丁保留焦点，窗口缩放和 Surface 失败在宿主层报告。外部文本仍是同步注入的服务夹具，不宣称连接了真实编辑器/IME 服务。
`scripts/interact-window-xvfb.sh FONT OUT_DIR` 用 Xvfb+xdotool 执行真实 X11 点击，并要求窗口进程以 `activated=1` 和已完成退休退出。

Linux 平台无障碍参考宿主：

```bash
cargo run --locked --features desktop-accessibility --example window -- FONT --once
```

该特性通过 AccessKit 窗口适配器连接 AT-SPI；相同的可选适配器在 Windows 选择 UIA，
macOS/Android/iOS 选择各自平台实现。默认核心构建不会依赖这些平台 API。

双窗口参考宿主：

```bash
cargo run --locked --features desktop,native-engine-cpu --example multiwindow -- FONT --once
```

两个窗口各自持有独立 owner、场景、Surface 代数和呈现后端；窗口编排仍归宿主所有。

Android ARM64 参考宿主：

```bash
bash android/build-apk.sh
adb install -r android/target/apk/jasper-ui.apk
```

Android 使用 NativeActivity、winit、wgpu GLES 和正式 `HostRenderer`；构建与真机验收限制见
[android/README.md](android/README.md)。

Windows 参考宿主的构建、字体栈和 VM 验收步骤见
[windows/README.md](windows/README.md)。Windows 示例接受一个有序字体文件列表，以同时覆盖
拉丁与 CJK：

```powershell
window.exe DejaVuSans.ttf C:\Windows\Fonts\msyh.ttc
```

macOS 参考宿主：

```bash
bash macos/build-app.sh
bash scripts/verify-macos-host.sh
```

macOS 使用 Cocoa 窗口、Metal Surface、AccessKit Cocoa 适配器；构建、命令行与验收限制见
[macos/README.md](macos/README.md)。

iOS / iPadOS 参考宿主：

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
bash ios/build-app.sh simulator
bash scripts/verify-ios-app.sh
```

同一二进制覆盖 iPhone 与 iPad；当前只完成构建、链接、bundle/签名静态验证与可移植规则单测，
运行验收需要模拟器运行时或设备签名身份，见 [ios/README.md](ios/README.md)。

GPU 设备丢失恢复演练：

```bash
JASPER_UI_PRESENT_MODE=fifo \
  cargo run --locked --features desktop,gpu-pending --example gpu_window -- FONT --once --exercise-device-loss
```

默认选择宿主支持的 `Fifo` present mode；演练沿真实宿主管恢复生命周期重建设备与 Surface，
但不会把合成的恢复事件冒充驱动诱发的硬件丢失。

要求 Rust 1.89+（以 `Cargo.toml` 为准）。

## 架构

```text
Jasper UI Framework / UI IR
              |
        typed Scene / SceneUpdate
              |
  engine::Engine (native-engine feature)
    ScenePatch -> Blitz DOM/style -> Blitz layout/events
                              |
                     Parley text -> Vello CPU/GPU
                              |
                RenderBackend / host Surface

LegacyEngine (explicit compatibility only)
    scene -> Taffy layout -> PaintOp -> UiBatch -> backend
```

- `scene`：当前实例树，稳定 `NodeId`；不是 Jasper 声明层。
- `Engine`：正式原生入口；先把完整更新帧应用到来源副本，再从终态构建独立 Blitz 候选，成功后单次替换。
- `Blitz`：负责原生树、样式、布局、焦点和事件；应用不接触 Blitz/DOM/CSS 类型。
- `Parley` / `Vello`：共享文字布局和 CPU/GPU 绘制，经宿主注入字体、Surface 与设备。
- `RenderBackend`：接收完整原生 RGBA8 帧，成功提交后才呈现；不支持的后端明确报错。
- `LegacyEngine`：仅供显式迁移兼容；活动原生示例和外部调用方不经过 Taffy→PaintOp→UiBatch 执行链。
- `platform`：窗口/输入/DPI 能力入口；内置宿主和自带宿主分别实现。

## 触摸、多点触控与触控笔

`src/input.rs` 的 `PointerEvent` 从第一天就带：

- `pointer_id`（多点触控/多笔区分）
- `kind`：Mouse / Touch / Pen / Unknown
- `phase`：Down / Move / Up / Cancel / Hover
- `pressure`、`tilt`、`twist`、`contact`（接触面积）
- `is_primary`、`eraser`、`button`、`modifiers`

触控笔与触摸不是“鼠标模拟”：平台适配层负责把 Windows Ink / Apple Pencil / Android stylus / libinput 的原始数据规范化；Jasper 层负责手势语义。参考手势识别与手掌拒绝已有原型；预测/合并事件、真实平台触控/笔覆盖和完整 IME 编辑闭环尚待实现与验证。

## 宿主矩阵

| 宿主 | 引擎来源 | 平台层 | 说明 |
|---|---|---|---|
| 桌面/移动（自带） | 应用携带 | winit 示例/平台适配（未完成平台验收） | 类似 Flutter |
| 内置系统宿主 | 系统 UI 服务提供 | 系统通道/合成器（后续） | 应用不携带引擎 |
| Web | 不使用本引擎 | 浏览器 DOM/JS/WASM | 由 Jasper Web 后端负责 |

本仓库的 trait 设计保证：**内置宿主与自带宿主共用同一套引擎核心，但平台/窗口/渲染实现可替换**。
内置宿主可用 `host_transport::HostInbox` 统一约束会话 epoch、精确序号和有界背压；
它只定义宿主中立传输语义，不实现系统 IPC。

无窗口的内置宿主参考：

```bash
cargo run --locked --features native-engine-cpu --example host_loopback -- FONT OUTPUT.ppm
```

该示例经 `HostInbox` 消费场景更新、CPU 帧与退休命令，不链接 winit/wgpu。

## 许可证

Apache-2.0，见 [LICENSE](LICENSE)。依赖许可证在 [docs/status.md](docs/status.md) 记录；新增依赖先审计再锁定版本。

## 交付要求与当前核查

- 系统内置宿主与应用自带宿主是一等目标；系统桌面依赖 UI，不能把内置模式仅当成后续移植。内置路径不得强制依赖 winit、POSIX、GPU 或系统字体目录；当前系统服务适配尚未实现。
- 只确定 Jasper 的 `jasper.ui.*` 命名空间，组件子包名称未冻结。Rust 不复制另一套组件/主题语义。
- 数学形式化证明是关键路径的硬性交付门槛；现有单元测试不构成数学证明。当前证明状态、义务与信任边界见 [verification.md](docs/verification.md)。
- 现有 Rust `UiBatch` 是绘制批，不是完整 UI 语义更新协议；见 [batch-boundaries.md](docs/batch-boundaries.md)。
- [status.md](docs/status.md) 是能力核查入口；测试、模型检查、实现级证明分开记录。
