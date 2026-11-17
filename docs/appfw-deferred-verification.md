# 引擎能力补齐（E1 / E11 / E12 / E13）暂缓的验证项登记

> 依据：`jasper-appfw/docs/引擎能力补齐规划.md` §5——新增 Rust 代码的完整六步验证链（Verus + Z3 → Proptest → Miri →
> cargo-fuzz → Loom，Kani 发版时）按用户「验证暂不投入」的意思**暂缓**，但不得让现有证明失效；暂缓项逐条登记于此。
> 分支 `appfw/e1-e13`，2026-11-12/13。

## 已重新通过（本机可执行部分）

| 项 | 命令 | 结果 |
|---|---|---|
| `verification/native-activation`（含改写后的 `scene_work_fits`、场景/组件帧准入模型、新角色的 tag 检查） | `cargo test --locked`（proptest + 边界穷举「miri_*」用例以普通测试执行） | 20 通过 |
| `verification/read-boundary`（`assert_role` 覆盖新角色） | `cargo test --locked` | 64 通过 |
| 引擎全量（原生 CPU + AccessKit） | `cargo test --locked --features native-engine-cpu,accesskit-pending`（注入 DejaVu / Noto CJK 字体环境变量） | 349 通过 |
| 引擎默认特性 | `cargo test --locked` | 198 通过 |

`verification/native-projection` 在改动前的 `main`（12f71b2）上即编译失败（`input-identity` 依赖的 `DesktopInput` 字段已变更），与本分支无关，未修。

## 证据失效、待重证（源码哈希已变）——已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994，见文末「Verus 重证」节

下列内核的源码或其依赖类型改动后，`evidence/<kernel>/report.json` 绑定的源码哈希不再匹配，按 `docs/verification.md` 规则其状态
应读作 not_verified，直到在装有 Verus 0.2026.10.18.7d82994 的构建 VM 内重跑 `scripts/verify-version.py` / `scripts/verify-ui.py`。
规格文本已在 `scripts/verify-version.py` 中同步改写（本机无 Verus，**未运行**）：

| 性质 / 内核 | 改动 | 规格改写 |
|---|---|---|
| UI-SCENE-WORK-CAPACITY-001 `scene_work_fits` | 固定 4096 → 参数 `limit`（无上限时 `usize::MAX`） | `result == (prepared + pending + incoming <= limit)`；两个变异体改为针对 `limit` |
| UI-SCENE-UPDATE-ADMISSION-001 `admit_scene_update` | 去掉 `count > 4096` 分支 | 去掉 CapacityExceeded 分支，`0<count<=4096` → `count>0`；新增两种额度错误 `=> false`（由内核外赋值） |
| `scene-update-error-type`（被 SCENE FRAME COMPLETION / FRAME ORDER 等依赖） | 去掉 `CapacityExceeded`，加 `NodeQuotaExceeded{limit,required}` / `TextQuotaExceeded{limit,required}` | 依赖它的 completion / order 契约中 `CapacityExceeded=>false` 改为两种额度错误 `=>false` |
| UI-COMPONENT-UPDATE-ADMISSION-001 `admit_component_frame` 与错误类型 | 去掉 `count > 4096` 与 `CapacityExceeded` | 去掉对应分支，`0<count<=4096` → `count>0` |
| `role-type`（被 UI-ROLE-ACTIVATION-001、native-projection、read-boundary、semantic-role-tag 依赖） | `Role` 增加 `Heading(HeadingLevel)`、`Status`、`Live(LivePoliteness)`、`Alert`、`Form`，类型标记区内新增 `HeadingLevel`、`LivePoliteness` | 规格不变（`is_activatable`/`activates_with_space` 对新角色为 false）；需确认 Verus 对带数据枚举的 `==` 规格与派生实现 |
| UI-ACCESS-ACTIONS-001 `access_actions` | 内核未改；调用方第一个参数从「是 Button」改为「`role_activates_on_click`」 | 规格不变；`docs/verification.md` 的文字描述（「activate 当且仅当为按钮」）应读作「当且仅当按角色可激活」 |

