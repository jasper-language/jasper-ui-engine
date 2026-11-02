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

## 原生窗口调用方（2026-11-02）

- `examples/window` 用 `ApplicationHandler` 创建 winit 窗口。
- `softbuffer` 把 `HeadlessBackend` 的 RGBA 帧转换为 `0x00RRGGBB` 并呈现。
- 新命令：`cargo run --locked --features desktop,native-engine-cpu --example window -- FONT [--once|--once-after-activation]`，仅经受限 VM 编译/运行。
- 指针、滚轮、命令键、IME 预编辑/提交与焦点变化现在送入根 `Engine` 和公共 `TextDisplayView` 客户端；原始物理指针坐标在宿主边界换算为逻辑坐标。
- 点击激活产生版本化、纯视觉 `ScenePatch`。更新后调用方检查场景焦点与无障碍树中的焦点/动作仍然存在。
- 窗口失焦关闭命令键和外部文本焦点租约；重新聚焦时恢复两者。暂停释放 Surface 但保留场景及外部快照，关闭请求让两端退休。
- 外部文本内容只接受宿主注入的服务快照；同步预编辑窗口只显示 transient preedit，不是编辑器或 IME 引擎。
- Xvfb 提供实际 X11 protocol Surface，已覆盖创建、呈现、重设尺寸和单帧退出；合成器、vsync、真实 session suspend/resume 与系统无障碍桥仍不属于本示例验收。
- `scripts/interact-window-xvfb.sh FONT OUT_DIR` 通过 xdotool 点击实际 X11 窗口，调用方仅在收到激活并完成视觉更新后退出的诊断模式下必须报告 `activated=1`。
- 每次绘制用物理窗口尺寸与 DPI 更新原生 viewport；尺寸/缩放变化请求重绘，零尺寸跳过呈现。
- 暂停释放宿主窗口/Surface 并停用原生焦点，恢复后重建宿主资源；终止退出后退休原生场景。
- 预算/像素转换/呈现失败报告并退出，不以绘制成功冒充宿主呈现成功。
- 完整六阶段实现验收、真实窗口/DPI/暂停恢复组合验收仍另列；本地集成通过不等于系统宿主已支持。

当前示例通过 `native::host::HostRenderer` 绑定 viewport 与 Surface 代数。尺寸/DPI 变化在
宿主完成 Surface 重配后生成新代数；暂停会释放宿主资源并保留原生场景，恢复重建资源和
viewport，关闭只退休一次。GPU 示例在 `Queue::present` 后确认 `gpu_presented`，present 前
继续视为待呈现。窗口失活只清原生 hover/press/focus，不冒充 Surface 暂停。

## 下一片

- 真实合成器/vsync、硬件 GPU/device-loss 与平台休眠恢复验收
- 触控/笔事件（winit `Touch` / 平台扩展）
- 窗口生命周期与多窗口

## 再下一片

- `wgpu` 适配器枚举，映射到 `src/graphics.rs` 的 `GpuPreference`
- GPU 后端消费 `UiBatch`；失败/device lost 回退软件
- 设置界面写回每应用偏好，运行时重建 device/surface
