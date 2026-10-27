# 通用输入法依赖与许可边界

用户 2026-10-24 明确裁决：通用输入法框架本身也须采用宽松许可；不采用 Fcitx 5 / fcitx5-rime 的 LGPL 路线。两项已从依赖锁、分发源包和构建入口移除，原构建未完成验收，历史失败记录仅留在 evidence/ime-deps/rejected-fcitx/，不得发布为产品依赖。

替代方向是 MIT 许可的 Keyman 通用输入法系统中可独立嵌入的 Core；librime 1.17.0（BSD-3-Clause）保留为中文转换引擎候选。这是两个后端方向，不把 Keyman Core 冒充包含所有系统前端、引擎管理、候选服务的完整桌面框架；需要完成真实宿主与统一 UI Rust 接口集成后才能交付。日文、韩文等语言引擎按需装配，不宣称本轮已集成。

当前 sources.lock.json 与 dist/ 固定 librime 原始源包及 Keyman 18.0.255 / commit 02d7ffe6008dac8d2fc21df7f9d5522c6070cc39 的选择性源码快照（Core、common/include、common/cpp、resources/build、根许可/版本）。快照用 git archive 生成，保留原始文件，不是完整 Keyman 应用；重建命令与哈希在锁文件中。Keyman Core 的 meson.build 明确 MIT；尚未完成构建、全部传递依赖及宿主适配核验，不提前列为已构建或已接通。根 Cargo/生产 Rust 没有输入法实现变更。

- 只接受经逐组件核验的宽松许可依赖；根项目的 MIT/BSD/Apache 声明不能替代传递依赖、键盘方案、词库及模型的许可证。
- librime 的 marisa-trie 存在 BSD/LGPL 双许可选项，采用 BSD 路径并保留原始声明；不因此将 LGPL 变为必需依赖。不启用 GPL legacy 插件，也不捆绑 LGPL 前端。
- librime 构建入口 scripts/ime-deps.py 只处理引擎源码、上游测试及本地 target 安装，不安装/启动系统输入法服务，不修改用户桌面输入法设置。
- 尚未完成全体传递依赖清单和所有词库授权核验，不宣称最终发行物已完成许可证审计。

通用 UI 输入契约负责：输入上下文身份/代数、焦点归属、预编辑/提交/取消、候选及光标几何、周边文本与索引单位、语言/引擎切换、密码/敏感字段策略和生命周期。平台已有 IME 通过宿主桥接；应用内引擎与系统 IME 不得重复消费同一事件。jasper-os 通过宿主接口接入，不强制依赖 X11、Wayland、D-Bus、POSIX 或 GPU；现有第三方代码在该宿主的可移植性仍未验证。

第三方 C++ 为明确未证明依赖；自有新增/修改 Rust 必须完成 Verus + Z3 → Proptest → Miri → cargo-fuzz → Kani → Loom 后方可交付。原双键 Kani 未完成义务不因这次依赖替换而解除。后续沿统一接口逐步替换第三方，保留行为对照与回退路径，不扩展到 OS 内核或语言侧。
