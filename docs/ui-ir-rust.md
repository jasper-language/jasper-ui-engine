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
- `UiIdentityLedger::snapshot` / `from_snapshot` 显式保留并恢复每个 semantic id 的
  last generation 与 live/retired 游标，不重新分配历史代际；
- `NativeUiDocument` 在每个 insert/remove 中原子 claim/retire 精确 `UiKey`；
- identity domain 为空时拒绝构造 ledger；
- `u64::MAX` 显式返回 `GenerationExhausted`，不回绕。

`src/ui_ir/events.rs` 要求事件同时绑定：

- `UiKey`；
- identity domain、owner、session；
- 当前 frame revision；
- named permission context 与事件所需 permission。

裸 `NodeId` 不进入事件授权入口。
`admit_native_event` 可将 Native 回调的 `NodeId` 仅作为内部查找提示；授权仍要求匹配
`UiEventContext.target`。`admit_activation` 明确区分 pointer click、keyboard 与 accessibility，
pointer 优先 `click`，其余使用 `activate`，不会把未绑定的键盘动作静默映射过去。
Focus、pointer capture、IME、scroll 等非 binding 操作使用 `admit_native_target`，分别要求
对应的 permission；同样不能用裸 NodeId 绕过 UiKey 和会话上下文。
Pointer 命中查询先经过 owner、frame revision、Surface generation、overlay 遮挡、圆角排除和
hit-slop 修正，再映射为 `UiKey`。实际 Pointer 路由会把每个派发事件转换为
`TypedUiPointerEvent`；其中 `target` 始终是 `UiKey`，backend `NodeId` 不进入回调。
Pointer Down/Up/Move 的绑定按实际接收节点单独准入；嵌套内容的点击若激活源祖先控件，
该激活绑定也按真实控件身份准入，不会错误地强迫回调目标等于最初的 Pointer 命中节点。
Focus 切换、AccessKit Focus 与 Pointer Down 若会触发失焦提交或结束正在进行的组合，
`text-commit`/`ime-cancel` binding 会在 Native 焦点变化前按实际将发生的编辑事件预准入；
`typed_clear_focus` 以同样顺序只清除焦点并保留按下/悬停；`typed_deactivate` 则清除焦点、
按下和悬停，两者都只向调用方返回旧焦点的 `UiKey`。
`typed_key`/`typed_key_with_activation` 对会改动编辑缓冲或选区的按键在默认编辑行为前要求
TextEdit；TextArea 的 ArrowUp/ArrowDown 也按编辑键处理；Enter 产生提交时预准入
`text-commit`，不能只凭 Keyboard 修改文本框。Tab/Shift+Tab 在任何 Native 焦点移动前要求
Focus permission，并预准入所有可达 Focus binding 与可能的失焦编辑结束。
Focus、Key、Pointer、键盘激活和 AccessKit 动作的 Typed 回调分别使用
`TypedUiFocusEvent`、`TypedUiKeyEvent`、`TypedUiPointerEvent`、
`TypedUiActivationEvent`，只携带 `UiKey`；如果 Native 回调目标无法映射回当前文档，
操作返回 `UnmappedTarget`，不会把 backend id 透传给调用方。
E14 编辑事件通过 `typed_take_edit_events` 取出：事件 revision 可来自已提交帧事务中的前一 revision，
但不得新于当前文档；
`Changed`（live-text 模式）、`Commit` 和 `CompositionCancelled` 分别准入
`text-commit`/`ime-cancel`；返回值只含 `TypedUiEditEvent`，不暴露 NodeId。
整批事件先完成 UiKey 投影和授权，全部成功后才从 Native 出站队列移除；授权失败时批次保留，
修正 permission context 后可重试。`typed_take_overlay_events` 对 `Dismiss` 使用同一事务性顺序。
当前编辑会话由 `typed_edit_state` 查询，返回 `TypedUiEditState`；文本、选区、预编辑和
caret 几何与 `UiKey` 一并返回，不暴露 backend 节点身份。

