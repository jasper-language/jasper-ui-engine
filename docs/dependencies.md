# 依赖与许可证策略

> 原则：**先验证，再锁定**。不以“最新版本”直接作为正式依赖；每次引入都要记录版本、许可证、平台支持、编译/运行验证结果。

## 当前依赖

| 依赖 | 版本 | 许可证 | 用途 | 状态 |
|---|---|---|---|---|
| `taffy` | 0.15.0 | MIT | Flex/Grid/Block 布局执行器 | 已锁定；测试通过 |
| `tiny-skia` | 0.12.0 | BSD-3-Clause | Canvas 组件的 CPU 光栅化（可选特性 `canvas-tiny-skia`，默认开启） | 已锁定；76 项库测试通过 |
| `cosmic-text` | 0.19.0 | MIT OR Apache-2.0 | 真实 shaping、字体回退、CJK/emoji/RTL、断行（可选特性 `real-text`） | 已锁定；`--features real-text` 测试通过 |
| `winit` | 0.30.13 | Apache-2.0 | 桌面窗口与原始输入事件（可选特性 `desktop`） | 已锁定；`--features desktop` 输入映射测试通过 |
| `wgpu` | 30.0.1 | MIT OR Apache-2.0 | GPU 实例与适配器枚举（可选特性 `gpu`） | 已锁定；`--features gpu` 测试通过 |
| `pollster` | 0.4.0 | MIT OR Apache-2.0 | 在同步宿主里等待 wgpu 的异步适配器枚举 | 已锁定；随 `gpu` 特性 |
| `softbuffer` | 0.4.8 | MIT OR Apache-2.0 | 桌面软件帧缓冲呈现（可选特性 `desktop`） | 已锁定；`examples/window -- --once` 实跑通过 |

> `tiny-skia` 是纯 Rust 的 Skia 子集，不引入 C++/系统图形库。BSD-3-Clause 与 Apache-2.0 兼容；完整声明见仓库根目录 `THIRD_PARTY_LICENSES.md`。
> `cosmic-text` 为可选特性；默认构建仍使用 `SimpleTextEngine` 的近似测量，开启 `real-text` 后由系统字体与真实 shaping 接管。

`Cargo.lock` 记录完整传递依赖；禁止手改锁文件绕过审计。

## 计划引入（未锁定）

| 依赖 | 计划用途 | 许可证（以实际 crate 为准） | 引入前检查 |
|---|---|---|---|
| `winit` | 桌面窗口/事件循环 | Apache-2.0 / MIT | 平台矩阵、线程模型、无头/内置宿主不需要 |
| `wgpu` | GPU API 抽象 | Apache-2.0 / MIT | 后端能力、device lost、Surface |
| `vello` / `vello_cpu` | GPU/CPU 2D 渲染候选 | 以 crate 为准（通常 Apache-2.0/MIT） | API 成熟度、功能兼容表、回退 |
| `parley` | 富文本排版/测量 | 以 crate 为准 | 字体回退、CJK/emoji/RTL、线程 |
| `accesskit` | 原生无障碍桥 | MIT / Apache-2.0 | 平台支持、语义树 |
| `image` | 位图解码 | MIT / Apache-2.0 | 编码器范围、内存上限 |
| `resvg` / `usvg` | 静态 SVG | **MPL-2.0** | 与 Apache-2.0 兼容评审、NOTICE、文件级 copyleft |

> `resvg/usvg` 的 MPL-2.0 不是 Apache-2.0，但通常可与 Apache-2.0 项目组合；引入前必须做正式许可证评审并保留 NOTICE。不要在未评审时直接打包发布。

## 明确不引入

- Golang 实现模块（项目约束只使用 Rust + Jasper）。
- 完整浏览器/JS 引擎作为 Native 依赖。
- 运行期反射/脚本引擎作为 UI 核心。
- 未验证的 GPU/窗口库直接进核心 trait。

## 许可证规则

- 项目本身：Apache-2.0。
- 新增依赖必须与 Apache-2.0 兼容；GPL/AGPL/SSPL/BSL 不进入。
- MPL-2.0 等弱 copyleft 需单独评审、保留声明、避免修改其文件（或按 MPL 发布修改）。
- 每个依赖记录：名称、版本、来源、许可证、用途、验证命令/结果。

## 版本与审计

- 版本锁定以 `Cargo.lock` 为准；升级走单独 PR/提交并跑全量测试。
- 后续接 `cargo-deny` / `cargo-audit`（许可证/漏洞/重复依赖）。
- 平台验证：Linux x86-64 先行；Windows/macOS 在引入 winit/wgpu 时补。
- 不承诺“跨平台可用”直到对应平台 CI/人工验证通过。

## 依赖变更流程

1. 说明需求与替代方案。
2. 查许可证/平台/维护状态。
3. 在分支锁定版本，编译 + 测试 + 最小集成。
4. 记录到本文件与 `status.md`。
5. 合并后再对外宣称能力。
