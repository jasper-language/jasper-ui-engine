# jasper-ui-engine

> Current direction: migrate the UI execution layer to pinned Blitz. The existing
> hand-assembled engine and optional Parley adapter are retained for migration
> comparison; they are not the target architecture. The root crate now offers
> `engine::Engine` behind `native-blitz-pending` (CPU painting:
> `native-blitz-cpu-pending`). `experiments/blitz-host` now consumes that root
> runtime instead of compiling a second adapter.
> This entry is not yet accepted. Native execution requires an explicit feature; old execution is available as LegacyEngine only. Canvas remains deferred.

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
- **语义树 + 焦点管理**：角色/标签/值/禁用/可聚焦，Tab/Shift+Tab 焦点遍历（AccessKit 后续）
- **场景版本 + 原子批量补丁**：成功才推进版本；过期批次拒绝；失败不半应用
- **参考手势层**：tap / long-press / pan / pinch / cancel，接在指针路由/capture 上
- **溢出裁剪 + 滚动偏移**：`Overflow::Hidden/Scroll` 裁剪子树；`Node::scroll` 平移子节点
- **滚动控制**：滚轮 delta 沿祖先链逐级消费、残余拆分、边界 clamp、fling 惯性；滚动条几何/命中/拖动/绘制
- **Canvas 组件**：记录式绘制指令（矩形/线/文字/裁剪），参与布局与内容裁剪；CPU 渲染走 `tiny-skia`
- **GPU 选择策略**：自动/集成/独立/指定适配器/纯软件，支持软件回退与运行时切换规划
- **桌面输入**：`--features desktop` 把 winit 鼠标/键盘/滚轮/IME/Focus 规范化为引擎事件
- **桌面窗口**：`--features desktop,native-blitz-cpu-pending --example window -- FONT [--once]` 使用根原生 CPU 候选，经宿主持有的 winit/softbuffer 呈现；新路径实际窗口验收待完成
- **GPU 选择**：`--features gpu` 用 wgpu 枚举真实适配器，套用自动/集成/独立/指定/纯软件策略
- 引擎流水线：场景 → 布局 → 绘制 → UiBatch → 后端
- 测试与可运行示例

## v0.1 明确未实现

- 真实文字光栅化（`DrawText` 在 Headless 后端返回 `Unsupported`；需要 Parley/字体后端）
- 实际 GPU 绘制后端、系统无障碍桥、图片/SVG 解码（winit 窗口与软件呈现示例已实现）
- Jasper ABI、Jasper UI Framework、UI IR、状态/事件/动画/路由
- Web HTML/CSS/JS/WASM 输出
- 完整文本编辑与 IME 往返、过冲回弹、系统无障碍桥（文本/组合事件映射与焦点路由已有实现）
- 这些缺口在 [docs/status.md](docs/status.md) 里逐项标注，不写成“已完成”。

## 快速开始

```bash
cargo test
JASPER_UI_VM_RUN=/path/to/vm-entry.sh scripts/verify-in-vm.sh env CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=experiments/blitz-host/target JASPER_UI_TEST_FONT=/path/in/vm/font.ttc cargo +nightly run --locked --features native-blitz-cpu-pending --example hello -- /path/in/vm/font.ttc
# 字体由验证 VM 宿主注入（示例路径用占位符）；生成 hello.ppm，运行原生候选入口，整体验收仍未完成。
```

原生鼠标/视觉更新/停用/退休示例为 `--features native-blitz-pending --example pointer -- FONT`，字体同样由 VM 宿主注入。通用按钮的焦点由 Framework 显式请求；该示例不声称触摸或 OS capture。旧触摸/capture 演示仅以 `--example legacy_pointer` 保留迁移对照。

要求 Rust 1.89+（以 `Cargo.toml` 为准）。

## 架构

```text
Jasper UI Framework / UI IR          （Jasper 侧，后续）
              |
        Native ABI                    （稳定边界，后续）
              |
  jasper-ui-engine (this repo)
    scene -> layout(Taffy) -> paint -> UiBatch -> backend
              |                         |
        text measure              Headless / Platform backend
        (cosmic-text measure; raster later)            (winit software example / GPU rendering later)
```

- `scene`：当前实例树，稳定 `NodeId`；不是 Jasper 声明层。
- `layout`：把 PSS 子集翻译成 Taffy；Jasper 公共语义不直接暴露 Taffy 类型。
- `paint`：只产出显示列表，不绑定 Vello/GPU。
- `batch`：渲染批编码，供 FFI/IPC/网络复用。
- `backend`：软件/GPU/远程后端的统一入口。
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

## 许可证

Apache-2.0，见 [LICENSE](LICENSE)。依赖许可证在 [docs/status.md](docs/status.md) 记录；新增依赖先审计再锁定版本。

## 交付要求与当前核查

- 系统内置宿主与应用自带宿主是一等目标；系统桌面依赖 UI，不能把内置模式仅当成后续移植。内置路径不得强制依赖 winit、POSIX、GPU 或系统字体目录；当前系统服务适配尚未实现。
- 只确定 Jasper 的 `jasper.ui.*` 命名空间，组件子包名称未冻结。Rust 不复制另一套组件/主题语义。
- 数学形式化证明是关键路径的硬性交付门槛；现有单元测试不构成数学证明。当前证明状态、义务与信任边界见 [verification.md](docs/verification.md)。
- 现有 Rust `UiBatch` 是绘制批，不是完整 UI 语义更新协议；见 [batch-boundaries.md](docs/batch-boundaries.md)。
- [status.md](docs/status.md) 是能力核查入口；测试、模型检查、实现级证明分开记录。
