# 技术设计

## 1. 定位

`jasper-ui-engine` 是 Jasper UI 的 **宿主无关 Native 执行引擎**（Rust）。它把 Jasper UI 层产生的场景/样式/文本请求，变成布局、绘制指令、渲染批和最终帧。

它不是：

- WebView / 浏览器 / Blink 封装
- Flutter/Qt/React Native 的包装
- Jasper UI 组件库、状态系统、主题、路由
- Web HTML/CSS/JS 生成器
- Jasper 编译器

## 2. 目标与非目标

### 目标

- 同一套引擎核心服务 **内置宿主**（系统 UI 服务）与 **自带宿主**（应用打包引擎）。
- 布局/绘制/输入/批处理与平台解耦；平台、GPU、字体、无障碍可替换。
- 触摸、多点触控、触控笔从第一天进入输入模型。
- 绘制批可走进程内、IPC 或网络；后端可替换。
- 未实现能力明确失败，不静默降级。

### 非目标（当前）

- 完整 CSS/HTML 语义、DOM、JS/WASM。
- 真实文本排版（Parley 未接）。
- GPU/窗口/无障碍/图片/SVG。
- Jasper ABI、UI Framework、UI IR。
- 稳定 API；v0.1 原型随时改。

## 3. 分层

```text
Jasper UI Framework / UI IR / Compiler        （Jasper 侧，后续）
              |
        Jasper Native ABI                     （草案见 abi.md）
              |
  jasper-ui-engine (this repo)
    scene -> layout(Taffy) -> paint -> UiBatch -> backend
              |                         |
        text measure              Headless / platform backend
        (Parley later)            (winit/wgpu/Vello later)
```

- **Jasper 侧**：声明层组件、状态、事件语义、主题、动画、导航、UI IR、Web 生成。
- **本仓库**：实例场景、布局、文字测量接口、显示列表、UiBatch、后端/平台 trait、软件光栅。
- **平台适配**：窗口/输入/DPI；不属于 Jasper 公共 API。

## 4. 数据流

1. 宿主把 `Scene` 或 `ScenePatch` 交给引擎。
2. `TaffyLayoutEngine` 把 PSS 子集翻译为 Taffy 样式；文本叶子通过 `TextEngine` 测量。
3. `paint` 遍历布局，产出 `DisplayList`（Fill/Stroke/Text/Clip）。
4. `UiBatch` 编码为版本化字节流（magic `JUEB`、version 1）。
5. `RenderBackend::submit` 解码执行；`HeadlessBackend` 用软件光栅落地。
6. 后续：同一批字节走 FFI/IPC/网络；GPU 后端消费同一批。

## 5. 模块职责

| 模块 | 职责 | 当前状态 |
|---|---|---|
| `geometry` | 点/尺寸/矩形/边距 | 已验证 |
| `style` | PSS 子集、颜色、Length | 已验证 |
| `scene` | 实例树、稳定 NodeId、增量补丁 | 已验证（补丁非事务） |
| `layout` | Taffy 适配、文本测量回调 | 已验证 |
| `text` | 测量 trait + 近似参考实现 | 近似，非 Parley |
| `paint` | 显示列表 | 已验证 |
| `batch` | UiBatch 编解码/边界检查 | 已验证 |
| `raster` | 软件光栅 | 基础图元 |
| `backend` | RenderBackend + Headless | 已验证 |
| `platform` | PlatformHost + HeadlessHost | 只有 trait |
| `input` | 指针/触摸/笔模型、活跃指针 | 已验证（无手势） |
| `hit_test` | 布局树命中、root→target 路径 | 已验证 |
| `pointer` | 指针路由、按 pointer capture、多指独立 | 已验证 |
| `semantics` | 语义树快照、焦点顺序、Tab/Shift+Tab | 已验证（AccessKit 未接） |
| `engine` | 场景→帧流水线 | 已验证 |

## 6. 宿主模式

| 宿主 | 引擎来源 | 平台层 | 说明 |
|---|---|---|---|
| 内置系统宿主 | 系统 UI 服务提供 | 系统通道/合成器（后续） | 应用不携带引擎；`UiBatch` 走系统 IPC/ring |
| 自带宿主 | 应用携带 | winit/平台适配（后续） | 类似 Flutter；应用带引擎/框架 |
| Web | 不使用本引擎 | 浏览器 DOM/JS/WASM | Jasper Web 后端负责 |

引擎核心不假设 POSIX、窗口系统或 GPU 一定存在；`PlatformHost`/`RenderBackend` 是唯一接口。

## 7. Scene 与 UI IR 的边界

- 本仓库的 `Scene` 是**实例层**：当前持久节点、样式、文本、父子关系、稳定 NodeId。
- Jasper 侧的 **声明层 UI IR**（组件组成/状态依赖）和 **增量更新协议** 不在这里。
- `Scene` 有单调 `version`；`apply` / `apply_batch` 原子应用（prototype 用 clone-on-write），
  成功只推进一次版本；失败保持原场景/版本。
