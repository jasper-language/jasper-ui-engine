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
- `apply_frame_with_patches` 为活动 `SceneHostView` 返回同一终态的 `ScenePatch` 批次；
- `ScenePatch::Move` 保持节点身份、焦点、指针按下与 TextArea 编辑缓冲/IME 预编辑，不用 remove+insert 近似重排；
- 维护 `UiKey ↔ NodeId`、事件 action、资源 generation 与 element kind；
- style 无法无损表达时返回 `NativeUiFrameError::Mapping(MappingIssue)`；
- revision 与 `Scene::version()` 同步推进。

## Native 映射状态

| 范围 | 状态 | 说明 |
|---|---|---|
| box/flex/paint/typography 基础子集 | implemented / tested | 设置页 fixture 经真实 Native CPU 执行 |
| 事件绑定与准入 | implemented / tested | click/text-commit、owner/session/revision/permission/generation 拒绝 |
| 活动宿主后继帧 | implemented / tested | `apply_frame_with_patches` 生成 patch，真实 SceneHostView 应用并与 detached 文档终态一致；Move 保持焦点、指针捕获和 IME/编辑状态 |
| Table/Row/Cell 语义 | implemented / tested | 公共角色、ARIA 和 AccessKit Table/Row/Cell 保留，data-table fixture Native CPU 可执行 |
| TextArea 语义与编辑 | implemented / tested | 保留 MultilineTextInput、换行输入、行内 Home/End、上下移动与多行编辑缓冲 |
| Popover/Tooltip 语义 | implemented / tested | Native 语义角色保留；AccessKit 分别投影为 Group/Tooltip，Popover 的宿主平台细节仍为 candidate |
| Selected 状态 | implemented / tested | Semantics、AccessibilityNode、ARIA 与 AccessKit selected 状态贯通 |
| Grid、Position、Image、Overlay | candidate | 可映射到现有候选字段，`host_accepted = false` |
| 部分 flex 组合 | structured reject | wrap、shrink、baseline、space-around/evenly 目前无 Native 等价字段 |
| 部分文字属性 | structured reject | 非默认水平 align、显式 bidi direction、baseline vertical align |
| element attributes | structured reject | v0.1 Native profile 无通用属性袋 |

拒绝使用字段路径、property、support level、issue kind 和 detail。candidate 映射不会被改写成
`required` 或已宿主验收能力。

## Fixtures

`tests/fixtures/ui-ir-v0/` 与 appfw `codex/ir-v0@1e55d96` 的三份 fixture 字节一致，并在测试中
固定 FNV-1a 摘要。当前覆盖：

- `settings-page.ir.json`：Schema 解析、Native 映射、真实 SceneHostView、语义、几何和 CPU 渲染；
- `data-table.ir.json`：Schema 解析、Table/Row/Cell 语义、真实 SceneHostView 与 CPU 渲染；
- `dialog-overlay.ir.json`：Schema 解析、candidate overlay/position、真实 SceneHostView overlay 查询与 CPU 渲染。

## 证据状态

| 维度 | 状态 |
|---|---|
| Schema | appfw v0.1 draft；Rust consumer profile 已实现 |
| Rust structural tests | passed |
| Native CPU same-fixture execution | all three canonical fixtures passed on Linux x86_64 test environment |
| Native GPU same-fixture | settings-page passed with llvmpipe/Vulkan; max channel delta 1 over 170/153600 pixels |
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

Native GPU fixture 对照（软件 Vulkan）：

```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
WGPU_BACKEND=vulkan \
/home/auth/.cargo/bin/cargo test --locked \
  --features native-engine-cpu,accesskit-pending,gpu-pending \
  --no-default-features --test ui_ir_v0 \
  same_settings_fixture_executes_on_native_gpu_and_tracks_cpu
```

CPU 测试只证明当前 Linux 环境下该 fixture 可构造并产出非空帧；不证明正式平台验收、GPU 或
Web 等价。Native GPU 对照需启用 `gpu-pending` 并通过 lvp/llvmpipe；该运行只证明软件 Vulkan
上的同 IR CPU/GPU 像素差上限为 1，不证明真实硬件、窗口呈现或跨平台验收。
