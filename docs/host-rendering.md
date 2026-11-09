# 宿主渲染与 Surface 生命周期

状态：工程闭环已接通；硬件 GPU、真实 device-loss/vsync 和 jasper-os 集成仍未验收。

`native::host::HostRenderer` 协调正式 `Engine` 与宿主拥有的窗口、软件缓冲、wgpu
Surface/TextureView/device/queue/present。它不是第二套渲染器，也不接管窗口系统。

## 契约

- 构造时绑定已认证 `owner`、初始物理尺寸和宿主显式注入的字体栈；不扫描系统字体。
- 每次尺寸或 DPI 变化成功重配后产生新的 Surface 代数。旧代数的迟到渲染、暂停、恢复和
  退休调用会被拒绝。
- `suspend` 清除原生 hover/press/focus 并释放逻辑 Surface 状态，但保留场景、版本和语义；
  `resume` 使用宿主已经重建的尺寸/DPI 生成后继代数；`retire` 只成功一次并永久关闭视图。
- `render_gpu_to_host_view` 只向宿主持有的 `wgpu::TextureView` 提交渲染。宿主调用
  `Queue::present` 成功后必须调用 `gpu_presented`；确认前 `needs_frame` 保持待呈现。
  引擎不取得 Surface、不配置交换链，也不调用 present。
- CPU 后端提交或 present 返回错误时，宿主协调器把视图重新标脏，允许重试，不把失败当作
  已呈现。

## 编译期能力矩阵

`host_capabilities::HostCapabilityMatrix::compiled()` 报告本次构建的声明，不等于设备枚举或
平台验收。`native()` 返回相同结果。判定表的证明见 `verification.md` 的
UI-HOST-CAPABILITY-MATRIX-001，第三方边界见 `render-tcb.md`。

| 能力 | 归属 |
|---|---|
| CPU RGBA8 注入 Surface | `cpu-pending` 下原生契约（`native-blitz-cpu-pending`、`gpu-pending` 都包含它） |
| DPI/尺寸重配 | `native-blitz-pending` 下原生契约 |
| 显式字体与资源代数 | `native-blitz-pending` 下原生契约 |
| 暂停/恢复/退休 | `native-blitz-pending` 下原生契约 |
| GPU TextureView 绘制 | `gpu-pending` 下原生契约 |
| GPU Surface 获取/配置 | 宿主 |
| GPU present | 宿主 |
| 硬件 GPU 呈现 | Ubuntu RTX 3060 Vulkan 已实测；其他平台未验收 |
| GPU device-loss 恢复 | 引擎报告 callback，宿主重建 device/Surface/renderer；硬件演练已通过，驱动诱发丢失未实测 |
| vsync | 宿主 |
| jasper-os 宿主接入 | 未提供 |

## 已观察环境

- Xvfb X11 + softbuffer：CPU 窗口一帧，初始 800×600 与重配后 640×360 截图像素已核对。
- Xvfb X11 + llvmpipe Vulkan：宿主创建 Surface/device/queue，Engine 绘制宿主持有的交换纹理，
  宿主 present 后正常退出。
- Ubuntu Wayland + NVIDIA RTX 3060 Vulkan：`Bgra8UnormSrgb`/`Fifo` 完成硬件呈现，
  `--exercise-device-loss` 经恢复生命周期创建第二代 device/Surface/renderer 并再次呈现。
- 二者都不是硬件 GPU、系统合成器、真实休眠恢复、vsync 或 jasper-os 支持证据。

`JASPER_UI_PRESENT_MODE` 可选择 `fifo`（默认）、`mailbox`、`immediate`、
`auto-vsync` 或 `auto-no-vsync`；平台不支持时回退 `Fifo`。present mode 选择不等于
测得真实 vblank 截止时间。

证据入口：`evidence/native-host-render-lifecycle/report.json`、
`evidence/window-xvfb-presentation/report.json`。