`src/ui_ir/native.rs` 提供 `NativeUiDocument`：

- 先在 detached clone 上完成整帧映射，成功后替换；
- 支持 insert/update/move/remove/resource/environment；
- `apply_frame_with_patches` 为活动 `SceneHostView` 返回同一终态的 `ScenePatch` 批次；
- `ScenePatch::Move` 保持节点身份、焦点、指针按下与 TextArea 编辑缓冲/IME 预编辑，不用 remove+insert 近似重排；
- 每帧从最终 Scene 重建 image 引用；未 Prepare/Commit、generation 不匹配、仍被节点引用时 Release 均整帧拒绝；
- 维护 `UiKey ↔ NodeId`、事件 action、资源 generation 与 element kind；
- `NATIVE_PROPERTY_PROFILE` 是公共 Typed Property Registry 的只读 Native consumer 镜像，
  记录 property、support 与 `host_accepted`；它不是第二套公共语义权威；
- style 无法无损表达时返回 `NativeUiFrameError::Mapping(MappingIssue)`；
- revision 与 `Scene::version()` 同步推进。

`src/ui_ir/compat.rs` 提供受限的 legacy Native 兼容投影：

- `project_native_style(&NativeStyle, ElementKind)` 返回 `NativeStyleProjection`；
- `project_legacy_scene(&Scene, &BTreeMap<NodeId, UiKey>)` 执行 raw Legacy Scene 的损失感知
  反向投影；每个 NodeId 必须显式映射，缺失或重复 UiKey 拒绝。Scene 不保存事件绑定，
  也不保存原始 ElementKind；element kind 只能推断并逐节点进入 losses。image 没有
  generation、事件绑定无法恢复；Canvas 因 Typed v0.1 无对应 kind 拒绝；
- `project_native_style_to_jui2(&NativeStyle)` 将可表达的基础字段投影为 JUI 2 `style`
  键值、扩展字段投影为 `xstyle` 键值，并报告 JUI 2 wire 不支持的 display、wrap/shrink、
  text-align/direction 等字段；扩展键仍需宿主 hello 协商；
- `project_native_semantics_to_jui2(&Semantics)` 投影基础 role、heading level、live
  politeness 和常用语义字段；TextArea/Table/Popover/Tooltip/selected 等 wire 缺口显式报告；
- `project_jui2_initial_frame(&NativeUiDocument)` 生成原有 JUI 2
  `frame/insert/text/style/xstyle/sem/end` 文本，统一转义 `\\`、tab 与换行，并执行 9 位
  transport ID 边界检查；UiKey、事件绑定、资源 payload/generation、environment 和当前
  revision 只进入 losses。该入口仅生成 fresh initial frame，不提供增量或可逆 adapter；
- `project_jui2_scene_update(&NativeUiDocument, &NativeSceneUpdate)` 生成后继 frame，
  覆盖 insert/remove/style/xstyle/semantics/text/scroll；JUI 2 没有 Move，使用
  remove+insert 兼容近似并明确报告焦点/按下状态损失；
- `project_jui2_event(&NativeUiDocument, &Jui2Event)` 将 active `UiKey` 映射到 transport
  NodeId，编码 hover/pointer/click/focus/key/text-commit/ime-cancel/dismiss-request
  事件，并复用 JUI 2 payload 转义；
- `examples/project_jui2.rs` 是只读诊断入口，默认输出 settings，也可指定任一 IR fixture
  作为 JUI 2 初始 frame 供旧 host reader 回环；
- `project_native_style_with_context` 接受节点精确的 image `ResourceRef` 与 overlay 锚点
  `UiKey`，可无损投影相应 candidate 字段；
- `NativeUiDocument::project_compatibility_style(UiKey)` 自动从当前节点映射这两项上下文；
- `NativeUiDocument::project_compatibility_node(UiKey)` 递归恢复 `UiNode` 子树、element、
  semantics、events、children 和每节点 style，并以 `UiKey` 为键聚合 `NativeNodeLoss`；
