# jasper-ui-engine

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

- 几何、颜色、PSS 子集（Flex/Block/Grid 语义、尺寸、间距、对齐、边框）
- 场景树（稳定 `NodeId`）与增量 `ScenePatch`
- **Taffy 布局适配**（Flexbox / Grid / Block；百分比、min/max、padding/margin/gap）
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
- **桌面窗口**：`cargo run --features desktop --example window` 打开真实 winit 窗口并用 softbuffer 呈现引擎帧
- **GPU 选择**：`--features gpu` 用 wgpu 枚举真实适配器，套用自动/集成/独立/指定/纯软件策略
- 引擎流水线：场景 → 布局 → 绘制 → UiBatch → 后端
- 测试与可运行示例

## v0.1 明确未实现

- 真实文字光栅化（`DrawText` 在 Headless 后端返回 `Unsupported`；需要 Parley/字体后端）
- winit 窗口、wgpu/Vello GPU、AccessKit、图片/SVG 解码
- Jasper ABI、Jasper UI Framework、UI IR、状态/事件/动画/路由
- Web HTML/CSS/JS/WASM 输出
- 输入法组合、过冲回弹、AccessKit 桥（v0.2 接；命中/捕获/语义/焦点/手势/裁剪/滚动/滚动条已落地）
- 这些缺口在 [docs/status.md](docs/status.md) 里逐项标注，不写成“已完成”。

## 快速开始

```bash
cargo test
cargo run --example hello
# 生成 hello.ppm；可用 `convert hello.ppm hello.png` 查看
```

要求 Rust 1.71+。

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
        (Parley later)            (winit/wgpu/Vello later)
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

触控笔与触摸不是“鼠标模拟”：平台适配层负责把 Windows Ink / Apple Pencil / Android stylus / libinput 的原始数据规范化；Jasper 层负责手势语义。手势识别、手掌拒绝、预测/合并事件、输入法组合属于后续阶段，当前只提供规范化事件模型和活跃指针跟踪。

## 宿主矩阵

| 宿主 | 引擎来源 | 平台层 | 说明 |
|---|---|---|---|
| 桌面/移动（自带） | 应用携带 | winit/平台适配（后续） | 类似 Flutter |
| 内置系统宿主 | 系统 UI 服务提供 | 系统通道/合成器（后续） | 应用不携带引擎 |
| Web | 不使用本引擎 | 浏览器 DOM/JS/WASM | 由 Jasper Web 后端负责 |

本仓库的 trait 设计保证：**内置宿主与自带宿主共用同一套引擎核心，但平台/窗口/渲染实现可替换**。

## 许可证

Apache-2.0，见 [LICENSE](LICENSE)。依赖许可证在 [docs/status.md](docs/status.md) 记录；新增依赖先审计再锁定版本。