## 新增代码，六步链全部暂缓

| 项 | 位置 | 现有替代证据 |
|---|---|---|
| `SceneQuota`、`scene_usage`、`admit_scene_quota`、`apply_patches_result_within`（额度准入与错误归属） | `native_scene.rs` | `tests/native_update_contract.rs`（节点/文字额度边界、文案、拒绝不消耗版本、10 002 节点恰好额度接受） |
| 无上限下的整帧事务（大帧原子性：源副本 + 分离候选整体换入） | `native_scene.rs` | 同上 + 原有 `frame_stage_panic_or_rejection_leaves_active_view_then_same_frame_commits`；宿主 5000/50000 节点首帧测试 |
| `role_activates_on_click` / `role_activates_on_key` | `scene_events.rs` | `tests/native_role_activation.rs::role_tables`（全角色穷举的一部分） |
| 指针激活需按下与抬起在同一控件内（`source_contains`，按 parents 大小有界） | `scene_events.rs` | `pointer_click_activates_every_activatable_role_and_focuses_it` |
| 指针按下聚焦 / 焦点控件内点击保持焦点 / 外部点击失焦（`nearest_focus_eligible`） | `native_scene.rs` | 同上 + 改写后的 `pointer_and_style_batch_publish_actual_scene_focus_transitions_once` |
| `key_with_activation_from_host` 键盘激活条件（重复、组合输入、修饰键、阻止默认、禁用、隐藏） | `native_scene.rs` | `keyboard_activation_follows_role_keys` |
| `HostRenderer::node_bounds` / `activate` / `key_with_activation` 的代数准入转发 | `render_host.rs` | `host_renderer_forwards_bounds_activation_and_key_activation` |
| 新角色的原生属性（`role`、`aria-level`、`aria-live`）与 AccessKit 映射（Heading level 1 起、live） | `native_scene.rs`、`access_kit.rs` | `new_roles_map_to_accesskit_with_level_and_live`（含参考 AccessKit 消费端接受） |

## 工具缺失（本机）

- Verus：本机未安装（证据记录的路径在构建 VM 内）。上表 Verus 重证已于 2026-11-17 在 JasperNative04 完成，见文末。
- Miri：nightly 未装 `miri` 组件；`miri_*` 边界用例只以普通测试运行。
- cargo-fuzz / Kani：未安装。`verification/native-activation/fuzz/fuzz_targets/scene_capacity.rs` 已随新签名改写（未运行）。
- 平台验收：AccessKit 的 heading level 采用 ARIA 的 1 起计数（AccessKit 文档称该属性 0 起，平台适配器实际按 1 起暴露标题级别），
  需在 AT-SPI / UIA 上实测确认；Live region 用 Region 承载，屏幕阅读器播报未实测。

## E7 动画/过渡执行器、E8 二维变换（分支 `appfw/e7-e8`，2026-11-13）

### 已重新通过（本机可执行部分）

| 项 | 命令 | 结果 |
|---|---|---|
| 引擎默认特性 | `cargo test --locked` | 214 通过 |
| 引擎全量（原生 CPU + AccessKit） | `cargo test --locked --features native-engine-cpu,accesskit-pending`（注入 `JASPER_UI_TEST_FONT`/`_LATIN_FONT`=DejaVuSans、`_BOLD_FONT`=DejaVuSans-Bold、`_ITALIC_FONT`=DejaVuSans-Oblique、`_CJK_FONT`=NotoSansCJK-Regular.ttc） | 395 通过 |
| 引擎全量 + GPU（硬件 wgpu 适配器） | `cargo test --locked --features native-engine-cpu,gpu-pending,accesskit-pending`（同上字体） | 409 通过 |
| `verification/native-activation` | `cargo test --locked` | 20 通过 |
| `verification/read-boundary` | `cargo test --locked` | 64 通过 |
| `scripts/check-blitz-proof-mapping.py` | 本机无 `evidence/*/report.json`，输出与 `appfw/e2-e10` 基线逐字相同（未新增失配） | 同基线 |

