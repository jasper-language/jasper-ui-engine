# 宿主集成说明（v0.1 原型）

> 这是给宿主/图形界面集成的**最小说明**。API 会变；先用来跑通链路，不要当稳定 ABI。

## 1. 自带宿主（桌面/移动）

1. 用平台层（后续接 winit）实现 `PlatformHost`：窗口尺寸、DPI、重绘请求、光标。
2. 用 GPU 后端（后续接 wgpu/Vello）实现 `RenderBackend`：`resize` / `submit(&UiBatch)` / `present`。
3. 把输入规范化成 `input::InputEvent`（触摸/笔/鼠标），交给 Jasper UI 层做手势与事件分发。
4. Jasper UI 层产出 `Scene` / `ScenePatch` / `SceneUpdate`，启用原生 feature 后通过 `engine::Engine` 接入。旧 `render(Scene)` 仅由显式 `LegacyEngine` 提供兼容。
5. 输入用 `hit_test` / `PointerRouter`：命中节点 + root→target 路径 + capture；
   多指/多笔各自独立，触控笔字段不要丢。
6. 需要参考手势时，把 route 喂给 `GestureRecognizer`（tap/long-press/pan/pinch）；
   它只是宿主中立原语，Jasper 层可以替换。

## 2. 内置宿主（系统 UI 服务）

- 引擎跑在 UI 服务进程，不跑在应用进程。
- 应用侧只发送场景/补丁；UI 服务侧用 `UiBatch` 走系统通道。
- `RenderBackend` 可以先用软件光栅（本仓库的 `HeadlessBackend` 是参考实现），后续接系统合成器/GPU。
- 不需要链接 winit/wgpu；`PlatformHost`/`RenderBackend` 由系统宿主实现。
- 触摸/触控笔事件在系统入口规范化成 `PointerEvent`，不要退化成鼠标事件。

## 3. 当前可直接用的部分

- `Scene` / `Node` / `Style` / `Length` / `Edges`
- `TaffyLayoutEngine`（Flex/Grid/Block 子集）
- `paint` + `UiBatch`
- `HeadlessBackend`（软件光栅；文字光栅未实现）
- `PointerEvent` / `PointerTracker`（多点、压力、倾斜、触控笔笔擦）
- `Engine` 流水线

## 4. 当前不能承诺

- 文本真实排版/字形（Parley 未接）
- GPU/窗口/无障碍/图片/SVG
- Jasper ABI、UI Framework、Web 输出
- 触摸手势、IME、命中测试

见 [status.md](status.md)。
