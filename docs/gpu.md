# GPU 适配器选择与切换

目标：像 Windows 的“图形设置”一样，**每个应用可以自己选 GPU**，并且能在运行时切换。

## 用户可见选项

| 选项 | 含义 |
|---|---|
| 自动 | 优先独立显卡，其次集成显卡，最后软件适配器 |
| 集成显卡 | 优先 iGPU；没有时回退到其它硬件 |
| 独立显卡 | 优先 dGPU；没有时回退到其它硬件 |
| 指定适配器 | 按名称子串匹配（不区分大小写），例如 `name:RTX 3060` |
| 纯软件 | 强制 CPU 适配器（WARP/lavapipe/llvmpipe 等） |

允许关闭软件回退：只有硬件适配器可用时才启动；否则把错误交给用户。

## 配置接口

- `JASPER_GPU=auto|integrated|discrete|software|name:<子串>`
- `JASPER_GPU_ALLOW_SOFTWARE=0|1`
- `GraphicsSettings::from_env()` 读取；`env_pairs()` 供设置界面写回。
- `select_adapter(adapters, preference, allow_software)` 是宿主中立策略，不依赖 wgpu。

## 运行时切换

1. 场景/布局/输入状态都在宿主中立层，切换时不需要重建用户界面。
2. 宿主销毁旧 surface/device，按新偏好重新枚举适配器并创建。
3. 新设备初始化失败时按回退顺序重试；报告 `SelectionFallback`。
4. device lost 也走同一条重建路径，并保留场景版本。

## 与 wgpu 的映射

- `AdapterInfo.kind` 来自 `wgpu::DeviceType`：IntegratedGpu / DiscreteGpu / Cpu / Other。
- `vendor`、`device` 原样保留，供“指定适配器”与诊断使用。
- 设置界面保存的是偏好，不保存易变的适配器索引。