### 证据失效、待重证（源码哈希已变）——`NATIVE SCENE TYPE` 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994，见文末「Verus 重证」节

| 性质 / 内核 | 改动 | 规格影响 |
|---|---|---|
| `NATIVE SCENE TYPE`（`native_scene.rs`，被 `verification/native-projection` 抽取；该 crate 在 main 上即编译失败，见上） | `NativeScene` 增加字段 `motion: MotionState` | 规格不涉及新字段；需在 Verus VM 重抽取并重证类型映射 |

其余已验证区段（`SCENE UPDATE ERROR TYPE`、`SCENE WORK CAPACITY`、`SCENE UPDATE ADMISSION`、`SCENE FRAME COMPLETION/ORDER`、`SCENE INTERACTION RESTORE`、
`frame_schedule.rs` 全部内核、`COLOR TYPE/CONSTRUCT`、`SCENE HOST TYPE` 等）逐字未变（脚本比对 `BEGIN/END VERIFIED` 区段）。
`needs_frame` 的返回值改为 `frame_dirty || 有过渡运行`：`FRAME PACE KERNEL` 本身未改，其输入语义扩展为「有未绘变化或动画在运行」，
`docs/verification.md` 中对 `needs_frame` 的文字描述应相应读作此含义。

### 新增代码，六步链全部暂缓

| 项 | 位置 | 现有替代证据 |
|---|---|---|
| 三次贝塞尔求解（牛顿 + 二分）与端点/单调性 | `src/motion.rs::cubic_bezier` | 单元测试 8 条曲线 × 1000 点单调、端点精确、NaN 输入 |
| 弹簧闭式解（三种阻尼）、包络与结束时刻 | `src/motion.rs::Spring` | 8 组参数 × 4 组初值无 NaN、包络约束、结束时刻在容差内、导数一致 |
| 通道启动/续接/取消、速度继承、减少动态规则 | `src/motion.rs::MotionState` | 单元测试 + `tests/native_motion.rs` 逐帧像素 |
| 样式插值与钳制（预乘色、阴影补层、长度单位、变换） | `src/motion.rs::{snapshot, endpoints, write}` | `interpolation_shapes`、像素中间值断言 |
| `transform` / `transition` 的 `xstyle` 解析与规范输出 | `src/style.rs` | 解析/往返/24 种拒绝 |
| 帧事务中的过渡状态携带（候选视图上 `retarget` + `write_presented`，失败退回目标值、不令帧失败） | `native_scene.rs::rebuild_candidate` | 原有整帧事务测试全过 + 删除重插、续接测试 |
| `animate` / `set_reduce_motion` / `animating` 归属与关闭准入 | `native_scene.rs`、`facade.rs`、`render_host.rs` | `tests/native_motion.rs`（未覆盖：非归属方调用——与既有入口同一 `caller != owner` 模式） |
| 合成仿射、逆矩阵（奇异判定）、视图点 ↔ 局部坐标；切角/命中扩展/叠加层在变换下的几何 | `native_scene.rs::{compose, invert, chain_matrix, to_local, to_view, paint_overlays}` | `engine_overlays_and_corner_cuts_follow_the_transform`、静态变换命中、GPU/CPU 一致 |
| 变换投影为 Blitz CSS（恒等为 `none`、极小缩放保持可逆） | `native_scene.rs::extended_style_properties` | 恒等变换逐像素一致、缩放/旋转像素与命中 |

### 未实测 / 限制

- Miri、cargo-fuzz、Kani、Verus 本机均不可用（同上）；`xstyle` 解析器未做模糊测试。
- 未在真实宿主窗口与真实 vsync 下测量动画帧耗时：每个动画帧对整份文档重新解析样式与布局（`document.resolve`），大场景下的帧预算未评估。
- 退出过渡（删除前淡出）与插入即淡入未实现（见 `appfw-motion-e7-e8.md` §6）。

