# Claude 复杂证明遗留清单

本清单用于第 6 项收尾时移交复杂证明，不表示任何条目已完成。功能闭环优先；这些义务在相关实现、后端或宿主证据变化后再复验。不得通过缩输入域、替换真实容器、移除展开断言、跳过析构、超时即通过或有限样例冒充实现级证明。

2026-11-04 起 Kani 不再是日常要求，改到正式版发版时验证，见文末「发版清单」。下面各条中以 Kani 为内容的部分现在不跑，也不阻塞验收；义务本身没有解除。

每项完成后，必须在对应 evidence 中记录：实际生产源码及哈希、规格/范围、固定工具版本、完整配置、全部阶段结果、替代证据边界和仍未覆盖事项。随后重新运行 `scripts/audit-closeout.py --require-ready`。

### CP-01

**真实捕获存储任意双键 Kani 义务。** 保留任意 `u64` 键/查询、真实容器构造、重复键与正常析构，解决当前双键符号执行的容量/后端问题。单键 harness 只能作诊断，不能解除这项义务。已移到发版清单，记在 `scripts/verify-ui.py` 报告的 `release_checklist`。

### CP-02

**真实 `BTreeMap` 的 `source_subtree_contains` 与整帧实现级证明。** 早先把 Bitwuzla 运行记为 `unwind=5/6/8` 展开不足，已更正（2026-11-03）：虚拟机中没有 bitwuzla 可执行文件，那些结果是求解器 `ERROR`，并未否定任何展开界。按循环设界（`--unwindset`，展开断言保留）后全域公式降到 190827 步/3376 个 VCC，但 CaDiCaL 在 26G 上限内于数组后处理阶段内存不足，仍无结论，见 `evidence/native-subtree-unwindset-capability/report.json`。真实 Bitwuzla 0.8.2 首次运行（2026-11-03，获批后在构建虚拟机内固定版本编译）：同一公式约 41–48 分钟返回 sat。此前据此推断“至少一个检查可违反”，已撤回（2026-11-03）：这个 sat 落在 Kani 的 `reachability_check` 探针上（表达式恒为 FALSE，只报告可达性），不是违反，也不是反例。排除探针、只查真实属性（1436/1503 个属性，3174 个 VCC）后，6300 秒超时，求解器无任何输出，所以真实属性既未通过也未被违反，展开界充分性仍未确认，见 `evidence/native-subtree-bitwuzla-capability/report.json`。只选 `real_map_case` 的相等断言一个属性（切片后 30 个 VCC）也在 45 分钟限时内没有结果，求解器零输出（当时构建机与六步链同跑，负载 3–6），未通过也未违反。直接调用 cbmc 时，其 `VERIFICATION FAILED` 须先核对属性类别。不得用较小展开、固定树或替代 Map 通过。还需闭合 `apply_update → apply_patches` 的完整整帧性质。上述 Kani / Bitwuzla 路径已移到发版清单，现在不跑；不依赖 Kani 的部分（如整帧性质的 Verus 证明）继续推进。

### CP-03

**NativeScene 候选构造、状态恢复与整帧有序证明。** 已有提交门和集成事务证据，但来源补丁应用、Blitz 候选构造/替换、焦点/滚动/指针/帧缓存恢复及第三方 panic 回滚仍未形成完整六阶段实现级证明。

已闭合的子性质（2026-11-03，UI-SCENE-FRAME-ORDER-001）：`order_scene_frame` 的阶段顺序。对任意阶段闭包，构造阶段的前置条件只能由精确后继上成功的来源满足，结果绑定到产生它的阶段输出；反例被拒，见 `evidence/scene-frame-order-chain/report.json`。阶段内 panic 由 Proptest/Miri 对真实内核、由集成测试对真实 `NativeScene` 与真实 Blitz 候选检查，这是测试证据，不是证明，且 panic 是在构造阶段外层注入的，不是 Blitz 内部抛出的。同日闭合 UI-SCENE-BATCH-STAGE-001：`stage_scene_batch` 的折叠。对任意状态、补丁、错误类型与任意闭包，`Ok` 是从独立值开始、每个补丁按序恰好成功应用一次的组合，`Err` 是成功前缀之后第一个失败的错误；四个反例被拒，见 `evidence/scene-batch-stage-chain/report.json`。真实 `Scene::apply_batch_at` 由独立树模型对照测试，这是测试证据，不是证明，所以 UI-TXN-ATOMIC-001 仍是 not_verified。

