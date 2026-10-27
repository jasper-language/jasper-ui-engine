# 桌面宿主

当前窗口候选使用根 `Engine` 与原生 CPU 帧，不生成旧 `UiBatch`。winit/softbuffer 由示例宿主持有，字体必须显式传入；不代表系统内置宿主或 GPU 呈现已接通。

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

## 原生窗口迁移候选（2026-10-26）

- `examples/window` 用 `ApplicationHandler` 创建 winit 窗口。
- `softbuffer` 把 `HeadlessBackend` 的 RGBA 帧转换为 `0x00RRGGBB` 并呈现。
- 新命令：`cargo run --features desktop,native-blitz-cpu-pending --example window -- FONT [--once]`，仅经受限 VM 编译/运行。
- 原有窗口实跑证据属于旧执行链，不能作为新路径的实际呈现验收。当前 VM 无可用显示服务；不转宿主运行，也不创建或重启共享显示服务。
- 每次绘制用物理窗口尺寸与 DPI 更新原生 viewport；尺寸/缩放变化请求重绘，零尺寸跳过呈现。
- 暂停释放宿主窗口/Surface 并停用原生焦点，恢复后重建宿主资源；终止退出后退休原生场景。
- 预算/像素转换/呈现失败报告并退出，不以绘制成功冒充宿主呈现成功。
- 完整六阶段实现验收、真实窗口/DPI/暂停恢复组合验收仍未完成。
- 输入事件经 `DesktopInput` 规范化并打印到 stderr。

## 下一片

- DPI/缩放与窗口尺寸变化的真实宿主组合验收
- 触控/笔事件（winit `Touch` / 平台扩展）
- 窗口生命周期与多窗口

## 再下一片

- `wgpu` 适配器枚举，映射到 `src/graphics.rs` 的 `GpuPreference`
- GPU 后端消费 `UiBatch`；失败/device lost 回退软件
- 设置界面写回每应用偏好，运行时重建 device/surface