- `project_compatibility_snapshot()` 返回 authority、identity 游标、revision、root、
  environment 以及当前 active/prepared resource 的只读文档快照；
- `NativeUiCompatibilitySnapshot::validate()` 在恢复前统一校验 authority、UiNode 结构、
  live identity 集合、environment、Native Property Registry、resource id/generation/kind
  及 active/prepared 生命周期可达性，并交叉核对 image 必须精确引用 committed Image/SVG、
  node overlay anchor 必须指向 live identity；
- `NativeUiCompatibilitySnapshot::to_json_vec()` / `from_json_slice()` 提供严格 JSON
  round-trip；固定格式标识为 `jasper-ui-native-compatibility/1`。未知字段、未知 property、
  重复 loss identity、错误格式版本和无效快照均结构化拒绝。该格式是 Rust compatibility
  envelope，不是公共 ABI；
- `NativeUiCompatibilitySnapshot::to_initial_frame()` 仅在全部身份为 generation 1 时重建
  `expected_revision=1` 的新会话帧；存在淘汰历史时结构化拒绝，不伪造 identity ledger；
- `NativeUiDocument::from_compatibility_snapshot` 可直接按显式 identity 游标恢复 active
  NativeUiDocument，支持当前 revision（至少 2）与 generation > 1 的 live keys；
- box/flex/Grid/position/paint/typography 中当前 Typed v0.1 可表达的字段会投影；
- Native shadow、单侧 border、outline、text decoration、hit-slop、transform、motion、
  gradient、material、image、overlay 等字段逐项进入 `losses`；
- 只有 Container/ScrollView 可通过 Grid style 的存在选择 Grid display；Table/Row/Cell
  等元素上的 Native Grid display 会进入 `losses`，即使其 grid 字段仍可表示；
- image/overlay 在没有 exact resource/UiKey 上下文时不会伪造身份，而是保留为损失；
- 只有 `is_lossless() == true` 才表示本次投影没有遗漏非默认 Native 字段。

恢复 API 默认只接受 `losses` 为空的快照；含 Native-only 字段遗漏时必须先补齐或明确改用诊断
流程，不能静默恢复成语义不同的文档。快照会把可表达 Native 默认显式化，并对无子节点/空事件
列表使用规范化 `None`；空事件不再因内部空 map 泄漏成 `Some([])`。全文档快照
包含 identity retirement 游标，但只包含当前 resource 状态，不包含 released resource
tombstones/history、Framework 组件声明、Signal 依赖、事件会话或正式 ABI。Grid 投影仍继承
candidate/`host_accepted=false` 状态。

### E2–E15 / P0-A 映射矩阵

| 既有能力 | Rust Native consumer 状态 |
|---|---|
| E2 corner radius | Native ↔ Typed `paint.radii`，双向可表达 |
| E3 box shadow | Typed v0.1 无 shadow group；旧 Native/xstyle 路径保留，反向投影逐项报告 `style.box_shadows` |
| E4 opacity | Native ↔ Typed `paint.opacity`，双向可表达 |
| E5 backdrop/material | Typed v0.1 无 material group；旧路径保留，反向投影报告 `style.backdrop` |
| E6 gradient | Typed v0.1 无 gradient group；旧路径保留，反向投影报告 `style.background_gradient` |
| E7 transition/motion | Typed v0.1 无 motion group；旧路径保留，反向投影报告 `style.transitions` |
| E8 transform | Typed v0.1 无 transform group；旧路径保留，反向投影报告 `style.transform` |
| E9 border/outline/decoration | uniform border 与 Typed `paint.border` 互映；单侧 border、outline、decoration 保留旧路径并逐项报告损失 |
| E10 hit-slop | Typed v0.1 无 hit-slop group；反向投影报告 `style.hit_slop`，不改变布局或绘制 |
| E14 text edit | UIKey、focus、TextEdit/IME permission、commit/cancel binding 和编辑事件已接入 |
| E15 line-height / vertical align | px line-height 与 top/center/bottom vertical align 可映射；Multiple line-height 报告语义变化，baseline vertical align 结构化拒绝 |
| P0-A Grid / Position | Typed candidate 字段进入真实 Native layout，仍为 `host_accepted=false` |
| P0-A Image / Overlay | Typed candidate 字段进入 Native image/overlay；反向投影必须提供 exact resource/UiKey 上下文，否则报告损失，仍为 `host_accepted=false` |

