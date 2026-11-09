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
  - `Touch` → 带稳定触控身份的 Down/Move/Up/Cancel；首个触点保持 primary，直到它结束
- 纯映射层不创建窗口，因此可以在无头 CI 中测试。

`PointerMotionCoalescer` 是平台无关的单帧移动合并器：同一指针连续的 Move/Hover 只保留
最新样本，Down/Up/Cancel、滚轮、键盘、文本和焦点事件会先冲刷待处理移动，保证生命周期
事件及顺序不被丢弃。Windows、macOS 和移动端适配器可以复用同一语义。

`PointerMotionPredictor` 基于宿主时间戳给出短时线性外推，只用于显示提示；下一真实
Move/Hover 必须校正。不同指针身份或生命周期事件会清空预测历史。它是宿主无关 API，
不宣称底层平台已有事件预测/合并能力。

winit 提供校准或归一化压力时，触控映射会把压力限制在 `0..=1`；平台未提供有限压力时，
按 Down/Move=`1`、Up/Cancel=`0` 的安全默认值报告。该默认值不是压力测量。

Windows Ink、Apple Pencil、Android MotionEvent 和 Linux libinput 的专用适配器应先构造
`platform_input::RawPointerSample`，再由 `normalize_pointer` 校验坐标、压力、倾斜、扭转、
接触面积及触摸/笔约束。失败不产生部分 `PointerEvent`。当前有该边界的单元测试，但没有
声称物理触控笔或上述平台已运行验收。

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
- `--features desktop-accessibility` 在 `examples/window` 中建立 AccessKit winit 适配器；
  Linux 经 AT-SPI，Windows 由同一可选依赖选择 UIA，macOS/Android/iOS 使用各自平台实现。
  该特性不把 AT-SPI、UIA 或 winit 类型带入默认核心构建。
- `examples/multiwindow` 为两个窗口分别建立场景、`HostRenderer`、Surface 代数和软缓冲后端；
  两个 owner 不共享可变视图状态。窗口创建、焦点和呈现仍由宿主负责；
  `--once --exercise-suspend-resume` 会释放宿主窗口/Surface、暂停两个引擎视图，重建后恢复。
- 每次绘制用物理窗口尺寸与 DPI 更新原生 viewport；尺寸/缩放变化请求重绘，零尺寸跳过呈现。
- 暂停释放宿主窗口/Surface 并停用原生焦点，恢复后重建宿主资源；终止退出后退休原生场景。
- 预算/像素转换/呈现失败报告并退出，不以绘制成功冒充宿主呈现成功。
- 完整六阶段实现验收、真实窗口/DPI/暂停恢复组合验收仍另列；本地集成通过不等于系统宿主已支持。

当前示例通过 `native::host::HostRenderer` 绑定 viewport 与 Surface 代数。尺寸/DPI 变化在
宿主完成 Surface 重配后生成新代数；暂停会释放宿主资源并保留原生场景，恢复重建资源和
viewport，关闭只退休一次。GPU 示例在 `Queue::present` 后确认 `gpu_presented`，present 前
继续视为待呈现。窗口失活只清原生 hover/press/focus，不冒充 Surface 暂停。

GPU 示例的 `--exercise-suspend-resume` 走 `Engine::suspend` → host device/window 重建 →
`Engine::resume`；`--exercise-device-loss` 走 wgpu DeviceLost 事件路径。两者均在 Ubuntu
Wayland/RTX 3060 上完成两代呈现，但恢复事件本身仍由宿主控制，不能替代真实 OS 会话冻结。

## 下一片

- vblank 截止时间与真实 OS 会话冻结验收
- 触控笔压力/倾斜的 Linux 平台扩展；winit `Touch` 已接基础压力存在性与生命周期
- 多窗口暂停/恢复状态保留（当前示例在会话暂停后重建窗口）

## 再下一片

- `wgpu` 适配器枚举，映射到 `src/graphics.rs` 的 `GpuPreference`
- GPU 后端消费 `UiBatch`；失败/device lost 回退软件
- 设置界面写回每应用偏好，运行时重建 device/surface
