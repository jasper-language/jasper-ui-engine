# Jasper Signal → Typed UI IR → 双后端契约（建议稿）

## 1. 术语裁决

当前仓库存在两个“Signal”概念：

1. `jasper.signal.SignalBus`：Command/Query/Subscription 应用消息总线；
2. `jasper.ui.State/Derived`：UI 响应式依赖追踪。

Issue #3 中的响应式能力当前继续由 `jasper.ui.State<T>` / `Derived<T>` 承担。
用户已修正裁决：暂不新增 `ReactiveSignal<T>` / `Computed<T>` 名称、语言关键字或专用语法；
是否将响应式机制抽为 Jasper 通用标准库，须在 UI 外出现真实可验证需求后另行研究。
`jasper.signal.SignalBus` 保持独立。文档使用“UI 响应式状态”或明确写出
`State/Derived`，避免只写含糊的 “Signal”。

## 2. 所有权

- Jasper UI Framework 持有 `State/Derived`、依赖图、组件状态、主题和业务身份；
- Native/Web Engine 不维护业务 Signal，不重新执行组件 render；
- Engine 只接收已合并、带 revision/generation 的 UI IR frame；
- 平台 Host 持有窗口、IME、剪贴板、字体来源和系统无障碍桥。

## 3. 更新链

```text
Host event / async completion
  → authenticated event target (semantic id + generation)
  → Signal writes / task completion in ReactiveScope
  → dependency invalidation
  → dirty component render, parent before child
  → typed UI IR diff
  → atomic frame validation
  → Native mapping OR Web mapping
  → ack / structured reject
```

同一事件/异步续体只形成一个 batch。render 读取全部写入后的值，不暴露中间态。
卸载先封闭事件/异步准入，再取消任务、退订依赖、回收 backend node 和资源。

## 4. 类型化 IR v0

建议形状（语言无关概念）：

```text
UiFrame {
  ir_version,
  expected_revision,
  next_revision,
  ops: [UiOp]
}

UiOp =
  Insert { parent: UiKey?, index, node: UiNode },
  Update { key: UiKey, fields: UiFields },
  Move { key: UiKey, parent, index },
  Remove { key: UiKey, expected_generation },
  Resource { key: ResourceKey, generation, admission, source },
  Env { appearance, density, text_scale, reduce_motion, contrast }

UiKey = { semantic_id: u64, generation: u64 }

UiNode {
  key, element, semantics, style, state, children, events, resources
}
```

`semantic_id` 是业务身份，`generation` 防止删除/重建后旧事件命中新节点。
backend handle、Blitz NodeId、DOM id、JUI 2 数字 id 都不是公共身份。

## 5. Style 表达

公共 style 使用 typed groups：

```text
BoxStyle / FlexStyle / GridStyle / PositionStyle / PaintStyle /
TextStyle / ImageStyle / MotionStyle / MaterialStyle / OverlayStyle
```

每个属性包含：

- typed value；
- support level；
- 可选 fallback；
- 是否可动画；
- 默认值。

当前 `xstyle` 字符串只用于 Native compatibility encoder。Web encoder 将同一 typed property
映射为 CSS；不得继续维护第二套手写语义表。

## 6. 资源契约

- Resource key 由 Framework/宿主创建，包含 id + generation + kind；
- Engine 从不自行网络获取；
- Prepare/Commit/Release 有明确结果；
- 过期 generation 拒绝；
- 缺失图片有 alt/placeholder/fallback，不得到静默空白；
- font/image/icon/material source 都通过同一资源身份模型；
- Web 映射为可审计 URL/bytes scope，Native 映射为 host-owned bytes/resource。

## 7. 帧确认

成功：`Ack { revision }`。

失败：`Reject { current_revision, code, path, detail, recoverable }`。

运行时保留最后一次已确认快照；reject 后重发整帧/快照，不把未确认帧状态当作已呈现。
Native 现有候选场景事务可作为实现基线，但 typed IR validator 必须位于后端映射之前。

## 8. 生命周期与顺序

- 事件先查 frame revision、node generation、enabled/visible/authority；
- focus 迁移先关闭旧文本/IME 所属关系，再建立新关系；
- pointer down 后 capture 绑定旧 target generation；
- overlay 关闭先发 dismiss request，Framework 提交移除帧；
- 主题/密度/语言变化是 style/semantic IR frame，不重建节点；
- 动画由 backend 执行 typed motion，Framework 不发送逐帧 patch。

## 9. 可观测性

每帧至少可查询：

- State/Derived write/invalidated/dirty/render counts；
- IR op counts by kind；
- reject path/code；
- resource cache/eviction/missing；
- material/frame budget degradation；
- backend mapping version；
- event→ack latency 和 frame revision。

## 10. 待用户裁决

1. 跨领域响应式标准库的模块归属与命名（独立研究，不是 `ir-v0` 前置条件）；
2. UiKey generation 是否能由现有 NodeId + frame ledger 推导，还是需要新 public id；
3. CSS 文本是否只作为导入/导出工具，还是需要运行时入口；
4. 允许的 Native/Web 视觉差异和阈值；
5. `jasper.signal` 是否改名以消除 double Signal 概念。
