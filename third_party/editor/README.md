# 暂用编辑与高亮依赖 / Temporary editor and lexer dependencies

> Scope correction: Scintilla/Lexilla archives and evidence are historical.
> Ropey + Helix Core + Tree-sitter is an external editor-core direction, outside
> this UI workstream. Do not integrate or implement that stack here. UI consumes
> the external editor contract; do not extend these native editor experiments.

用户 2026-10-24 授权：先复用 Scintilla 编辑能力和 Lexilla 语法高亮，之后沿接口以经过完整验证的 Rust 实现逐步替换。此目录属于 UI Rust 的原生依赖集成，不接管 Workbench 业务、语言或 OS 内核。

## 固定源与离线构建

- Scintilla 5.6.7：编辑文档、撤销/重做、选区及平台编辑控件实现。
- Lexilla 5.5.4：配套语法着色/折叠 lexer 库，不提供语言服务或输入法引擎。
- `dist/` 保存官网原始源码包；`sources.lock.json` 固定 URL、版本和 SHA-256。保留包内全部原始版权、许可与测试数据，顶层复制各自 License.txt。
- 源码不打补丁。`scripts/editor-deps.py` 先验证归档哈希/路径/许可，再提取至按锁文件哈希隔离的 `target/editor-deps/`；离线即可重复构建，不执行下载脚本。

```sh
python3 scripts/editor-deps.py --prepare-only
# 重型编译在外部构建 VM 内执行（入口脚本由运行环境通过 JASPER_UI_VM_RUN 提供）：
JASPER_UI_VM_RUN=/path/to/vm-entry.sh VM_NICE=15 bash "$JASPER_UI_VM_RUN" jasper-ui-engine python3 scripts/editor-deps.py --jobs 2
```

运行实际 Scintilla 文档核心测试、构建 Lexilla 静态/动态库，再运行 Lexilla 单元及 lexer 对照测试。日志、工具版本、命令、源/库哈希在 `evidence/editor-deps/`。需要 Python 3.12+、make、支持 C++20 的编译器（库本身 C++17；上游 lexer 测试 C++20）。测试只在 Linux 构建 VM 验证，其他宿主能力不能据此推定。

## 当前边界

源码依赖引入与上游构建不等于正式编辑后端接通。当前根 Cargo 默认依赖和生产 Rust 没有变化：没有把 C++ 对象指针、异常、回调或平台消息直接暴露给 Jasper API，也不宣称通过六阶段验证。上游 C++、标准库、lexer 和原生 ABI 均为明确未证明依赖；上游测试不能代替自有 Rust 的 Verus + Z3 → Proptest → Miri → cargo-fuzz → Kani → Loom。

Scintilla 的 Document 核心可无 GUI 测试；完整 Editor/View 的字体测量、绘制、事件、剪贴板、无障碍及 IME 桥接需要真实平台适配。jasper-os 不依赖 GTK、Win32、Cocoa 或 Qt，不能直接选用那些端口；后续适配使用宿主提供的字体/Surface/输入服务。Skia 可作为绘制依赖，不扩展 Canvas 主线。

## 接入与后续替换顺序

1. 固定并验证这两个原始依赖；隔离构建，不让普通 UI 使用者默认携带全部 lexer。
2. 编辑后端边界使用 UTF-8 字节位置，显式处理范围/溢出/失效、文档归属及 UI 线程约束；禁止把字节偏移误当字符或 UTF-16 索引。自有 Rust 接入与旧索引代码补证同步完整验收后才能交付。
3. 接通实际文档编辑、选区、撤销/重做与 Lexilla lexer 实例，再接平台视图、命中/光标几何与 IME 预编辑/提交/取消；原生输入法引擎仍由宿主提供。
4. 通用文本控件消费同一 UI 宿主接口；Scintilla 不替代通用文字 shaping，也不自动解决 RTL、辅助技术或安全密码框语义。不得以编辑器展示测试代替这些能力验收。
5. 逐项以验证后的 Rust 替换文档/索引、编辑状态、布局/视图及高亮适配。每项保留原后端行为对照和可回退选择，最后移除不再使用的 C++ 部件；不做一次性整体重写。

官方参考：[Scintilla 文档](https://www.scintilla.org/ScintillaDoc.html)、[架构](https://www.scintilla.org/Design.html)、[Lexilla 构建与接口](https://www.scintilla.org/LexillaDoc.html)。