该矩阵只描述 Rust consumer 对既有能力的当前映射，不把 Native-only loss 当成 Typed acceptance，
也不删除 PSS/xstyle、LegacyEngine 或 ScenePatch 回滚路径。

## Native 映射状态

| 范围 | 状态 | 说明 |
|---|---|---|
| box/flex/paint/typography 基础子集 | implemented / tested | 设置页 fixture 经真实 Native CPU 执行 |
| 事件绑定与准入 | implemented / tested | click/text-commit、owner/session/revision/permission/generation 拒绝；Native NodeId 只能经 UiKey 上下文映射，activation 来源显式区分 |
| 活动宿主后继帧 | implemented / tested | `apply_frame_with_patches` 生成 patch；`HostRenderer::try_apply_ui_frame_at` 将 IR mapping、精确 Surface generation、Native 事务与 reduce-motion 环境合成原子准入；旧 generation 或暂停时 document/view 均不推进 |
| Table/Row/Cell 语义 | implemented / tested | 公共角色、ARIA 和 AccessKit Table/Row/Cell 保留，data-table fixture Native CPU 可执行 |
| TextArea 语义与编辑 | implemented / tested | 保留 MultilineTextInput、换行输入、行内 Home/End、上下移动与多行编辑缓冲 |
| Popover/Tooltip 语义 | implemented / tested | Native 语义角色保留；AccessKit 分别投影为 Group/Tooltip，Popover 的宿主平台细节仍为 candidate |
| Selected 状态 | implemented / tested | Semantics、AccessibilityNode、ARIA 与 AccessKit selected 状态贯通 |
| Typed Accessibility | implemented / tested | 完整树与增量 update 均经 HostRenderer Surface generation 门控；前后 document revision、update 顺序、removed/changed UiKey 映射均校验 |
| Typed AccessKit actions | implemented / tested | 入口先校验 document revision 与 Surface generation；backend target 必须匹配 UiKey；Focus/Click 分别要求 Focus permission 与 accessibility-action binding；Focus 切换所需 binding 在平台动作前预准入，回调只暴露 UiKey |
| Typed semantic activation | implemented / tested | `typed_activate` 要求当前 UiKey、owner/session/revision、Surface generation 与 Activate binding/permission，回调只暴露 UiKey |
| Typed input targets | implemented / tested | HostRenderer Focus/Key/KeyActivation/TextInput/IME/Deactivate 使用同一 UiKey 上下文授权；编辑按键和失焦将发生的 text-commit/ime-cancel binding 在 Native 状态变化前预准入，Activation 回调只暴露 UiKey；`typed_focused_node` 返回 UiKey；键输入匹配当前 Native focus，TextInput/IME 必须匹配当前编辑节点，键盘激活另需 activate binding；错误 target、permission 和 revision 均拒绝 |
| Typed IME commit | implemented / tested | `typed_ime_commit` 要求 Ime + TextEdit permission 与当前编辑节点，合法 `ime-commit` binding 在文字插入前准入并返回 action |
| Typed edit events | implemented / tested | 有序 drain E14 Changed/Commit/CompositionCancelled；owner、event revision、UiKey 与 text-commit/ime-cancel permission/binding 均校验；批次授权失败不丢事件 |
| Typed pointer routing | implemented / tested | hit query 与 Pointer 路由使用同一 overlay/圆角/hit-slop 目标；owner/revision/Surface generation/Pointer permission 先准入；只有可能聚焦的 Pointer Down 会预准入命中链 Focus bindings，Move 不要求多余 Focus 权限；Move 的 Hover binding 已覆盖；回调只暴露 UiKey；Down→Move 后继帧后 Up 仍保持身份并准入 click binding |
| Typed geometry queries | implemented / tested | bounds/scroll/table metrics 直接消费 UiKey，并校验 document revision、Surface generation 和 active key；table/cell 测量结果也只含 UiKey |
| Typed scroll event | implemented / tested | `typed_scroll_event` 校验 Surface、revision、UiKey、有限非零 delta 与 Scroll binding/permission；不改 Native offset，视觉更新仍由 Framework 的版本化场景帧提交 |
| host/document revision sync | implemented / tested | focus/pointer/geometry/scroll/edit/overlay 查询与准入均要求 Native view 当前 revision 等于 document；分叉时返回结构化 HostRejected，不把已知失配伪装成空焦点或无命中 |
| 资源生命周期 | implemented / tested | Prepare 仅是 pending、不替换旧 Committed；Commit 原子提升；Release 只释放 active；上传与删除分别受 `insert_ui_image`/`remove_ui_image` 状态校验；含代数复用拒绝、节点移除后 Release 与实际解码/绘制 |
| Environment | implemented / tested | 首帧和后继帧的 reduce-motion 均应用到 Native host；appearance/density/text-scale 保留为 Framework 环境语义 |
| Grid、Position、Image、Overlay | candidate | 可映射到现有候选字段，`host_accepted = false`；Grid columns/rows/gap 与 Position/Overlay/Image 均已有 Native 执行测试 |
| flex wrap/shrink/alignment | implemented / tested | wrap/wrap-reverse、shrink、baseline、space-around/evenly 映射到 Blitz CSS；不新增 PSS/xstyle 键 |
| 文字 align/direction | implemented / tested | start/center/end/justify 与 ltr/rtl 映射到 Blitz CSS；auto 保持宿主默认方向 |
| baseline vertical align | structured reject | 当前 Native 文字垂直布局入口没有 baseline 模式 |
| element attributes | structured reject | v0.1 Native profile 无通用属性袋 |

