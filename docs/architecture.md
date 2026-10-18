# 架构说明

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

本仓库不实现 Jasper 组件、响应式状态、主题、动画、导航、Web 生成；这些归 Jasper。

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

- 自带宿主：winit + wgpu/Vello（后续）。
- 内置宿主：系统 UI 服务 + 通道 + 软件光栅/GPU（后续）。
- Web：不使用本引擎，由 Jasper Web Runtime 负责 DOM。

内置宿主不应被迫链接 winit/wgpu；自带宿主也不应假设系统有 UI 服务。

## 5. 触摸/触控笔

`input.rs` 只做**平台无关规范化**：

- `PointerKind`：Mouse/Touch/Pen/Unknown
- `PointerPhase`：Down/Move/Up/Cancel/Hover
- `PointerEvent`：id、位置、压力、倾斜、扭转、接触面积、主指针、笔擦
- `PointerTracker`：多指/多笔活跃状态

平台适配层负责把各平台原始事件（Windows Ink、Apple Pencil、Android MotionEvent、libinput 等）映射到该模型。手势识别、手掌拒绝、预测/合并事件、输入法组合由 Jasper UI 层后续定义。

## 6. 当前边界

- v0.1 只实现布局/绘制/批处理/软件光栅/输入模型。
- 文字：有测量 trait 和近似实现；真实 Parley 排版与字形光栅化未接。
- GPU/窗口/无障碍/图片/SVG：未接。
- ABI/UI Framework/Web：未接。

详见 [status.md](status.md)。
