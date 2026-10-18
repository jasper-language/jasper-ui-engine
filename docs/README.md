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
| [design.md](design.md) | 分层、数据流、模块职责、宿主模式、触摸/笔、布局/绘制/后端边界 |
| [api.md](api.md) | 当前 Rust API 与用法（含限制） |
| [abi.md](abi.md) | Jasper ↔ Rust Native ABI 草案（**未实现**） |
| [integration.md](integration.md) | 内置宿主/自带宿主的最小集成步骤 |
| [dependencies.md](dependencies.md) | 依赖选型、许可证、锁定策略 |
| [roadmap.md](roadmap.md) | v0.1 → v1 的阶段、验收与“会变”的部分 |
| [status.md](status.md) | 已实现/未实现/未验证的逐项状态 |
| [size.md](size.md) | release 体积基线（rlib / 示例 / strip） |

## 重要声明

- 这是 **v0.1 原型**，用于宿主/图形界面集成实验；**API 会改，不是稳定 ABI**。
- Native 引擎只负责底层执行；Jasper UI Framework、UI IR、Jasper ABI、Web 后端不在本仓库。
- 未实现的能力必须显式报错或标注，不用空实现/固定结果冒充完成。