## E5 材质（毛玻璃）、E6 渐变（分支 `appfw/e5-e6`，2026-11-14）

### 已重新通过（本机可执行部分）

| 项 | 命令（`taskset -c 16-17 nice -n 19`，`-j 2`；字体环境变量同 E7 节） | 结果 |
|---|---|---|
| 引擎默认特性 | `cargo test --locked` | 216 通过 |
| 引擎全量（原生 CPU + AccessKit） | `cargo test --locked --features native-engine-cpu,accesskit-pending` | 415 通过，2 忽略（两个 `--ignored` 测量用例） |
| 引擎全量 + GPU（硬件 wgpu 适配器） | `cargo test --locked --features native-engine-cpu,gpu-pending,accesskit-pending` | 430 通过，2 忽略 |
| `verification/native-activation` | `cargo test --locked` | 20 通过 |
| `verification/read-boundary` | `cargo test --locked` | 64 通过 |
| `scripts/check-blitz-proof-mapping.py` | 本机无 `evidence/*/report.json`；输出与 `appfw/e7-e8` 基线除路径外逐字相同 | 同基线 |
| `BEGIN/END VERIFIED` 区段比对（相对 `appfw/e7-e8`） | 脚本逐区段比较 | 仅 `NATIVE SCENE TYPE` 变化（见下） |

### 证据失效、待重证（源码哈希已变）——`NATIVE SCENE TYPE` 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994，见文末「Verus 重证」节

| 性质 / 内核 | 改动 | 规格影响 |
|---|---|---|
| `NATIVE SCENE TYPE`（`native_scene.rs`） | `NativeScene` 增加字段 `material: MaterialState`（预算 + 上一帧报告） | 规格不涉及新字段；需在 Verus VM 重抽取并重证类型映射（同 E7 的 `motion` 字段） |

其余 `BEGIN/END VERIFIED` 区段（`COLOR TYPE/CONSTRUCT`、`GPU READBACK LAYOUT`、场景准入/帧序/完成、`glyph_skew` 等）逐字未变。
`GpuRenderer` 结构（不在验证区段内）新增 `transient` 字段与图像释放，不影响 `gpu_readback_layout`。

### 新增代码，六步链全部暂缓

| 项 | 位置 | 现有替代证据 |
|---|---|---|
| `Gradient` / `Backdrop` 校验与 `xstyle` 解析、规范输出 | `src/style.rs` | `xstyle_gradient_and_backdrop`（往返、18 种拒绝且样式不变） |
| 渐变/材质过渡插值与钳制（色标保序） | `src/motion.rs::{snapshot, write}` | `material_and_gradient_interpolate_only_between_like_shapes` + 像素中间帧 |
| 节点 id ↔ `backdrop-filter` 标签编码（22 位三段，f32 精确） | `material.rs::{tag_css, decode_tag}` | `tags_round_trip_through_blitz_filters`（含 0、2^22±1、u64::MAX） |
| 三遍盒滤波高斯近似（滑动和、转置、边缘取值）与饱和度矩阵 | `material.rs::{box_passes, gaussian_blur, saturate}` | 单元：平坦不变、单调、对称、预乘 ≤ alpha、σ 误差 ≤ 15% |
| 录制帧改写：层深度跟踪、区域计算（3σ 外扩、视口相交、变换行列式）、预算计入、降级 | `material.rs::compose` | `tests/native_material.rs` 像素与报告断言；中性材质逐像素一致 |
| 录制回放（借用命令、平移） | `material.rs::replay` | 中性材质逐像素一致、GPU/CPU 一致 |
| GPU 单帧图像释放顺序 | `gpu_render.rs::paint` | 重复 GPU 帧逐字节一致 |
| 预算/报告宿主入口的归属准入 | `native_scene.rs`、`facade.rs`、`render_host.rs` | 非归属方返回 None/false 的断言 |

### 未实测 / 限制