拒绝使用字段路径、property、support level、issue kind 和 detail。candidate 映射不会被改写成
`required` 或已宿主验收能力。

Overlay 虽仍为 candidate，其查询和 dismiss-request 已有 Typed 出口：
`TypedUiOverlayInfo`/`TypedUiOverlayEvent` 只包含 `UiKey`，并校验 owner、document revision、
Dismiss binding 与 permission；整批授权成功前不出队，拒绝后可用修正上下文重试。不会因为
identity 已 Typed 而把 overlay 能力提升为 accepted。

## Fixtures

`tests/fixtures/ui-ir-v0/` 与 appfw `codex/ir-v0@1e55d96` 的三份 fixture 字节一致，并在测试中
固定 FNV-1a 摘要；三份 fixture 同时通过 JSON→Rust→JSON→Rust 结构 round-trip。当前覆盖：

- `settings-page.ir.json`：Schema 解析、Native 映射、真实 SceneHostView、语义、几何和 CPU 渲染；
- `data-table.ir.json`：Schema 解析、Table/Row/Cell 语义、真实 SceneHostView 与 CPU 渲染；
- `dialog-overlay.ir.json`：Schema 解析、candidate overlay/position、真实 SceneHostView overlay 查询与 CPU 渲染。

同目录的 `typed-properties-v0.json` 来自 appfw 的
`docs/ui-ir-v0/schema/typed-properties-v0.json`，以固定 FNV-1a 摘要校验。测试要求 Native profile
逐项覆盖 registry 名称与 support，显式 `host_acceptance=false` 不得被镜像成已接纳，并检查三份
fixture 实际映射出的 `mapped_properties` 与 profile 一致。

## 验收前审计（2026-11-18）

