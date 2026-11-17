# Issue #3 架构裁决记录（已批准）

裁决日期：2026-11-17。用户于私有 issue #4 正式确认：**1A / 2A / 3A / 4A / 5A / 6A**。

本记录是架构方向批准，不表示 typed UI IR、ABI、双后端一致性或跨平台验收已经实现。

## 1. 裁决摘要

1. Reactive 能力采用 `ReactiveSignal<T>` / `Computed<T>`；保留
   `jasper.signal.SignalBus` 作为 Command/Query/Subscription 总线。
2. 公共身份采用 `UiKey { semantic_id: u64, generation: u64 }`，由 Framework 分配；
   同一身份域内旧组合永不复用，generation 耗尽显式拒绝。
3. Typed Style 是唯一规范；CSS 文本仅作导入/导出/互操作输入。
4. 公共 IR 由 Jasper Language/UI 共有层拥有；Framework 生成，Native/Web 消费；
   Rust Engine 不反向定义公共语义。
5. 语义、身份、事件、焦点和编辑状态一致；有限视觉/平台差异必须登记并有 golden case。
6. E2–E15 与 P0-A 保留为 candidate/`ir-v0` 映射输入；冻结新增公共 PSS/`xstyle` 扩张。

## 2. 强制约束

- Reactive Signal 是 Jasper 通用响应式能力，不是 Engine 或某组件库私有运行时。
- `UiKey` 必须绑定 UI 会话/身份域；跨进程事件还须带 owner/session、frame revision、
  permission context。捕获、焦点、IME、异步续体、资源身份分别准入。
- 公共属性由 Typed Property Registry 定义：类型、默认值、继承/层叠、支持等级、动画性、
  验证、降级。
- `IR → Native` 和 `IR → Web` 是规范执行映射；反向兼容 adapter 只有信息完整时才声明可逆，
  否则必须报告信息损失。
- 保留 Blitz/Parley/Vello、现有原子 Scene 事务、PSS/xstyle、JUI 2 和回滚路径。

## 3. 后续顺序

1. 固化 `ir-v0` Schema、身份/事件生命周期、Typed Property Registry。
2. 实现旧 Element/ScenePatch/JUI 2 的兼容 adapter，旧路径不删除。
3. 将 E2–E15 与 P0-A 候选能力逐项映射；未映射能力保持 candidate。
4. 同一 IR 的 Native CPU/GPU、Web SSR/浏览器 golden case。
5. 每步分别报告 Schema、测试、真实宿主、形式化证明。

## 4. 状态

| 项 | 状态 |
|---|---|
| 架构方向与六项裁决 | approved |
| `ir-v0` Schema | in progress |
| Typed Property Registry | in progress |
| 兼容 adapter | planned |
| 候选能力映射 | planned |
| 双后端 golden case | not run |
| 正式 ABI | not started |
