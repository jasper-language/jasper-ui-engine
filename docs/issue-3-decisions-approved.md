# Issue #3 架构裁决记录（已批准）

裁决日期：2026-11-17。用户于私有 issue #4 正式确认第 2–6 项为 A，并补充修正第 1 项：
**保留现有 `jasper.ui.State<T>` / `Derived<T>`，暂不引入 `ReactiveSignal<T>` / `Computed<T>` 公共名称。**

本记录是架构方向批准，不表示 typed UI IR、ABI、双后端一致性或跨平台验收已经实现。

## 1. 裁决摘要

1. 保留 `jasper.ui.State<T>` / `Derived<T>` 作为当前响应式 API 基线；依赖追踪、动态依赖、
   批量更新和卸载清理复用并验证现有实现。暂不新增 `ReactiveSignal<T>` / `Computed<T>` 公共名称，
   也不新增语言关键字或专用语法。`jasper.signal.SignalBus` 保持独立，负责
   Command/Query/Subscription 应用消息传递。跨领域响应式标准库归属留作独立研究事项。
2. 公共身份采用 `UiKey { semantic_id: u64, generation: u64 }`，由 Framework 分配；
   同一身份域内旧组合永不复用，generation 耗尽显式拒绝。
3. Typed Style 是唯一规范；CSS 文本仅作导入/导出/互操作输入。
4. 公共 IR 由 Jasper Language/UI 共有层拥有；Framework 生成，Native/Web 消费；
   Rust Engine 不反向定义公共语义。
5. 语义、身份、事件、焦点和编辑状态一致；有限视觉/平台差异必须登记并有 golden case。
6. E2–E15 与 P0-A 保留为 candidate/`ir-v0` 映射输入；冻结新增公共 PSS/`xstyle` 扩张。

## 2. 强制约束

- 当前 UI 响应式能力继续由 `State/Derived` 承担；是否抽成 Jasper 通用标准库另行研究，
  不作为 `ir-v0` 前置条件。
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
| `ir-v0` Schema | Language/UI draft；Rust Native consumer implemented / tested |
| Typed Property Registry | upstream registry pinned；Rust profile implemented / tested |
| 兼容 adapter | `UiFrame → ScenePatch`、受限 Native Style ↔ Typed Style、raw Legacy Scene → UiNode（显式 identity + loss report）、Native Style/Semantics → JUI 2 键值、NativeUiDocument/SceneUpdate → JUI 2 initial/successor frame、UiKey → JUI 2 event 行、UiNode 子树/资源/环境/identity 快照及直接文档恢复 implemented / tested；三份 canonical initial frame、settings initial+successor 双帧流和 typed event lines 已与旧 Host CPU reader/formatter 对齐，真实窗口 CPU softbuffer smoke 产生完整 pointer/focus/click，交互流经 host 注入 text input/Enter 产生 `text-commit Alice Bob`；Xvfb X11/XTest 系统窗口级鼠标与键盘事件经 winit/`DesktopInput` 完成 Save 激活和文本 revision 推进，IBus/libpinyin XIM 预编辑与提交得到 `External display 中文你好`；Intel/NVIDIA 硬件 Vulkan 离屏 `ui_ir_v0` 通过，NVIDIA 硬件窗口单帧 present 通过；desktop-accessibility host 在 AT-SPI 总线发布 Settings/Name/Save 注册树，Save `DoAction` 返回 true 并映射为 JUI 2 click，Intel 窗口受 Surface/adapter queue family 限制；Table role/Move/Scene events 等损失仍显式报告；无条件可逆 adapter、物理桌面/真实 Wayland 合成器输入与 IME、硬件窗口完整无障碍及 Intel 硬件窗口验收 not complete |
| 候选能力映射 | implemented / tested；Grid/Position/Image/Overlay 仍 candidate、`host_accepted=false`，不因映射已执行而提升为宿主验收 |
| 双后端 golden case | not run |
| 正式 ABI | not started |
