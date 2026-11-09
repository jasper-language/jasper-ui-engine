# Android 宿主

Android 参考宿主位于 [`android/`](../android/)；详细构建与设备验收步骤见
[android/README.md](../android/README.md)。

它使用 NativeActivity、winit Android 后端、wgpu GLES Surface 和 Jasper UI 的正式
`HostRenderer`。Android 专属窗口、Surface、生命周期和输入代码不进入默认核心构建。
APK 同时嵌入 DejaVu Sans 与 Droid Sans Fallback；按钮中的 UTF-8 中文
`中文 激活` 经有序字体栈回退后实际光栅化，而不是把缺失字形显示为方框。

## 生命周期

- `android_main` 建立 winit event loop。
- `resumed` 创建宿主持有的 Window 和 wgpu Surface/device，并调用 `Engine::reconfigure`
  或 `Engine::resume`。
- `suspended` 调用 `Engine::suspend`，保存返回的后继 Surface 代数，然后释放宿主 GPU 资源。
- 前台恢复使用所保存的代际，避免旧代数被错误拒绝。
- 旋转或内容尺寸变化重新配置宿主 Surface，并同步根场景的逻辑尺寸。

## 输入

winit `Touch` 经 `DesktopInput` 转换为 `PointerEvent`，再由正式 `Engine::pointer` 路由。
Android MotionEvent 的原文压力、倾斜、工具类型和历史点适配仍应由平台专用层送入
`platform_input::RawPointerSample`，再复用同一规范化与手势路径。

## 已知边界

- 当前只验收 arm64-v8a。
- 当前 APK 用 NativeActivity，不含 Java/Kotlin UI 层。
- 当前中文验收覆盖参考按钮的 CJK 字体回退和截图，不宣称完整中文语言包、地区格式或系统
  字体发现。
- AccessKit/Android 系统无障碍服务、活 IME、应用进程隔离和跨进程 UI 服务尚未接入。
- 设备旋转与前后台切换已实测；低内存杀进程恢复、分屏和多窗口仍需设备矩阵。
