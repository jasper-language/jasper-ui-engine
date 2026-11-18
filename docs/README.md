# 技术文档索引

本目录是 `jasper-ui-engine` 的技术文档。代码是唯一事实来源；文档描述**当前 v0.1 原型**与**后续计划**，两者分开标注。

## 阅读顺序

| 角色 | 建议顺序 |
|---|---|
| 宿主集成（内置/自带） | [design.md](design.md) → [api.md](api.md) → [integration.md](integration.md) |
| Jasper ABI/运行时 | [design.md](design.md) → [abi.md](abi.md) → [roadmap.md](roadmap.md) |
| 贡献者 | [design.md](design.md) → [dependencies.md](dependencies.md) → [roadmap.md](roadmap.md) |
| 只想知道现状 | [status.md](status.md) |

## 文档清单

| 文档 | 内容 |
|---|---|
| [closeout.md](closeout.md) | 第 6 项全量收尾、审计、基础回归与最终交付门 |
| [claude-closeout-proof-backlog.md](claude-closeout-proof-backlog.md) | Claude 复杂证明遗留与禁止降级条件 |
| [design.md](design.md) | 分层、数据流、模块职责、宿主模式、触摸/笔、布局/绘制/后端边界 |
| [api.md](api.md) | 当前 Rust API 与用法（含限制） |
| [appfw-style-e2-e10.md](appfw-style-e2-e10.md) | 扩展样式 E2/E3/E4/E9/E10：字段、JUI 2 `xstyle` 键、能力声明、实现与对齐说明 |
| [appfw-motion-e7-e8.md](appfw-motion-e7-e8.md) | E7 动画/过渡执行器与 E8 二维变换：字段、`xstyle` 键、能力名、执行语义、对齐说明 |
| [appfw-deferred-verification.md](appfw-deferred-verification.md) | 引擎能力补齐（E1–E13、E7/E8）暂缓的验证项登记 |
| [abi.md](abi.md) | Jasper ↔ Rust Native ABI 草案（**未实现**） |
| [integration.md](integration.md) | 内置宿主/自带宿主的最小集成步骤 |
| [dependencies.md](dependencies.md) | 依赖选型、许可证、锁定策略 |
| [roadmap.md](roadmap.md) | v0.1 → v1 的阶段、验收与“会变”的部分 |
| [status.md](status.md) | 已实现/未实现/未验证的逐项状态 |
| [verification.md](verification.md) | 数学证明硬门槛、性质登记、执行与信任边界 |
| [batch-boundaries.md](batch-boundaries.md) | 语义更新/绘制/传输及内置/自带宿主边界 |
| [issue-3-baseline-audit.md](issue-3-baseline-audit.md) | Issue #3 基线审计：现有 PSS/JUI 2/ScenePatch/Web 映射的可复用、冲突与缺失 |
| [issue-3-decisions-approved.md](issue-3-decisions-approved.md) | 用户正式裁决 1A–6A、强制约束和后续状态 |
| [issue-3-semantic-scope.md](issue-3-semantic-scope.md) | HTML-like/CSS-like 元素、事件、布局和样式的建议兼容范围 |
| [issue-3-signal-ui-ir-contract.md](issue-3-signal-ui-ir-contract.md) | State/Derived → typed UI IR → Native/Web 双后端契约建议 |
| [issue-3-migration-plan.md](issue-3-migration-plan.md) | 渐进迁移、回滚、风险与待裁决项 |
| [issue-3-cross-backend-report.md](issue-3-cross-backend-report.md) | Native/Web 双后端验证矩阵与尚未运行项 |
| [ui-ir-rust.md](ui-ir-rust.md) | Typed UI IR v0.1 Rust 消费、身份/事件准入、Native fixture 与证据状态 |
| [host-rendering.md](host-rendering.md) | Surface 代数、DPI/尺寸、暂停/恢复/退休、GPU TextureView/present 与能力矩阵 |
| [platform-hosts.md](platform-hosts.md) | 跨平台宿主责任、Linux/Windows/macOS/移动端与 jasper-os 边界 |
| [android.md](android.md) | Android NativeActivity、GPU Surface、触摸与前后台生命周期 |
| [render-tcb.md](render-tcb.md) | 渲染路径第三方信任边界登记、设备丢失现状与能力矩阵的绑定 |
| [size.md](size.md) | release 体积基线（rlib / 示例 / strip） |

## 重要声明

- 这是 **v0.1 原型**，用于宿主/图形界面集成实验；**API 会改，不是稳定 ABI**。
- Native 引擎只负责底层执行；Jasper UI Framework、公共 UI IR 语义权威、Jasper ABI、Web
  后端不在本仓库。`ui_ir` 是 v0.1 Schema 的 Rust consumer profile，不拥有公共语义。
- 未实现的能力必须显式报错或标注，不用空实现/固定结果冒充完成。

- [UI 宿主资源槽准入](resource-admission.md)：单槽代际生命周期与认证/存储边界。
