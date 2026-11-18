# Typed UI IR v0.1 Rust consumer

状态日期：2026-11-18。

本文记录 Rust UI 引擎对 Jasper Typed UI IR 的消费能力。公共 Schema 与语义权威仍属于
Jasper Language/UI 共有层；Rust 不反向定义公共语义。

## 权威与边界

- Canonical Schema：appfw `docs/ui-ir-v0/schema/ui-ir-v0.schema.json`，Rust 夹具来源提交
  `1e55d96`。
- `UiKey { semantic_id, generation }` 是公共节点身份；`scene::NodeId` 只在 Rust Native
  文档映射内部使用。
- Typed Style 是输入规范；本模块不新增 PSS/`xstyle` 公共字段。
- JSON 只作为 v0.1 交换表示。进入 Native 前先完成严格反序列化和结构校验。
- 当前没有发布正式 ABI，也不声明 Native/Web 双后端一致或跨平台验收。

## Rust 实现

`src/ui_ir/mod.rs` 提供：

- `UiFrame`、`UiOp::{Insert, Update, Move, Remove, Resource, Env}`；
- `UiNode`、`UiElement`、`UiSemantics`、`EventBinding`；
- `.box`、`.flex`、`.grid`、`.position`、`.paint`、`.typography`、`.image`、
  `.overlay` typed style groups；
- 严格字段、range、重复 `UiKey`、revision successor、颜色与非有限数值检查；
- `UiColor` 的 `#RRGGBB` / `#RRGGBBAA` 精确解析。

`src/ui_ir/identity.rs` 提供单 identity domain 的 reference generation ledger：

- 首次分配 generation 1；
- retirement 后只分配 `last + 1`；
- 旧组合不可复用；
- identity domain 为空时拒绝构造 ledger；
- `u64::MAX` 显式返回 `GenerationExhausted`，不回绕。

`src/ui_ir/events.rs` 要求事件同时绑定：

- `UiKey`；
- identity domain、owner、session；
- 当前 frame revision；
- named permission context 与事件所需 permission。

裸 `NodeId` 不进入事件授权入口。

`src/ui_ir/native.rs` 提供 `NativeUiDocument`：

- 先在 detached clone 上完成整帧映射，成功后替换；
- 支持 insert/update/move/remove/resource/environment；
- 维护 `UiKey ↔ NodeId`、事件 action、资源 generation 与 element kind；
- style 无法无损表达时返回 `NativeUiFrameError::Mapping(MappingIssue)`；
- revision 与 `Scene::version()` 同步推进。

## Native 映射状态

| 范围 | 状态 | 说明 |
|---|---|---|
| box/flex/paint/typography 基础子集 | implemented / tested | 设置页 fixture 经真实 Native CPU 执行 |
| 事件绑定与准入 | implemented / tested | click/text-commit、owner/session/revision/permission/generation 拒绝 |
| Grid、Position、Image、Overlay | candidate | 可映射到现有候选字段，`host_accepted = false` |
| 部分 flex 组合 | structured reject | wrap、shrink、baseline、space-around/evenly 目前无 Native 等价字段 |
| 部分文字属性 | structured reject | 非默认水平 align、显式 bidi direction、baseline vertical align |
| table/table-row/table-cell 语义 | structured reject | 现有 Rust `Semantics::Role` 不能无损表达 |
| element attributes | structured reject | v0.1 Native profile 无通用属性袋 |

拒绝使用字段路径、property、support level、issue kind 和 detail。candidate 映射不会被改写成
`required` 或已宿主验收能力。

## Fixtures

`tests/fixtures/ui-ir-v0/` 与 appfw `codex/ir-v0@1e55d96` 的三份 fixture 字节一致，并在测试中
固定 FNV-1a 摘要。当前覆盖：

- `settings-page.ir.json`：Schema 解析、Native 映射、真实 SceneHostView、语义、几何和 CPU 渲染；
- `data-table.ir.json`：Schema 解析；Native 对不可表达的 table role 显式拒绝；
- `dialog-overlay.ir.json`：Schema 解析、candidate overlay/position 映射与状态保留。

## 证据状态

| 维度 | 状态 |
|---|---|
| Schema | appfw v0.1 draft；Rust consumer profile 已实现 |
| Rust structural tests | passed |
| Native CPU same-fixture execution | passed on Linux x86_64 test environment |
| Native GPU same-fixture | not run in this slice |
| Web SSR / real browser | not run in this slice |
| Real platform host acceptance | not run |
| Formal verification | not performed for UI IR validation/mapping/event admission |
| Cross-backend equivalence | not established |

## 复现

默认结构/映射测试：

```bash
/home/auth/.cargo/bin/cargo test --test ui_ir_v0
```

Native CPU fixture 执行：

```bash
export JASPER_UI_TEST_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
/home/auth/.cargo/bin/cargo test --locked \
  --features native-engine-cpu,accesskit-pending \
  --no-default-features --test ui_ir_v0
```

CPU 测试只证明当前 Linux 环境下该 fixture 可构造并产出非空帧；不证明正式平台验收、GPU 或
Web 等价。