2026-11-04 闭合 UI-SCENE-INTERACTION-RESTORE-001：`restore_candidate_interaction` 的重放。对写入遵守 trait 契约的任意候选，成功时写入序列恰为视口、仍被准入的按下与焦点、悬停、一次解析、采纳指针与帧缓存，`None` 只出现在写入失败之后；反例被拒，见 `evidence/scene-interaction-restore-chain/report.json`。`NativeScene` 的 trait 实现由真实文档测试和九个手动变异检查，这是测试证据，不是证明；Blitz 对这些写入的处理也不在证明内。

2026-11-04 闭合 UI-SCENE-NODE-REMOVE-001：`remove_node` 在真实 `Node` 声明上的语义（其余字段类型在证明中是内核读不到的值）。对任意树与任意身份：返回值恰为该身份是否是真后代；未命中时树结构相同；命中时移除首个匹配的直接子节点，否则只递归进首个含该身份的子树，其他节点结构相同。反例被拒，见 `evidence/scene-node-remove-chain/report.json`。Kani 只跑完根加一个子节点的形状；嵌套形状 25 分钟超时、无判定，这部分只有 Verus 与测试证据。

2026-11-04 闭合 UI-SCENE-NODE-FIND-001：`find_node` 与 `find_node_mut` 在真实 `Node` 声明上的语义（其余字段类型在证明中是内核读不到的值）。对任意树与任意身份：未命中当且仅当节点本身和所有后代都没有该身份；只读命中返回前序首个匹配；可变命中借出同一节点，借用结束时的树恰为旧树把该节点换成借用留下的值，路径上的节点其余字段与子节点数不变，其他子节点不变；可变未命中不改树。反例被拒，见 `evidence/scene-node-find-chain/report.json`。

2026-11-04 闭合 UI-SCENE-PATCH-001：`apply_tree_patch` 在真实 `Node`、`NodeKind`、`ScenePatch`、`SceneError` 声明上的七个分支（`Canvas`、`Style`、`Semantics`、`Point` 在证明中是内核读不到的值）。对任意树与任意补丁，未命中、种类拒绝与根拒绝报对应错误且不写；命中只改补丁所指的字段，`Insert` 按钳制后的下标放置，`Remove` 移除首个匹配。反例被拒，见 `evidence/scene-patch-chain/report.json`。Kani 只查了三个分支的最小形状，`Insert` 分支每种形状都超时、无判定。

2026-11-04 闭合 UI-SCENE-UNIQUE-IDS-001：`collect_unique_ids` 与 `check_insert_identities` 在真实 `Node`、`SceneError` 声明上的判定（其余字段类型在证明中是内核读不到的值）。对任意树和遵守 `IdentitySet` 契约的任意集合：相对前序身份序列（插入检查是树后接子树），`Ok` 只在无重复且都不在起始集合时出现，集合恰为起始加序列；`DuplicateNode(d)` 指名首个重复位置，其前缀无重复，集合恰为起始加该前缀；不出现其他错误。反例被拒，见 `evidence/scene-unique-ids-chain/report.json`。真实 `BTreeSet` 满足契约只经测试，Kani 上真实集合即使单节点也没有判定。

2026-11-04 闭合 UI-SCENE-BATCH-COMMIT-001：`commit_scene_batch` 在真实 `Scene`、`Node`、`SceneError` 声明上的提交（其余字段类型在证明中是内核读不到的值）。对任意场景与只拿到共享引用的任意检查、暂存闭包：陈旧、耗尽、检查失败、暂存失败都报各自的错误并让场景保持原值；空批报检查的结果并保持原值；暂存只能在检查成功后调用；成功时场景恰为暂存的树加旧版本加一，与暂存值带的版本无关。反例被拒，见 `evidence/scene-batch-commit-chain/report.json`。生产传入的闭包（`Scene::validate`、`Scene::clone`、对 `apply_detached` 的折叠）不在此性质内，`Scene::clone` 没有证明。