| 要求 | 当前证据 | 结论 |
|---|---|---|
| 严格消费 v0.1 Schema | 未知字段、版本、revision、重复身份、数值边界与 canonical round-trip 测试 | Rust consumer 已覆盖 |
| `UiKey` 防复用 | Native insert claim / remove retire；旧组合拒绝、下一 generation 接受、失败不推进 revision | 已覆盖当前文档生命周期 |
| 事件身份与权限 | owner/identity domain/session/revision、Pointer/Focus/Key/Activate/Scroll/编辑/IME/Overlay/AccessKit Typed 准入 | 已覆盖现有 v0.1 事件 |
| 焦点与编辑一致性 | 当前 focus/编辑目标必须等于授权 UiKey；Move 保持焦点、按下、编辑缓冲与 IME | 已覆盖 Native |
| 资源生命周期 | Prepare/Commit/Release、generation、节点引用、上传/删除与 GPU/CPU image 测试 | 已覆盖当前 image/SVG 路径 |
| 活动宿主同步 | detached mapping、精确 Surface generation、原子 ScenePatch 和 reduce-motion 同提交 | 已覆盖 Rust host |
| Property Registry | 公共 registry 固定摘要；Native profile 逐项一致；candidate 不冒充 accepted | 已覆盖 consumer profile |
| 回滚与兼容 | Native Style/Typed Style 双向受限投影、raw Legacy Scene 显式 identity 反向投影、JUI 2 initial/successor/event 桥及逐项损失已测试；三份 canonical 初始帧和 settings 双帧流经旧 Host CPU reader 接受；旧 raw Scene/ScenePatch、PSS/xstyle、LegacyEngine 测试继续通过 | Rust 兼容桥已覆盖；无条件可逆与真实平台未验收 |
| Web / 真浏览器 | 未执行 | 未验收 |
| 真实硬件 GPU / 平台宿主 | Intel UHD 630 与 NVIDIA RTX 3060 Vulkan 离屏 UI IR 全量测试通过；NVIDIA RTX 3060 硬件窗口 `gpu_window --once` present 成功；Intel 窗口因 Surface/adapter queue family 不兼容失败；Xvfb X11 系统窗口级 XTest 指针/键盘与 IBus/libpinyin 预编辑/提交通过；desktop-accessibility host 在 AT-SPI 总线发布 Settings/Name/Save 树，Save `DoAction` 回环到 JUI 2 click | 部分覆盖；物理桌面/真实 Wayland 合成器输入与硬件窗口完整无障碍未验收；硬件窗口只覆盖 NVIDIA 单帧基础场景 |
| 形式化证明 | 本批未新增 | 未验收 |

本表是 Rust consumer 的审阅清单，不是 issue #3 全链路完成声明；语言侧 Schema/Framework、
Web 后端和正式宿主仍需分别验收。

## 证据状态

| 维度 | 状态 |
|---|---|
| Schema | appfw v0.1 draft；Rust consumer profile 已实现 |
| Rust structural tests | passed |
| Rust Clippy | `cargo clippy --lib --tests` 成功退出；仍保留非阻塞工程 warning，不作为形式化证明 |
| Native CPU same-fixture execution | all three canonical fixtures passed on Linux x86_64 test environment |
| Native GPU same-fixture | settings-page passed with llvmpipe/Vulkan, Intel UHD 630/Vulkan and NVIDIA RTX 3060/Vulkan; hardware max channel delta 1 |
| Web SSR / real browser | not run in this slice |
| Real platform host acceptance | partial: CPU window smoke passed; hardware Vulkan offscreen passed on Intel UHD 630 and NVIDIA RTX 3060; NVIDIA hardware window single-frame present passed; Intel hardware window blocked by Surface/adapter queue-family mismatch; Xvfb X11/XTest pointer, keyboard and IBus/libpinyin preedit/commit passed through winit and the public host; AT-SPI tree publication and Save action round-trip passed; physical desktop input, real Wayland compositor input and hardware-window accessibility not run |
| Formal verification | not performed for UI IR validation/mapping/event admission |
| Cross-backend equivalence | not established |

