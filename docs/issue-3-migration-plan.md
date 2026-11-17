# Issue #3 渐进迁移与风险方案（建议稿）

## 1. 当前候选实现处置

以下提交保留为 private 候选能力，不构成公共语义批准：

- 图片/SVG、浮层、Grid/Table/Position、样式请求协商、主题事务测试；
- 输入撤销/重做、RTL、AccessKit 文字动作等未提交工作保持隔离。

不得据此删除 PSS、改 public ABI 或重写 Web backend。候选实现先进入审计映射表。

## 2. 阶段

### P0：冻结与基线

- 冻结新增 public `xstyle`/PSS 字段；
- 建立现有 Style/JUI2/ScenePatch/Web CSS 的属性映射表；
- 为每个属性标记 Required/Optional/HostOwned/WebOnly/NativeOnly/Unsupported；
- 建立三套主题当前降级报告。

### P1：Typed IR v0

- 固化 Element/UiKey/Style groups/Resource/Env/Event/Ack/Reject；
- 为 JUI 2 与 Rust ScenePatch 写 IR adapters；
- IR validator 在前，backend mapping 在后；
- 旧 JUI 2 wire 不变，保证现有 Native 宿主继续运行。

### P2：Framework 迁移

- `Element.layout`/Style mirror 改为 typed style；
- Runtime diff 输出 typed UiFrame；
- JUI 2 encoder 成为 Native transport；
- HtmlRenderer 消费 typed IR/CSS mapping，不再读 Engine Style 词表；
- 保留 `State<T>/Derived<T>` 作为当前 UI 响应式 API；不把改名或通用标准库抽象作为迁移前置条件。

### P3：后端映射

- Native：UiFrame → validated ScenePatch/Style 内部映射；
- Web：UiFrame/style → deterministic DOM/CSS mapping；
- 相同能力使用同一属性定义和 fallback；
- 未知能力按 backend hello/structured rejection 处理。

### P4：双后端黄金用例

- 设置页、表格、表单、Dialog/Menu/Popover；
- Native CPU、Native GPU、Web SSR、真实浏览器；
- 节点/语义/焦点/编辑/滚动身份与像素/结构差异量表；
- 真实 Linux 宿主先跑，Windows/macOS 环境另报。

### P5：兼容与弃用

- 旧 PSS Style 标记为 Native compatibility/internal；
- E2–E15 与新 P0-A 候选能力逐项映射到 typed property；
- 无直接对应者保留 backend extension；
- 发布多个版本周期后再决定删除/重命名；
- 每个阶段保留旧 encoder 和回滚开关。

## 3. 不做

- 不一次性重写 Blitz/Parley/Vello；
- 不在 Jasper 实现浏览器 DOM/CSSOM；
- 不让 Native Engine 复制 Signal runtime；
- 不为了“统一”把 Web-only 功能塞给 Native；
- 不删除仍有调用方的 `Style`/JUI 2；
- 不以静态 mockup 或文档代替双后端运行。

## 4. 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| NodeId 回收导致旧事件命中 | 编辑/点击错投 | UiKey generation + revision-bound event |
| Double Signal 命名 | API/文档混淆 | 保留 State/Derived 与 SignalBus，禁止混称 |
| 双语义期 | 同一功能两套映射 | IR adapter + 单一 property registry |
| Web CSS 手写分叉 | Native/Web 外观和可访问性不同 | typed mapping + golden cases |
| 主题在 backend 重算 | 品牌主题泄漏、双后端差异 | Framework 先解析 theme，IR 只含最终语义 |
| 资源 generation 漏检 | 旧图片/字体复活 | Prepare/Commit/Release + generation |
| 输入态迁移失败 | 主题切换丢 IME/选区/撤销 | frame transaction identity tests |
| 预算降级静默 | 跨后端语义不明 | Required/Optional + structured degradation |
| 性能回退 | typed IR 额外复制/映射 | borrowed frames, op-count/timing budget |
| 历史证明失效 | verification 状态失真 | verified regions frozen; migration separately logged |

## 5. 回滚

- 新 IR 通过 `ir-v0` feature/selector 启用；
- 旧 JUI 2/ScenePatch encoder 保留；
- Native raw Style 与 typed Style 双向映射有 round-trip tests；
- Web 可在 typed IR path 与旧 Element path 间按页面切换；
- 任一 backend validator/reject 回归可关闭新 path，不影响现有发布。

## 6. 完成定义映射

issue #3 的五个研究交付分别对应：

- 本文 + `issue-3-baseline-audit.md`：边界与矩阵；
- `issue-3-semantic-scope.md`：兼容范围；
- `issue-3-signal-ui-ir-contract.md`：Signal/IR 契约；
- 双后端黄金用例：尚需执行，当前只能明确未运行状态；
- 本文：迁移、风险和回滚。

在这些文档被审阅并作出争议项裁决前，不开始大规模代码迁移。