- Miri、cargo-fuzz、Kani、Verus 本机不可用（同上）；`backdrop` / `background-gradient` 的 `xstyle` 解析器、`compose` 的整数区域运算未做模糊测试或溢出证明
  （区域坐标为 f64 钳制到视口后转 u32，面积按 u64 计）。
- 未在真实宿主窗口与真实 vsync 下测帧耗时；预算数值来自无头测量（`docs/appfw-material-e5-e6.md` §7）。
- 平台视觉验收（与 macOS / Windows 原生毛玻璃的观感对照）未做；Liquid Glass 折射高光不在本步。

## E14 场景内编辑态、E15 文字垂直对齐（分支 `appfw/e14-e15`，2026-11-14）

基于 `appfw/e5-e6` f1f3e0a。说明见 `docs/appfw-text-edit-e14-e15.md`。

### 已重新通过（本机可执行部分）

| 项 | 命令（`taskset -c 16-17 nice -n 19`，`-j 2`；字体环境变量同 E7 节） | 结果 |
|---|---|---|
| 引擎默认特性 | `cargo test --locked` | 216 通过 |
| 引擎全量（原生 CPU + AccessKit） | `cargo test --locked --features native-engine-cpu,accesskit-pending` | 436 通过，2 忽略（新增 21：E15 6、E14 10、编辑缓冲单元 5） |
| 引擎全量 + GPU（硬件 wgpu 适配器） | `cargo test --locked --features native-engine-cpu,gpu-pending,accesskit-pending` | 451 通过，2 忽略 |
| `verification/native-activation` | `cargo test --locked` | 20 通过 |
| `verification/read-boundary` | `cargo test --locked` | 64 通过 |
| `BEGIN/END VERIFIED` 区段比对（相对 `appfw/e5-e6` f1f3e0a） | 脚本逐区段比较 | 仅 `NATIVE SCENE TYPE` 变化（见下） |

### 证据失效、待重证（源码哈希已变）——`NATIVE SCENE TYPE` 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994，见文末「Verus 重证」节

| 性质 / 内核 | 改动 | 规格影响 |
|---|---|---|
| `NATIVE SCENE TYPE`（`native_scene.rs`） | `NativeScene` 增加字段 `edit: scene_edit::EditState`（编辑会话、事件发件箱、拖动、失焦保持显示） | 规格不涉及新字段；需在 Verus VM 重抽取并重证类型映射（同 `motion`、`material`） |

`text_offset_admits` / `text_range_admits`（TEXT OFFSET / RANGE KERNEL）未改，编辑叠加层的几何仍经其准入。

### 新增代码，六步链全部暂缓

| 项 | 位置 | 现有替代证据 |
|---|---|---|
| `TextAlignVertical` / `LineHeight` 校验与 `xstyle` 解析、规范输出 | `src/style.rs` | `capability_and_xstyle_grammar`（往返、8 种拒绝且样式不变） |
| E15 CSS 投影（列/行/块三种覆盖、只对 Text 节点、默认不写） | `native_scene.rs::text_style_properties` | 默认/显式 top 逐像素一致；居中留白相等 ±1px；node_bounds 与命中不变 |
| 编辑缓冲：字素簇/按词删除与移动、选区、预编辑显示偏移 ↔ 缓冲偏移映射、字符边界不变式 | `text_edit.rs::EditBuffer` | 5 个单元测试（含 ZWJ 表情、组合附加符、CJK 边界） |
| 会话生命周期（随焦点开始/失焦提交/帧内静默结束）、跨帧携带与外部更新冲突规则 | `scene_edit.rs::{sync_edit_session, carry_edit_from}` | `enter_and_blur_commit_once…`、`external_updates_win…`、`removal_ends…` |
| 显示文本写入原生文字节点（不写源、不耗版本）、空文本零宽空格占位 | `scene_edit.rs::{write_node_text, write_edit_display}` | `snapshot_identity` 不变断言；空文本光标几何 |
| 键盘编辑、剪贴板钩子、指针定位与拖选（含 E8 逆变换） | `scene_edit.rs::{edit_key, edit_pointer, edit_offset_at}` | `keyboard_editing…`、`pointer_places_caret…` |
| 叠加层（选区、预编辑下划线、光标）的裁剪/变换/透明度 | `scene_edit.rs::edit_overlays` | 光标列、选区色、下划线像素断言 |
| 闪烁相位计算与减少动态 | `scene_edit.rs::caret_blink_from_host` | `caret_blinks_on_the_host_clock…` |
| 宿主入口归属/版本准入 | `scene_edit.rs`、`facade.rs`、`render_host.rs` | 非归属方 None、过期版本 false 断言 |