- `apply_batch_at(expected_version, patches)` 拒绝过期批次（`SceneError::StaleVersion`）。
- `LayoutTree.scene_version` 与 `FrameStats.scene_version` 把版本带到宿主/事件侧。
- 当前 `ScenePatch` 只是引擎内部增量；不是跨语言 ABI，也不是最终 UI IR。
- 后续 ABI 会把“实例层场景更新”定义成版本化批量操作；见 [abi.md](abi.md)。

## 8. 布局

- 语义由 Jasper UI 定义；本仓库用 **Taffy 0.15** 作为 Native 执行器。
- 当前 PSS 子集：Flex/Block/Grid、固定/百分比/自动尺寸、min/max、padding/margin/gap、align/justify、flex_grow、边框宽度。
- 不含：阴影、滤镜、变换、Grid 全部高级特性、滚动、虚拟化。
- 文本叶子通过 `TextEngine::measure` 参与测量；真实 Parley 适配后续替换。
- Taffy 类型不暴露给 Jasper 公共 API。

## 9. 绘制与批处理

- `paint` 只产出 `PaintOp`：Clear/FillRect/StrokeRect/DrawText/PushClip/PopClip。
- `UiBatch` 是唯一后端入口；带 magic/version/op count；解码做边界/字符串/操作数上限检查。
- 后端不支持的操作返回 `BackendError::Unsupported`（例如 Headless 的 DrawText）。
- 当前不做局部失效/缓存/多图层；后续按帧调度和场景缓存补齐。

## 10. 输入：触摸、多点、触控笔

`PointerEvent`（`src/input.rs`）字段：

- `pointer_id`：多点触控/多笔区分
- `kind`：Mouse / Touch / Pen / Unknown
- `phase`：Down / Move / Up / Cancel / Hover
- `position`、`pressure`、`tilt`、`twist`、`contact`
- `is_primary`、`eraser`、`button`、`modifiers`、`timestamp_millis`

`PointerTracker` 维护活跃指针：Down 插入，Move/Hover 更新（未按下则忽略），Up/Cancel 移除；重复 Down/未知 Up 返回错误。

平台映射（后续）：

| 平台 | 原始能力 |
|---|---|
| Windows | Windows Ink / WM_POINTER：pressure、tilt、barrel、eraser |
| Apple | Apple Pencil / UIKit touch：pressure、altitude、azimuth、hover、Pencil 2/Pro |
| Android | MotionEvent：pressure、tilt、orientation、tool type、历史点 |
| Linux | libinput tablet/touch：pressure、tilt、distance、eraser |

边界：Rust 只做**规范化 + 命中/捕获**；手势识别、手掌拒绝、预测/合并事件、输入法组合由 Jasper UI 层后续实现。触控笔不能退化成鼠标事件。

当前 `hit_test` 按绘制顺序取最上层**有绘制内容**的节点；透明容器本身不吞事件，命中路径用于冒泡。`PointerRouter` 在 Down 时记录 target/path，Move/Up/Cancel 沿用 capture，即使指针移出节点；多指/多笔各自独立。

## 10.5 语义与焦点

- `Semantics` 由 Jasper 组件层提供（role/label/description/value/disabled/focusable/hidden）；
  Rust 只做宿主中立的快照与焦点遍历，不根据绘制结果反推语义。
- `SemanticsTree::from_scene` 生成树；`focusables` 排除 hidden/disabled。
- `FocusManager` 维护 pre-order 焦点顺序，支持 `focus_next`/`focus_prev`/`Tab`/`Shift+Tab`。
- 后续 AccessKit 适配把 `SemanticsNode` 转成平台无障碍树；Web 用标准 HTML/ARIA，不走这里。
- 当前没有动作执行（click/focus 动作）、没有焦点环/滚动到可见；由 Jasper UI 层后续定义。

## 11. ABI 草案

当前是 Rust API，不是稳定 ABI。计划见 [abi.md](abi.md)：固定宽度、代际句柄、批量操作、错误码、无 panic 穿越、版本协商、线程队列。

## 12. 性能与测试

- 当前无性能承诺；v0.1 只验证正确性。
- 后续指标：布局/绘制/文本/提交分段耗时、帧预算、内存、缓存命中、降级恢复。
- 测试：单元 + 端到端 + 属性/模糊（后续）+ 跨平台布局一致性（后续）。
- 软件后端保留为无 GPU/降级/视觉回归基线。

## 13. 安全与可靠性

- FFI 参数检查、句柄失效、重复释放、资源配额（ABI 实现时）。
- 图片/字体解析异常、GPU device lost、窗口销毁生命周期（后续）。
- Panic 不穿越 FFI；错误可映射回 Jasper 组件（后续）。
- Web 端的安全（HTML 注入/CSP/SSR）归 Jasper Web 后端，不在本引擎。

## 14. 当前状态

逐项见 [status.md](status.md)；阶段见 [roadmap.md](roadmap.md)。
