# 桌面宿主

目标：用 `winit` 提供真实窗口、输入循环和 DPI，把事件规范化为引擎的 `InputEvent`，再通过软件或 GPU 后端呈现 `UiBatch`。

## 已落地（第一切片）

- `--features desktop` 引入 winit 0.30。
- `DesktopInput` 把以下事件映射为引擎事件：
  - `CursorMoved` → `PointerEvent`（无按键为 Hover，有按键为 Move）
  - `MouseInput` → Down/Up，保留 Left/Right/Middle/Back/Forward/Other
  - `MouseWheel` → `ScrollEvent`（行/像素 delta 统一为逻辑像素，符号按引擎滚动约定反转）
  - `KeyboardInput` → `KeyEvent`；无 Ctrl/Alt/Meta 的字符键额外产出 `Text`
  - `Ime::Commit` / `Preedit` → `Text` / `Composition`
  - `Focused` → `FocusEvent`
  - `ModifiersChanged` → `Modifiers`
- 纯映射层不创建窗口，因此可以在无头 CI 中测试。

## 已落地：真实窗口 + 软件呈现

- `examples/window` 用 `ApplicationHandler` 创建 winit 窗口。
- `softbuffer` 把 `HeadlessBackend` 的 RGBA 帧转换为 `0x00RRGGBB` 并呈现。
- `cargo run --features desktop --example window -- --once` 已实跑：创建窗口、渲染一帧、退出。
- 输入事件经 `DesktopInput` 规范化并打印到 stderr。

## 下一片

- DPI/缩放与窗口尺寸变化的完整处理
- 触控/笔事件（winit `Touch` / 平台扩展）
- 窗口生命周期与多窗口

## 再下一片

- `wgpu` 适配器枚举，映射到 `src/graphics.rs` 的 `GpuPreference`
- GPU 后端消费 `UiBatch`；失败/device lost 回退软件
- 设置界面写回每应用偏好，运行时重建 device/surface