### 未实测 / 限制

- Miri、cargo-fuzz、Kani、Verus 本机不可用；`EditBuffer` 未做模糊测试（建议：随机操作序列下的字符边界不变式）。
- 未在真实窗口中实测输入法（fcitx/IBus 预编辑、候选窗定位）与系统剪贴板；宿主侧路由尚未改（见说明文档 §3）。
- AccessKit `TextSelection` 与文本动作未接；长文本单行水平滚动、双向文本视觉移动、撤销未做。

### 修复：材质预算跨帧丢失（2026-11-14，宿主接入发现）

`rebuild_candidate` 帧事务以 `from_scene_with_context` 新建候选视图，`material: MaterialState` 取默认值而未从旧视图携带，
导致宿主经 `set_backdrop_budget` 设置的预算在每个已应用帧后回到 `BackdropBudget::DEFAULT`（报告也被清空）。
修复：候选携带 `material`（预算与上一绘制帧报告）。同查减少动态（随 `motion` 克隆携带）与编辑态闪烁（随会话携带）无此问题，补测试。
`BEGIN/END VERIFIED` 区段未变（改动在区段外），无新增待重证项。

| 项 | 命令（同上） | 结果 |
|---|---|---|
| 引擎默认特性 | `cargo test --locked` | 216 通过 |
| 引擎全量（原生 CPU + AccessKit） | `cargo test --locked --features native-engine-cpu,accesskit-pending` | 439 通过，2 忽略（新增 3：`e5_budget_persists_across_applied_frames`、`reduce_motion_persists_across_applied_frames`、`caret_blink_and_reduce_motion_persist_across_applied_frames`） |
| 引擎全量 + GPU | `cargo test --locked --features native-engine-cpu,gpu-pending,accesskit-pending` | 454 通过，2 忽略 |
| `verification/native-activation` / `read-boundary` | `cargo test --locked` | 20 / 64 通过 |

## Verus 重证（2026-11-17，构建 VM JasperNative04，分支 `appfw/verus-recheck`，基于 main ebddcc6）

工具：VM 内 `~/verus/verus`，Verus 0.2026.10.18.7d82994（release，linux_x86_64，工具链 1.99.1），随包 Z3 4.16.0。
入口：`JASPER_UI_VM_RUN=…/vm-run.sh JASPER_VM_PROFILE=native JASPER_UI_VM_TIMEOUT_SECONDS=3600 JASPER_UI_VM_MEMORY=4G
scripts/verify-in-vm.sh bash -c 'python3 scripts/verify-version.py --verus "$HOME/verus/verus" --kernel <名>'`（逐个 kernel）。
范围由脚本确定：相对改动前基线 12f71b2 逐区段比较 `BEGIN/END VERIFIED` 抽取结果，变化的区段为 `role-type`、
`component-update-error-type`、`component-update-admission`、`scene-work-capacity`、`scene-update-error-type`、
`scene-update-admission`、`native-scene-type`；依赖闭包含这些区段的对外内核共 10 个，全部重跑。证据在 `evidence/<kernel>-kernel/`。

