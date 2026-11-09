# 跨平台宿主边界

Jasper UI 是跨平台框架，但跨平台不等于在每个平台重复一份引擎。默认
`jasper-ui-engine` 构建只包含宿主无关场景、布局、绘制、输入与生命周期契约；
winit、wgpu、AT-SPI、UIA、Cocoa、Android 和 iOS 能力都放在显式门控的宿主适配层。

## 所有权

| 责任 | 引擎核心 | 平台适配器 | 系统内置宿主 |
|---|---|---|---|
| 场景、版本、补丁、语义树 | 拥有 | 消费 | 消费 |
| 指针/触摸规范化后的生命周期 | 拥有 | 把原始事件送入 | 把 `system.input.*` 事件送入 |
| 窗口、焦点、DPI、Surface | 只维护代际/拒绝语义 | 创建并持有 | 由系统 UI 服务创建并持有 |
| GPU 设备、交换链、present、vsync | 提供 TextureView 与 device-loss 契约 | 持有 wgpu 设备与 Surface | 系统合成器持有 |
| AccessKit 树转换与动作准入 | 拥有 | 选择 AT-SPI/UIA/Cocoa 等平台实现 | 接系统无障碍服务 |
| 编辑器、IME、词库 | 只消费外部权威快照 | 连接外部服务 | 只提供系统输入服务 |

## 当前实现

| 目标 | 窗口/输入 | 无障碍 | 验收状态 |
|---|---|---|---|
| Ubuntu/Wayland | winit + softbuffer；winit Touch | `accesskit_winit` → AT-SPI | CPU 双窗口、硬件 RTX 3060 Vulkan、AT-SPI `click` 已实测 |
| Windows | 同一 winit 边界 | `accesskit_winit` → UIA | 仅纳入跨目标核心编译；尚无 Windows 运行验收 |
| macOS | 同一 winit 边界 | `accesskit_winit` → Cocoa | Linux 交叉编译 x86_64/AArch64；尚无运行验收 |
| Android | NativeActivity + winit + wgpu + Touch | Android 平台后端待接 | C66/Android 11/arm64 已实测 GPU、触摸、旋转与前后台恢复 |
| iOS | 平台生命周期适配器 | `accesskit_winit` 对应平台后端 | 尚未实现或运行验收 |
| jasper-os 内置宿主 | 系统窗口/输入服务 | 系统无障碍服务 | 不属于 Ubuntu 参考宿主；尚未接入 |

`desktop-accessibility` 特性只是在参考宿主中启用平台适配器，不把平台 API
编译进默认核心。宿主必须在窗口首次可见前创建 AccessKit 适配器，并在 Surface
代际变化后继续通过当前 `HostRenderer` 版本生成树更新。

平台原始触控/笔输入统一经过 `platform_input::RawPointerSample` →
`normalize_pointer`。该边界校验压力、倾斜、扭转、接触面积、eraser 和触摸按钮约束；
具体平台库仍只存在于各自适配器，默认核心不链接 Windows Ink、Cocoa、Android 或 libinput。

## 后续平台工作

- Windows：UIA 事件循环、DPI、WM_POINTER/Windows Ink、硬件 Surface/device-loss。
- macOS：Cocoa 无障碍、Retina、NSView Surface、Apple Pencil 宿主。
- Android：AccessKit/系统无障碍服务、活 IME、分屏/多窗口与更多 ABI 设备矩阵。
- iOS：生命周期、焦点、软键盘/IME、触摸与 Pencil。
- jasper-os：系统窗口/显示/输入/无障碍服务及其 IPC。

任何平台完成运行验收前，只能声明“通过目标编译检查”，不能把 Ubuntu 结果外推为
该平台已经可用。

当前 `desktop-accessibility` + `examples/window` 已通过 Windows x86_64 GNU、
macOS x86_64 和 macOS AArch64 的 `cargo check --locked`；Windows UIA 与 macOS
Cocoa 无障碍服务仍需在对应真机完成运行验收。宿主无关核心另外通过 Android AArch64
和 iOS AArch64 目标编译；移动窗口、输入和生命周期运行适配仍未实现验收。
