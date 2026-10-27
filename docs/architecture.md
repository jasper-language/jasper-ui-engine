# 架构说明

## Current ownership: Blitz-based UI engine

Framework owns components, Signal/state/theme, layout declarations, interaction
and accessibility semantics. This Rust repository consumes that contract; it
does not implement the Jasper-language Framework or Signal runtime.

The Rust Engine wraps Blitz and adapts scene updates, render batches and events.
Blitz/Vello/Parley execute layout, drawing and text shaping. Platform hosts own
window surfaces, DPI, GPU device access/presentation, native input and native
accessibility bridges. External text/input/syntax services own editing algorithms,
authoritative documents, language parsing, IME engines, dictionaries and learning.
The Engine implements only their UI-side client adapters.

Candidate windows belong to the host input service or its designated UI host;
the UI framework provides a presentation view only when explicitly delegated.
UI accessibility semantics stay public; OS accessibility translation stays in
platform adapters. An independent Web backend consumes the same public semantics
and is not coupled to the native Blitz host. Desktop applications do not serialize
HTML: internal tree/style details are encapsulated behind native facade APIs.

Acceptance workstreams: (1) Rust-facing facade contracts; (2) Engine/Blitz and
host-interface integration; (3) interaction/lifecycle and external-service clients;
(4) migration/verification. The fourth is engineering work, not a UI feature.
The following original implementation description is historical prototype context;
it is not authorization to rebuild capabilities already provided by Blitz.

## 1. 分层

```text
Jasper Application
  Jasper UI Framework / UI IR / Compiler        （Jasper 侧）
        |
  Native UI ABI（稳定句柄、批量操作、错误码）   （后续）
        |
  jasper-ui-engine                              （本仓库，Rust）
    scene / style / layout / text / paint / batch / backend / platform / input
        |
  Platform + GPU + Font + Accessibility         （后续适配）
```

本仓库不实现 Jasper 组件、响应式状态、主题规则、动画语义、导航、Web 生成；这些归 Jasper。

## 2. 数据流

1. 宿主把当前场景（`Scene`）或增量（`ScenePatch`）交给引擎。
2. `TaffyLayoutEngine` 把 PSS 子集翻译为 Taffy 样式，计算布局。
3. `paint` 遍历布局树，产出 `DisplayList`。
4. `UiBatch` 把显示列表编码为版本化字节流。
5. `RenderBackend` 解码并执行；`HeadlessBackend` 用软件光栅落地。
6. 未来：同一 `UiBatch` 走 FFI / 系统 IPC / 远程通道。

## 3. 关键不变量

- `NodeId` 在场景生命周期内稳定；句柄表使用代际信息（后续 ABI）。
- 布局只依赖 PSS 子集 + 文本测量；不直接暴露 Taffy 类型。
- 绘制只产出显示列表；不直接依赖 GPU/Vello。
- `UiBatch` 带 magic/version/op count；解码必须做边界检查。
- 后端错误必须可识别；不支持的能力返回 `BackendError::Unsupported`，不得静默成功。
- Rust panic 不穿越 ABI（ABI 层后续统一 catch）。

## 4. 宿主无关

`PlatformHost` 只描述最小窗口/DPI/重绘能力；`RenderBackend` 只描述提交/呈现。具体实现：

- 自带宿主：winit + softbuffer 示例已实现；GPU 渲染待接。
- 内置宿主：系统 UI 服务 + 通道 + 软件光栅/GPU（后续）。
- Web：不使用本引擎，由 Jasper Web Runtime 负责 DOM。

内置宿主不应被迫链接 winit/wgpu；自带宿主也不应假设系统有 UI 服务。

## 5. 触摸/触控笔

`input.rs` 只做**平台无关规范化**：

- `PointerKind`：Mouse/Touch/Pen/Unknown
- `PointerPhase`：Down/Move/Up/Cancel/Hover
- `PointerEvent`：id、位置、压力、倾斜、扭转、接触面积、主指针、笔擦
- `PointerTracker`：多指/多笔活跃状态

平台适配层负责把各平台原始事件（Windows Ink、Apple Pencil、Android MotionEvent、libinput 等）映射到该模型。参考手势识别与手掌拒绝已有原型；与 Jasper 组件语义的唯一权威边界仍需定义。平台事件规范化/组合事件路由不等于完整 IME 编辑。

## 6. 当前边界

- v0.1 已实现布局/绘制/批处理/软件光栅/输入、交互与语义原型，具体范围以 status.md 为准。
- 文字：有测量 trait、近似实现及可选 cosmic-text 真实测量；字形光栅化与完整文本编辑未接。
- 窗口：winit + softbuffer 示例已接；GPU 仅枚举与选择，无障碍只有中立语义树，图片/SVG 未接。
- ABI/UI Framework/Web：未接。

详见 [status.md](status.md)。

## 7. 协议与证明边界

现有 `UiBatch` 是布局后的绘制批；场景更新与事件身份不能通过它自动恢复。语义更新、绘制和传输的职责见 [batch-boundaries.md](batch-boundaries.md)。系统内置服务与应用自带宿主共用执行核心；不在引擎另建系统合成器。

数学形式化证明为关键性质的交付门槛，见 [verification.md](verification.md)。版本计算函数已建立首项实现级证明切片；整个场景事务及其余核心性质仍待证明，不得把单项结论或文字声明扩大为整个引擎已证明。

## System-built-in host boundary (user-provided namespace plan)

- `system.window.*`: application windows, size/scale/state, focus requests and
  presentation of window content. A request does not establish focus ownership.
- `system.display.*`: monitor enumeration, resolution/refresh/layout/color
  capabilities; mutations require system permission, outside UI authority.
- `system.input.*`: authorized-window keyboard/pointer/touch/text input, routing
  and capture. In-window UI routing never grants access to another window.
- `system.event.*`: shared subscriptions, sequence numbers, overflow resync and
  batched wait. Reuse this mechanism across domains rather than implementing a
  parallel system event mechanism in the UI framework.

These are responsibility constraints, not frozen IPC/function/payload definitions.
The UI host adapter maps authorization and window lifetime into UI identity and
cancellation contracts; Blitz stays behind that adapter. Presentation does not
transfer compositor ownership. Overflow/sequence-gap recovery needs a coherent
system snapshot/subscription protocol before stale-state-dependent input resumes.
Sequence scope, cross-domain ordering and snapshot handoff are unresolved contracts.
Actual jasper-os integration remains deferred until after Canvas; no OS changes.