### 2026-11-18 执行记录

- `native-engine-cpu` + `desktop` 完整 `cargo test` 成功退出：库测试
  `282 passed, 1 ignored`（忽略项是显式性能测量），全部集成测试和 doc-test 通过；
- 同一特性集的 `ui_ir_v0`：`48 passed`；
- 软件 Vulkan（lvp/llvmpipe）+ `gpu-pending` 的 `ui_ir_v0`：`49 passed`；
- 硬件 Vulkan 离屏的 `ui_ir_v0`：Intel UHD 630 `49 passed`（141/153600 pixels differ,
  max channel delta 1）；NVIDIA RTX 3060 `49 passed`（157/153600 pixels differ,
  max channel delta 1）；不代表硬件窗口 Surface/swapchain 呈现；
- `gpu_window --once` 在 NVIDIA RTX 3060/Vulkan 上成功：
  `Bgra8UnormSrgb`、present `Fifo`、frame 1、device generation 1；Intel UHD 630
  能创建 adapter，但其 X11 Surface 报 adapter queue family 不兼容，因此 Intel 硬件窗口不计通过；
- 无 Native 特性的 Legacy 库测试：`180 passed`；`cargo check --no-default-features
  --all-targets` 成功；
- 旧 `jasper-ui-host --headless` reader 在 `/tmp` 副本中指向本分支后分别接受
  settings/data-table/dialog 三份初始 frame：均为 `ack 2`、`accepted 1 / rejected 0`，
  各生成 480×320 PNG 与 dump；data-table 的 Table role 仍按 wire loss 降级，不宣称语义等价；
- settings 的 initial+successor 双帧流返回 `ack 2`、`ack 3`，最终
  `accepted 2 / rejected 0`；后继 frame 的 style/semantics/text/insert/remove 和
  Move→remove+insert 兼容近似都进入最终 dump，Move 状态损失保持显式；
- 在临时旧 host 测试中，`project_jui2_event` 对 hover/pointer/click/focus/key/
  text-commit/ime-cancel/dismiss-request 的输出逐字等于 host 公开 `event_line`；
- 在可用 X11 `:0` / Wayland `wayland-0` 环境中，settings initial+successor 双帧流通过
  真实 `--window --smoke` CPU softbuffer 呈现：`ack 2/3`、`accepted 2 / rejected 0`，
  生成 480×320 截图；该验证不覆盖窗口平台输入/IME、AccessKit 或硬件 GPU；
- settings 初始 frame 经旧 host CPU 命中测试注入 Save 点击后，输出 target 4 的
  `hover in → pointer-down → focus in → pointer-up → click`，帧保持 accepted；
  这是 host 注入动作，不代表真实平台 OS 事件已验收；
- 同一点击动作在真实 `--window --smoke` 路径也产生上述 target 4 事件序列并成功写出
  480×320 呈现截图；仍属于 host 注入，不是系统级鼠标事件验收；
- settings `--interaction` 四帧流在真实窗口中依次接受 focus、` Bob` text input 和
  Enter：`ack 2/3/4/5`、`accepted 4 / rejected 0`，输出 target 3 的
  `focus in → key Enter → text-commit "Alice Bob"`，截图确认最终编辑文本；
- 启用 desktop-accessibility 的临时 host 构建在同一 JUI 2 双帧窗口流上完成
  `accepted 2 / rejected 0`，并通过会话 AT-SPI 总线发布注册树：root 为
  `jasper-ui-host`，其下可见 `Settings`、`Name` 和 `Save` 节点；ToolkitName 为
  `Jasper UI`，Version 为 `0.1.0`；
- 对该树中的 `Save` 节点调用平台
  `org.a11y.atspi.Action.DoAction(index=0)` 返回 `true`，host 随后输出
  `event 4 click` 并保持 `accepted 1 / rejected 0`。这验证了 AT-SPI 注册树发布、
  Settings/Name/Save 语义投影和 Save 动作到 JUI 2 click 的端到端回环，但不证明真实
  OS 鼠标/键盘事件、平台 IME 或硬件窗口完整无障碍；
