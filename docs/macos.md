# macOS 宿主

macOS 参考宿主位于 [`macos/`](../macos/)；构建、命令行与限制见
[macos/README.md](../macos/README.md)。

它使用 winit 的 Cocoa 窗口、wgpu 的 Metal `CAMetalLayer` Surface 和 Jasper UI 的正式
`HostRenderer` / `GpuRenderer`。macOS 专属窗口、Surface、Retina、无障碍与打包代码不进入
默认核心构建。

## 数据路径

- 宿主创建窗口、`Instance`、Surface、`Device`/`Queue` 和 `GpuRenderer`。
- `Engine::reconfigure` 接收物理 `inner_size` 与 `scale_factor`；根场景使用逻辑尺寸。
- 指针事件在宿主边界按 `scale_factor` 换算为逻辑坐标后进入 `Engine::pointer`。
- `render_gpu_to_host_view` 把 Vello hybrid 帧画进宿主交换链纹理；`Queue::present` 成功后
  宿主调用 `gpu_presented` 确认，确认前 `needs_frame` 保持待呈现。
- Surface 为 `Bgra8UnormSrgb` 时，场景声明色（线性 RGB）在 drawable 中按其 sRGB 编码存储；
  读回证据同时给出编码后的期望值与实测计数。

## 无障碍（Cocoa）

- 可选特性 `desktop-accessibility` 让 `accesskit_winit` 选择 macOS（`accesskit_macos`）后端。
- 适配器在窗口首次可见前建立；窗口可见后由 `InitialTreeRequested` 触发首次树更新。
- **坐标单位**：AccessKit 的 bounds 是物理像素，`accesskit_macos` 会除以
  `backingScaleFactor`。场景 bounds 是逻辑坐标，因此宿主用 `Affine::scale(scale)` 转换；
  直接传逻辑 bounds 会让 AX 帧缩小一半并整体偏移。参考宿主已按此实现，真机 AX 帧与
  `node_bounds` 精确对应。
- 动作经 `Engine::accesskit_action` 回到场景；`Click` 触发的激活与指针点击走同一条
  版本化补丁路径。

## 生命周期

- `resumed` 建立窗口与 GPU 资源；`suspended` 释放宿主资源并保留场景。
- 窗口遮挡时 `get_current_texture` 返回 `Occluded`，宿主在有限次数内重试并重新请求重绘，
  不会把“取不到 drawable”当成已呈现。
- `CloseRequested` 退休原生视图；`--frames`/`--once` 模式在达到帧数后退休并退出。

## 验收

```bash
bash macos/build-app.sh
bash scripts/verify-macos-host.sh
```

真机（Apple M2，macOS 15.7.5，Retina 2x）实测：

| 项目 | 结果 |
|---|---|
| Metal 适配器 | `Apple M2` / Metal，`Bgra8UnormSrgb`，`AutoVsync` |
| Retina | 960×640 逻辑 → 1920×1280 物理，`scale=2` |
| 呈现与读回 | 首帧背景 2,140,111 / 按钮 293,924 / 文字与抗锯齿 23,565 像素；两次运行逐字节一致 |
| 无障碍树 | `AXButton` 标题 `激活 Activate`，屏幕坐标与场景 bounds 一致（421,424 / 627×120） |
| AX 激活 | `AXPress` → `ActionRequested` → `stage=activate` → 新帧 |
| 真实系统点击 | `CGEvent` 在按钮中心投放 → `stage=activate` → 新帧 |
| 中文渲染 | 宿主注入 `DejaVuSans + DroidSansFallbackFull` 有序栈后中文行与按钮中文渲染正确（墨迹 23,565）；`--cjk-tofu` 去掉回退字面后同位置是缺字框（15,349），两帧 SHA-256 不同 |

像素计数、文件哈希与完整会话日志见 `evidence/macos-reference-host/report.json`。

## 未完成

- Metal 设备丢失：引擎当前把 `gpu_device_loss_recovery` 报为 `Unsupported`。
- vblank 截止时间、合成器节奏与硬件无关的帧时间门限。
- 活系统 IME 往返、窗口重设/遮挡/多显示器自动化、公证与 App Store 分发。
- Intel（x86_64）运行验收：`--universal` 只完成合并构建。
