# 渲染路径第三方信任边界（TCB）登记

证明止步于本仓库的源码。下表列出原生渲染路径（`gpu-pending` 及其依赖的 feature）依赖的第三方代码：这些代码被信任，不是被验证。列在这里不代表它正确，只说明它在证明之外。

`python3 scripts/check-render-tcb.py` 用 `Cargo.toml` 和 `Cargo.lock` 核对本表：渲染 feature 的每个直接依赖和下表的几个必列项都必须按锁定版本列出，所列 crate 必须确实在闭包内，闭包变化会使下面的数量和摘要失配。`scripts/test-render-tcb.py` 检查这些漂移都会被报告。

闭包数量：338

闭包摘要：`c7101c226f002ae54295174ca3ee8a295df0e0cffb19c18f7fad1720ff4515c7`

闭包按 `Cargo.lock` 计算，包含所有平台和可选路径，比任何一次实际构建都宽。摘要覆盖名称、版本和来源。

## 登记表

| crate | 锁定版本 | 信任什么 | 现有证据（测试，不是证明） |
|---|---|---|---|
| `wgpu` | `30.0.1` | 设备、队列、纹理、命令提交与错误作用域的语义 | `tests/native_gpu_render.rs`、`tests/native_gpu_host_view.rs`、`tests/native_gpu_frame_damage.rs`，只在 llvmpipe/Vulkan 上运行 |
| `wgpu-core` | `30.0.1` | 资源跟踪与校验 | 同上，间接 |
| `wgpu-hal` | `30.0.1` | 到各图形 API 的 unsafe 映射 | 同上，只走过 Vulkan 后端 |
| `wgpu-types` | `30.0.1` | 格式、用法与限制的定义 | 同上，间接 |
| `naga` | `30.0.1` | 着色器翻译 | 同上，间接 |
| `wgpu_context` | `0.10.0` | `GpuRenderer::new_headless` 的适配器与设备获取 | `tests/native_gpu_render.rs` |
| `pollster` | `0.4.0` | 在同步调用中等待设备获取与错误作用域 | 同上 |
| `vello_gpu` | `0.3.0` | Vello hybrid 的 GPU 渲染 | 上述 GPU 测试与帧对照 |
| `vello_gpu_shaders` | `0.3.0` | Vello hybrid 的着色器 | 同上，间接 |
| `vello_common` | `0.3.0` | 场景与纹理标识的公共类型 | 同上 |
| `vello_cpu` | `0.3.0` | CPU 光栅 | `tests/native_cpu_acceptance.rs`、`tests/native_frame_differential.rs` |
| `anyrender` | `0.14.0` | 绘制抽象 | CPU 与 GPU 路径的集成测试 |
| `anyrender_vello_cpu` | `0.18.0` | anyrender 到 Vello CPU 的转换 | 同上 |
| `anyrender_vello_hybrid` | `0.11.0` | anyrender 到 Vello hybrid 的转换 | 上述 GPU 测试 |
| `peniko` | `0.6.1` | 颜色、画笔与图像类型 | 集成测试，间接 |
| `kurbo` | `0.13.1` | 几何运算 | 集成测试，间接 |
| `fearless_simd` | `0.7.0` | CPU 路径的 SIMD 派发 | 集成测试，间接 |
| `glifo` | `0.4.0` | 字形缓存与绘制 | 文字渲染集成测试 |
| `parley` | `0.12.0` | 文本布局 | `tests/native_text_layout.rs` |
| `fontique` | `0.12.0` | 字体枚举与回退 | `tests/native_font_family.rs`、`tests/native_font_weight_style.rs` |
| `harfrust` | `0.12.0` | 文字整形 | 文字渲染集成测试，间接 |
| `skrifa` | `0.44.0` | 字体读取与字形轮廓 | 同上，间接 |
| `blitz-dom` | `0.3.0-beta.4` | 文档、样式与布局编排，固定修订见 `Cargo.toml` | 原生引擎集成测试 |
| `blitz-paint` | `0.3.0-beta.4` | 从文档到绘制调用 | CPU 与 GPU 帧测试 |
| `blitz-traits` | `0.3.0-beta.4` | Blitz 公共类型 | 集成测试，间接 |
| `stylo` | `0.22.0` | CSS 样式计算 | 集成测试，间接 |
| `taffy` | `0.15.0` | 布局算法 | 集成测试，间接 |
| `keyboard-types` | `0.8.3` | 键盘事件类型 | 输入集成测试 |
| `rustc-hash` | `2.1.3` | `GpuRenderer` 内部的哈希表 | GPU 测试，间接 |

表外还信任闭包内其余 crate、Rust 编译器与标准库，以及任何真实运行时的图形驱动、窗口系统和合成器。后三者没有版本钉扎，也不在闭包内。

## 证明与 TCB 的分界

- 本仓库被证明的是准入与编排：Surface 生命周期、宿主协调、渲染结果（`verification.md` 的 UI-SURFACE-LIFECYCLE-001、UI-HOST-SURFACE-COORDINATION-001、UI-HOST-RENDER-OUTCOME-001），以及读回布局 UI-GPU-READBACK-001。
- 引擎闭包内调用上表代码之后发生的事不在任何证明内。像素是否正确只有帧测试。
- `GpuRenderer` 只对校验错误设置错误作用域。内存不足与内部错误不经这条路径报告。

## 设备丢失

引擎提供 `GpuRenderer::set_device_lost_callback`：宿主持有的 wgpu device 报告
`DeviceLostReason` 后，宿主必须暂停引擎视图、释放旧 Surface/device/renderer，重建后
`resume` 并重绘。`gpu-pending` 构建把这项能力声明为原生契约，其他构建为 feature-gated。
VM 通过真实 `Device::destroy()` 验证 callback；Ubuntu Wayland/RTX 3060 通过同一宿主
恢复生命周期演练并完成第二代帧。驱动诱发的硬件丢失、不同 GPU 驱动行为与其他平台仍
需要真机验收；wgpu/Vello 继续是第三方 TCB。

## 能力矩阵的绑定

`tests/host_capability_claims.rs` 把矩阵绑定到实际构建，需要在以下每组 feature 下运行：不开 feature、`native-blitz-pending`、`native-blitz-cpu-pending`、`gpu-pending`。声明原生契约的项，由该文件点名承载它的公开入口，入口被删除或改换门控时编译失败。