- 临时验证曾短暂启用 `org.gnome.desktop.interface toolkit-accessibility`，结束后已恢复
  原值 `false`；
- 上述临时 host 副本只为新增 SceneError/Role 变体补了 exhaustive match 分支，
  reader 协议逻辑未改；
  原 host 仓库未修改。其中 headless reader 证据只覆盖 CPU transport；AT-SPI 树与动作来自
  desktop-accessibility window host，不包含真实 OS 输入或硬件窗口无障碍验收；
- 在隔离 Xvfb X11 显示服务器中，`XTest` 先聚焦真实 winit 窗口并发送系统窗口级鼠标
  down/up；Save 控件产生一次场景激活，进程输出 `activated=1`、`retired=true`；
- 同一路径发送普通键盘字符 `a`，winit 将系统 key/text 事件交给 `DesktopInput`，
  外部文本快照从 revision 1 推进到 2，窗口正常关闭并原子退休；
- 在独立 session bus 中启动 IBus + `libpinyin`，X11 XIM 依次产生预编辑
  `年 → 你 → 你和 → 你哈 → 你好`，Space 提交产生 `Text("你好")`；最终快照为
  `External display 中文你好`、revision 2、`retired=true`。该验证经过真实
  winit/IBus/XIM 事件接口，但使用虚拟显示和合成输入，不证明物理键盘、真实 Wayland
  合成器或完整候选窗/光标定位已验收；
- `TextDisplayView::caret` 从同一 Parley 布局返回已准入 UTF-8 偏移的光标矩形；
  `examples/window` 在布局更新后调用 `set_ime_cursor_area`。当前验证证明几何查询与调用
  路径，不证明平台候选窗实际像素位置或候选列表；
- `DesktopInput` 现在在组合态给 `KeyEvent.composing` 置位并抑制 key-derived text，
  `Ime::Commit` 是提交来源；普通无 IME 输入仍使用 winit `event.text`。宿主忽略组合态
  命令键，并只为确有场景 key-down 归属的命令 key-up 转发，避免 IME 消费按下后仍转发的
  key-up 被误判为致命拒绝；
- 独立 `verification/input-identity`：`5 passed`；独立 `verification/read-boundary`：
  `64 passed`。后者补齐了 `ScenePatch::Move` 的独立 oracle，并把随机 patch 属性测试扩展到
  Move 而不删除既有 host 测试；
- `scripts/system-window-smoke.py` 固化本轮可复现系统窗口路径：独立 Xvfb、XTest 点击、
  普通键盘 revision 和隔离 session bus 中的 IBus/libpinyin 提交；脚本使用合成输入，
  不替代物理桌面或真实 Wayland 合成器测试；
- Clippy 成功退出；仓库仍保留已登记的非阻塞历史 warning。

运行前注入了 Latin/CJK、regular/bold/italic 测试字体；这些计数只对应当前 Linux 测试环境。
软件 Vulkan 计数不是硬件证据；硬件 Vulkan 计数来自实际 Intel/NVIDIA adapter，但不代表硬件
窗口呈现、完整平台宿主或 Web/浏览器验收。

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

JUI 2 reader 诊断：

```bash
/home/auth/.cargo/bin/cargo run --locked --no-default-features \
  --example project_jui2 -- [fixture-path]
```

将 stdout 直接作为旧 host 的 `--frames -` 输入即可；host 的锁定依赖和 engine path 属于
另一个兼容 checkout，本次回环在临时源码副本中完成，不修改原 host 工作树。
追加 `--successor` 会输出 default settings 的初始帧与后继帧双帧流。
追加 `--interaction` 会再输出两个版本化 no-op frame，供 host 逐帧注入 focus/
text-input/Enter 交互。