仍未覆盖：`apply_one` 在 `Insert` 前的身份检查、`Scene` 的克隆与身份校验、`apply_batch_at` 中版本赋值与整体替换两条语句的实现级规格、候选构造本身、`NativeScene` 的 `RestoreCandidate` 实现满足契约（含写入不改变准入判定）、`*self = candidate` 析构旧视图时的 panic、活动文档读取路径上第三方内部可变性的影响。

### CP-04

**浮点、布局、颜色混合及一般原生化变更规格。** 现有内核覆盖若干位模式与标量准入，不能外推为浮点渲染几何、完整布局或任意场景变更已证明。先界定可验证性质与第三方信任边界，再补真实实现映射。

### CP-05

**DesktopInput/Instant 状态的 Kani 时钟边界。** 指针状态路径已闭合（2026-11-03，UI-DESKTOP-POINTER-001 / UI-DESKTOP-POINTER-STATE-001）：`DesktopPointerState` 不含时钟，转换显式接收任意 `u64` 时间戳，按下集合改为严格升序 `Vec`，Verus 状态路径与 Kani 真实状态路径（按下/修饰键/拖动/抬起，任意按钮、坐标、时间戳）均通过。没有假时钟，也没有替换状态。仍未覆盖：`Instant::now`/`elapsed` 本身（标准库 TCB，只在 `DesktopInput::new` 与 `now_millis` 读取）、`now_millis` 组合的单调性、winit 事件到状态转换的映射（`translate`/`cursor_moved`/`mouse_input`），以及 `DesktopInput::ime` 的 Kani（Verus 对全部状态的不变性证明保留）。

### CP-06

**宿主 GPU Surface、device-loss、vsync 与能力矩阵。** Xvfb/llvmpipe 的软件 X11 和 TextureView 证据不等于硬件 Surface、合成器、垂直同步、休眠恢复或设备丢失验收。第三方 wgpu/Vello TCB 仍需明确登记。

### CP-07

**活外部 IME、无障碍服务与组合生命周期。** 焦点/绑定/预编辑/取消/AccessKit 转换和注入服务集成已通过；仍缺活编辑器/IME 服务、平台无障碍桥及迟到事件、重绑定、外部取消回执、宿主捕获组合的形式化证明。

### CP-08

**`full_scene`/窗口适配器实现级组合与跨宿主性能门。** 当前 `full_scene` 已覆盖声明、更新、呈现、焦点/滚动/无障碍、注入外部文本、资源拒绝和退休，窗口也已通过真实 X11 点击与重设；仍需把该编排、物理到逻辑坐标转换、失焦恢复和事件循环组合形式化。现有 153 µs 中位/177 µs P95 只是同 VM 回归参照，最终 release host 的优化门仍待建立。

## 复验顺序

1. 先处理与第 1–5 项功能完成直接相关的 CP-02（不依赖 Kani 的部分）、CP-03、CP-05、CP-06、CP-07、CP-08。
2. CP-01 按原双键义务在发版时恢复；功能推进不能把该旧缺口误报为已解除。
3. CP-04 在性质边界稳定后补实现映射；为避免范围漂移，每次只闭合可陈述、可证伪的子性质。
4. 所有条目完成后，运行各切片完整有序链，再做 `verify-ui.py` 全链和最终收尾门。

本清单故意不提供“快速通过”命令。证明失败、超时、工具不支持与未运行必须分别保留。

## 发版清单

正式版发版时验证，现在不跑：

- 全部现有 Kani harness（`verify-ui.py --kani-full` 或 `--kani-harness`）。旧证据绑定各自提交，不算发版时通过。
- CP-01：真实捕获存储任意双键义务。
- CP-02：真实 `BTreeMap` 的 Kani / Bitwuzla 路径，包括真实属性求解和错误目标对照。
- 其他条目里以 Kani 为内容的部分，如 CP-05 的时钟边界。
