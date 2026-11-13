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

## 证据失效、待重证（源码哈希已变）

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

- Verus：未安装（证据记录的路径在构建 VM 内）。上表所有 Verus 重证待 VM。
- Miri：nightly 未装 `miri` 组件；`miri_*` 边界用例只以普通测试运行。
- cargo-fuzz / Kani：未安装。`verification/native-activation/fuzz/fuzz_targets/scene_capacity.rs` 已随新签名改写（未运行）。
- 平台验收：AccessKit 的 heading level 采用 ARIA 的 1 起计数（AccessKit 文档称该属性 0 起，平台适配器实际按 1 起暴露标题级别），
  需在 AT-SPI / UIA 上实测确认；Live region 用 Region 承载，屏幕阅读器播报未实测。
