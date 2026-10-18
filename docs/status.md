# 能力状态（v0.1）

> 状态定义：**已验证** = 有真实实现 + 测试；**实现未充分验证** = 有实现但缺平台/一致性测试；**未实现** = 无实现或明确返回 Unsupported；**外部依赖** = 等 Jasper 编译器/宿主能力。

## 已实现且已验证

| 能力 | 说明 |
|---|---|
| 几何/颜色/边距 | `geometry.rs`、`style.rs` |
| PSS 子集 | 尺寸/百分比/min-max/内外边距/间距/Flex 方向/对齐/边框；不含阴影/滤镜/变换 |
| 场景树 + 增量补丁 | `scene.rs`；稳定 NodeId；插入/更新/删除；版本/原子批 |
| Taffy 布局 | Flex/Grid/Block；文本测量接入 measure 回调 |
| 文字测量 | `TextEngine` trait + 近似参考实现（非 Parley） |
| 显示列表 | Fill/Stroke/Text/Clip |
| UiBatch | 版本化编码/解码；边界检查；往返测试 |
| 软件光栅 | RGBA、填充、描边、裁剪、alpha 混合、PPM |
| Headless 后端 | 真实帧缓冲；文字绘制明确返回 Unsupported |
| 指针/多点/触控笔模型 | `PointerEvent` + `PointerTracker` |
| 命中测试 | 布局树最上层命中 + root→target 路径 |
| 指针路由 | 按 pointer capture；多指/多笔独立 |
| 语义树 | `SemanticsTree::from_scene`，角色/标签/值/隐藏/可聚焦 |
| 焦点管理 | pre-order `FocusManager`，Tab/Shift+Tab |
| 参考手势 | tap/long-press/pan/pinch/cancel；阈值可配 |
| 引擎流水线 | Scene → Layout → Paint → Batch → Backend |

## 已实现但尚未充分验证

| 能力 | 缺口 |
|---|---|
| Taffy 百分比/min-max | 只有单元测试，缺跨平台/大场景一致性测试 |
| 文字近似测量 | 不是真实字形度量；CJK/emoji/RTL 未验证 |
| 软件光栅 | 未做模糊/渐变/圆角/抗锯齿；仅轴对齐矩形 |
| 场景补丁 | 已做原子批量 + 版本/stale；未做并发多写/合并策略 |
| 命中测试语义 | 只看绘制内容，未接组件可点击/可聚焦语义 |

## 未实现（明确返回错误或未接入）

| 能力 | 当前行为 |
|---|---|
| 文字光栅化 | `HeadlessBackend` 对 `DrawText` 返回 `BackendError::Unsupported` |
| 真实字体排版（Parley/字体回退/CJK/emoji/RTL） | 未接 |
| winit 窗口/输入循环 | 未接；只有 `PlatformHost` trait |
| wgpu/Vello GPU | 未接；只有 `RenderBackend` trait |
| AccessKit | 未接（已有宿主中立语义树 `SemanticsTree`） |
| 图片/SVG | 未接 |
| 输入法组合 | 未接（手势参考层已落地） |
| Jasper Native ABI | 未接；后续单独规范 |
| Jasper UI Framework/UI IR | Jasper 侧，未开始 |
| Web HTML/CSS/JS/WASM | Jasper 侧，未开始 |

## 依赖与许可证

| 依赖 | 版本 | 许可证 | 状态 |
|---|---|---|---|
| `taffy` | 0.15.0 | MIT | 已锁定；布局执行器 |

- 其他重型依赖（winit/wgpu/vello/parley/accesskit/image/resvg）**暂不引入**，等平台/许可证/API 验证后再逐项锁定。
- `resvg/usvg` 为 MPL-2.0，引入前需做 Apache-2.0 兼容与 NOTICE 评审。

## 下一步（按依赖顺序）

1. 布局一致性测试 + 文字 measure 的 CJK/换行基线。
2. 命中测试/指针路由已完成 v0.1 原型；下一步手势层与 Jasper 事件语义。
3. 手势层与 Jasper 事件语义的边界规范。
4. Parley 文本适配 + 字体回退（替换近似测量）。
5. winit 平台适配（桌面自带宿主）。
6. wgpu/Vello 渲染后端；软件后端保留为降级/测试。
7. Jasper Native ABI v1 + 句柄表。
8. AccessKit / 图片 / SVG。
9. 内置宿主适配（系统 UI 服务/通道）。
10. Web 后端（Jasper 侧）。