| 性质 / 内核 | 结果（已验证义务数；反例被拒数） |
|---|---|
| UI-SCENE-WORK-CAPACITY-001 `scene-work-capacity` | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（1；2/2） |
| UI-SCENE-UPDATE-ADMISSION-001 `scene-update-admission`（含 `scene-update-error-type`、`scene-work-capacity`） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（5；2/2） |
| UI-COMPONENT-UPDATE-ADMISSION-001 `component-update-admission`（含 `component-update-error-type`） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（2；2/2） |
| UI-SCENE-FRAME-COMPLETION-001 `scene-frame-completion`（依赖 `scene-update-error-type`） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（2；2/2） |
| UI-SCENE-FRAME-ORDER-001 `scene-frame-order`（依赖 `scene-update-error-type`） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（3；4/4） |
| UI-ROLE-ACTIVATION-001 `role-activation`（依赖 `role-type`；带数据枚举 `Heading`/`Live` 的 `==` 规格无问题） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（5；2/2） |
| UI-SEMANTIC-ROLE-TAG-001 `semantic-role-tag`（依赖 `role-type`） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（5；2/2） |
| `NATIVE SCENE TYPE` → UI-SCENE-SNAPSHOT-IDENTITY-001 `scene-snapshot-identity` | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（9；2/2）——见下「规格同步」 |
| `NATIVE SCENE TYPE` → UI-SCENE-HOST-CLOSE-001 `scene-host-close` | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（10；2/2） |
| `NATIVE SCENE TYPE` → UI-SCENE-HOST-IDENTITY-001 `scene-host-identity` | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（11；2/2） |
| UI-ACCESS-ACTIONS-001 `access_actions` | 内核与依赖区段逐字未变，现有证据哈希仍匹配，未重跑（文字描述按上表读作「按角色可激活」） |

**规格同步（`scripts/verify-version.py`）**：首次运行时三个 `NATIVE SCENE TYPE` 依赖内核均失败（`not_verified`，rustc
E0433/E0425：`jasper_ui_engine::motion::MotionState`、`MaterialState`、`scene_edit::EditState` 在证明文件中无定义）。这是证明外壳缺少
新字段类型声明，不是代码缺陷：仿照既有 `blitz_dom::NodeId` / `BaseDocument` 做法，在 `native-scene-type` 的 `spec_prelude` 中补三个
`#[verifier::external_body]` 不透明结构（路径与源码一致），规格与契约文本不变。修改后上述 10 个内核用同一 runner
（sha256 20c535cc…）全部重跑通过。其余未重跑的旧证据所记 `runner_sha256` 为旧脚本哈希（脚本此前已多次改动），其内核/依赖/证明哈希仍匹配。

**附带运行（不在本次登记范围，如实记录）**：

| 项 | 结果 |
|---|---|
| UI-NATIVE-COMPONENT-VERSION-001 `native-component-version`（旧证据早于本批改动即已失配） | 已重证 2026-11-17 JasperNative04 Verus 0.2026.10.18.7d82994：proved（26；2/2） |
| `native-host-context`、`text-display-version`、`text-display-close` | **失败**：Verus 报 `core::bool::impl&%0::then_some` 不受支持（`native_view.rs` NATIVE HOST CONTEXT KERNEL 中 `(…).then_some(self.context)`）。该写法由 d3a29f3（UI32，早于 appfw 基线 12f71b2）引入，与本批改动无关；原 proved 证据针对更早源码，此前已失配。判断：证明外壳/写法问题——需在规格侧为 `bool::then_some` 加 `assume_specification`，或把实现改写为 `if … {Some(…)} else {None}`（后者涉及实现，未擅自改）。三份证据现记录为 not_verified。 |
| `verification/native-projection`（VM 内 `cargo test --locked`） | **未能编译**（与上文登记一致，早于本分支）：其依赖 `verification/input-identity` 仍按旧 `DesktopInput{pointer_id,position,pressed,modifiers}` 构造，且抽取出的 `DesktopInput` 引用未声明的 `DesktopPointerState`/`DesktopTouchState`（9 个错误）。该 crate 中 `NATIVE SCENE TYPE` 的映射测试因此无法运行；Verus 侧已由上面三个内核覆盖。修复需改写 input-identity 验证外壳，未做。日志：`evidence/native-projection-recheck/cargo-test.log`。 |
