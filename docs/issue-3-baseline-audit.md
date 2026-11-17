# Issue #3 基线审计：HTML/CSS-like 语义 + Jasper Signal

状态：2026-11-17 架构研究基线；**不是已批准的 API/ABI，也不要求立即重构**。
来源：issue #3、引擎 `docs/architecture.md`、`api.md`、`batch-boundaries.md`、
`jasper-appfw/docs/CONTRACT-v0.md`、`CONTRACT-v1.md`、`jasper.ui/README.md`、
`jasper.ui.html/HtmlRenderer.jas`、`jasper-ui-host` JUI 2 读取器。

## 1. 结论

职责边界总体正确：组件、State/Derived、主题和业务事件在 Jasper；Blitz/Parley/Vello
在 Native 引擎；窗口、输入、IME、剪贴板和平台无障碍在宿主。

真正的问题是中间层没有独立出来：

```text
当前：Jasper Element + Theme ⟶ Jasper Style 镜像/ScenePatch ⟶ Native ScenePatch/Style
                         └──── JUI 2 文本 ───────────────┘
                              同时 Web SSR 手写 Style → CSS
```

`Element`、`ScenePatch`、JUI 2 和 Rust `Style` 都各自合理，但被连成了一条公共语义链。
因此每加一种视觉能力，往往要同步修改 Framework 词表、JUI 2、Host、Rust Style、
Blitz 映射和 Web CSS 映射。这正是 issue #3 指出的长期风险。

目标结构应为：

```text
Jasper Element + Reactive Signal
        ↓ 依赖追踪 / 批量合并
版本化类型化 UI IR
        ↓                         ↓
Native backend mapping       Web backend mapping
Scene/Blitz/Parley/Vello     DOM/CSS/events
```

JUI 2 和 Rust `ScenePatch` 都只能作为 backend transport/execution snapshot，
不能继续充当唯一公共语义。

## 2. 可复用、冲突与缺失

| 层/能力 | 当前实现 | 判定 | 原因/处理 |
|---|---|---|---|
| `Element` 树 | kind、role、variant、size、content、semantics、children、bindings、key | **可复用** | 已具备中立声明骨架；需改为 typed IR 的声明模型，移除对 Engine Style 字符串的依赖 |
| `State<T>` / `Derived<T>` | 依赖追踪、惰性、动态依赖、批处理、卸载清理 | **可复用** | 与 reactive Signal 需求最接近；当前名称和 `jasper.signal` Command/Query 总线冲突，需正式命名裁决 |
| `Runtime` reconciler | 事件/异步续体批次、父先子、稳定 key、最小补丁、主题换样式 | **可复用** | 输出目标应从 Engine `ScenePatch` 改为 typed UI IR frame |
| Component/SignalBus | Command/Query bus、取消、授权上下文 | **可复用但概念不同** | 这是 application SignalBus，不是 reactive dependency Signal；禁止混称 |
| JUI 2 | 帧、Insert/Style/Semantics/Text/Scroll/Remove、事件、hello/env | **保留为 Native transport** | 可继续承载 Native host 兼容；不能作为 Web 契约，也不应新增公共视觉语义 |
| Rust `Scene` / `SceneUpdate` | 稳定 NodeId、版本、原子候选事务、结构化拒绝 | **保留为 Native execution snapshot** | 是当前最佳事务基线；其 NodeId 不是跨帧/跨后端稳定语义 ID |
| Rust `Style` / PSS | 基础布局 + E2–E15 + P0-A 增量 | **内部/兼容层，冻结新公共增长** | E2–E15 实现应保留；公共侧应映射 typed style，未知新能力不得直接加 xstyle |
| `xstyle` | 圆角、阴影、渐变、材质、变换、过渡、文字、图片、浮层、Grid/定位等 | **Native 内部兼容协议** | 现有键保留兼容；新增能力先进入 typed style/IR，禁止继续扩大公共 xstyle 面 |
| Image/SVG | Engine 资源注入、额度、缓存、CPU/GPU 绘制 | **可复用实现，缺少 IR 语义** | 资源引用、generation、生命周期和缺失回退需成为独立资源通道，不放在 style 字符串 |
| Overlay/Dialog | Engine `Style.overlay`、焦点/模态/关闭请求 | **候选实现，缺少上层语义** | UI IR 需要 overlay layer/锚点/生命周期；不能假定 Web 与 Native 都通过同一 `Style.overlay` 字段 |
| Grid/Table/Position | Engine Style + Blitz CSS 映射 | **候选实现，未获架构裁决** | 先纳入 typed layout scope；Web 映射尚未覆盖，不能宣称双后端一致 |
| Web SSR | `Element`+Theme → HTML，手写 Style→内联 CSS，无 JS 表单 | **可复用但语义分叉** | Web 必须消费同一 typed IR/style mapping；HTML tag 提示不等于正式元素语义 |
| 资源身份 | Font bytes、Image resource id、Surface generation 分散 | **缺失统一契约** | UI IR 需区分 semantic node、resource key、host generation 和 backend handle |
| 事件身份 | scene version + NodeId；事件回传后按 id 查找 | **有风险** | 删除后同 id 可复用；跨 transport 应绑定 node generation/frame revision |
| ABI/transport | JUI 2 + host shadow tree；尚无冻结跨语言 ABI | **符合待实现状态** | 不把文本行格式误作 typed IR 或稳定 ABI |

## 3. 当前真实数据链

Native：

```text
Component.render() → Element
→ Theme.resolve(StyleQuery) + Element.layout
→ Jasper Style (EngineStyle)
→ Runtime diff → ScenePatch/SceneUpdate
→ JUI 2 or host adapter
→ Rust HostCore → ScenePatch → SceneHostView / Blitz
```

Web：

```text
同一 Component.render() → Element
→ Theme.resolve + Element.layout
→ HtmlRenderer.styleOf
→ 手写 Engine Style 词表 → CSS
```

共同点只在组件声明早期；样式与更新语义在进入后端前已经分叉。所谓“同一套 UI”
目前主要证明组件业务逻辑与 HTML 结构复用，尚未证明同一 typed style/IR 在 Native/Web
执行语义一致。

## 4. 现有证据能说明什么

已存在：

- JUI 2/HeadlessHost 版本、原子帧、能力握手、style/xstyle、事件和 resync 测试；
- Native CPU/GPU 渲染、帧事务、焦点、编辑、AccessKit、图片、浮层、Grid/Table 候选测试；
- appfw 主题映射、三家族样式、native 视觉回归和 Web SSR 表单回归。

尚未存在：

- 独立 typed UI IR；
- 同一 IR/style 输入的 Native/Web golden 双后端执行；
- 跨帧 node generation；
- Web 对 P0-A 新能力（图片/SVG、覆盖层、Grid/定位、真实 IME/编辑动作）的等价映射；
- Signal → dependency graph → IR diff 的形式化生命周期契约；
- HTML/CSS-like 兼容范围与允许差异的正式裁决。

## 5. 审计裁决建议

1. 保留 Blitz/Parley/Vello、现有 Scene 事务和 E2–E15 实现。
2. 立即冻结新的公共 `Style`/`xstyle` 增长；新能力先进入 typed style/IR 提案。
3. JUI 2 与 Rust `ScenePatch` 明确降为 Native transport/execution snapshot。
4. 先定义 typed UI IR v0，再做任何 PSS 删除、字段迁移或 Web 重写。
5. `jasper.signal`（Command/Query bus）与 reactive Signal 必须使用不同正式名称。
6. 未通过双后端 golden case 的能力只能标记为 Native candidate，不能标记跨后端完成。
