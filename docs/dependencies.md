# 依赖与许可证策略

> 原则：**先验证，再锁定**。不以“最新版本”直接作为正式依赖；每次引入都要记录版本、许可证、平台支持、编译/运行验证结果。

## 当前依赖

| 依赖 | 版本 | 许可证 | 用途 | 状态 |
|---|---|---|---|---|
| `taffy` | 0.15.0 | MIT | Flex/Block 布局适配；上游 Grid 未映射到本引擎公开样式 | 已锁定；测试通过 |
| `tiny-skia` | 0.12.0 | BSD-3-Clause | Canvas 组件的 CPU 光栅化（可选特性 `canvas-tiny-skia`，默认开启） | 已锁定；76 项库测试通过 |
| `cosmic-text` | 0.19.0 | MIT OR Apache-2.0 | 真实 shaping、字体回退、CJK/emoji/RTL、断行（可选特性 `real-text`） | 已锁定；`--features real-text` 测试通过 |
| `winit` | 0.30.13 | Apache-2.0 | 桌面窗口与原始输入事件（可选特性 `desktop`） | 已锁定；`--features desktop` 输入映射测试通过 |
| `wgpu` | 30.0.1 | MIT OR Apache-2.0 | GPU 实例与适配器枚举（可选特性 `gpu`） | 已锁定；`--features gpu` 测试通过 |
| `pollster` | 0.4.0 | MIT OR Apache-2.0 | 在同步宿主里等待 wgpu 的异步适配器枚举 | 已锁定；随 `gpu` 特性 |
| `softbuffer` | 0.4.8 | MIT OR Apache-2.0 | 桌面软件帧缓冲呈现（可选特性 `desktop`） | 已锁定；`examples/window -- --once` 实跑通过 |

> `tiny-skia` 是纯 Rust 的 Skia 子集，不引入 C++/系统图形库。BSD-3-Clause 与 Apache-2.0 兼容；已有声明见仓库根目录 `THIRD_PARTY_LICENSES.md`。
> `cosmic-text` 为可选特性；默认构建仍使用 `SimpleTextEngine` 的近似测量，开启 `real-text` 后由系统字体与真实 shaping 接管。

`Cargo.lock` 记录完整传递依赖；禁止手改锁文件绕过审计。

## 尚未接入的候选能力（未选定）

| 依赖 | 计划用途 | 许可证（以实际 crate 为准） | 引入前检查 |
|---|---|---|---|
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

现有历史测试表述不代表本次已重跑；以带环境/提交的验收证据为准。字体发现必须支持宿主注入；当前 `real-text` 启用 fontconfig，尚不能作为无系统字体服务的内置宿主实现。


## UI 验证专用依赖（2026-10-23）

- Proptest 1.12.0（Cargo.lock 解析；MIT OR Apache-2.0）：属性生成、shrink、反例持久化；只用于测试。
- Loom 0.7.2（MIT）：验证专用 crate 的调度检查，不进入 UI 运行时。
- libfuzzer-sys 0.4.13（MIT OR Apache-2.0）：单独 fuzz crate；cargo-fuzz 0.13.2 使用 nightly-2026-10-17 及 ASan。
- Kani 0.68.0 / CBMC 6.11.0：构建虚拟机验证工具，不进入 UI 发行依赖。Miri 版本在每次顺序验收日志记录。
- 独立 crate 的 Cargo.lock 分别提交到 `verification/read-boundary/` 和 `fuzz/`；不通过浮动依赖代替验收时的版本记录。传递依赖的完整发行审计仍待完成，不宣称仅列直接依赖即已覆盖全部许可。


## Skia 早期接入实验（2026-10-24，历史记录）

`experiments/skia-cpu-probe/` 固定 skia-safe/skia-bindings 0.153.3，使用默认功能加 textlayout 的上游 CPU 预编译组合；该早期实验当时未改变根 Cargo.toml/Cargo.lock。绑定为 MIT，Skia BSD 许可及附带第三方许可需在正式打包时完整归档。实际 asset URL、GitHub metadata digest、特性图、锁文件、构建日志和 native 库列表见 evidence/skia-cpu-probe。上游 master 0.155.0 未在本次 registry 查询中发布；不跟随 master 版本号。

极简无默认功能组合没有匹配预编译资产，源码回退缺 ninja；该失败保留，不计通过。采用上游已发布 CPU 组合后，构建 VM 安装 FreeType/fontconfig 开发库解决链接，真实 Skia 像素冒烟与 locked 复跑通过。无 GPU/窗口/字体调用不等同未来功能无系统依赖，jasper-os 目标仍待验证。Skia/native FFI 是显式第三方信任边界；实验不属于正式生产接入的六阶段完成证据。

## Scintilla / Lexilla 暂用依赖（2026-10-24）

根据用户最新裁决，编辑后端优先复用 Scintilla，高亮复用 Lexilla，后续逐层用验证后的 Rust 替换。源包、许可、固定版本与构建入口位于 `third_party/editor/`，详见其 README。C++ 内部为显式未证明第三方边界，不伪称数学证明；自有新增/修改 Rust 门槛保持完整六阶段。当前仅引入固定依赖及离线构建/上游测试入口，生产 Rust/Cargo 未变，正式编辑、视图和 IME 适配尚未交付。jasper-os 不默认引入 GTK/Qt/Win32；Canvas 仍最后。

## 输入法方案许可裁决（2026-10-24，覆盖此前 Fcitx 方向）

用户要求通用输入法框架自身也使用宽松许可，撤销 Fcitx 5 / fcitx5-rime 选型。两者的锁定依赖、源包、构建步骤及 LGPL 许可副本已移除；保留的历史失败证据不是现行依赖。当前通用方案改为核验 MIT 许可 Keyman 的独立 Core 与宿主接口，中文引擎保留 BSD-3-Clause librime 候选，不宣称 Core 等于完整系统输入法服务。引擎/词库/传递依赖均须逐项核验，不将顶层宽松许可外推。当前尚无 Rust 输入法交付，详见 third_party/ime/README.md；生产 Rust/Cargo 无变更。


替代源已固定：Keyman 18.0.255 / 02d7ffe6008dac8d2fc21df7f9d5522c6070cc39，Core 的 meson.build 与上游 LICENSE.md 均声明 MIT。选择性原始源码快照、许可与精确重建命令/SHA-256已纳入 third_party/ime，安全提取、文件核对通过。旧 Fcitx/framework/addon 步骤与分发源包均不在现行清单。Keyman 尚未构建和接通；librime 仅保留源候选，通用框架输入上下文/候选服务/宿主适配未完成，不以固定 Core 源码代替完整框架交付。生产 Rust/Cargo 未变，全部传递依赖与数据授权仍待单独核验。


当前新增：根 crate 已增加固定 =0.153.3 的 optional skia-safe +textlayout，
仅由默认关闭的 skia-text-pending feature 启用，本地文本模块自有Rust尚未验收。
Cargo.lock已保留对应可选依赖解析。上述MIT绑定、Skia BSD及原生第三方许可/构建
信任边界继续适用；不宣称无POSIX/系统库依赖或jasper-os可运行。默认构建不启用
此依赖。新数值准入的限定验证不代表原生字体、布局、渲染或全接入已证明。
