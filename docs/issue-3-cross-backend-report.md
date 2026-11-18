# Issue #3 Native/Web 双后端验证状态

日期：2026-11-17。本文是审计报告，不把历史测试扩大为双后端黄金用例完成。

## 1. 已有证据（仅据仓库记录）

| 证据 | 范围 | 不能推出 |
|---|---|---|
| appfw `CONTRACT-v1` §3.1–3.2 | 主题映射、JUI 2 握手、qemu 宿主视觉回归、SSR/stdio 回归 | typed UI IR 或同一 IR 双后端执行 |
| 引擎 Native CPU/GPU 测试 | Blitz/Parley/Vello、事务、输入、AccessKit、E2–E15 | Web CSS 等价 |
| appfw `jasper.ui.html` 测试 | Element → HTML、无 JS 表单 | 与 Native 像素/布局一致 |
| theme `tests` | 三主题 Token、能力和降级 | 真实 Windows/macOS 宿主验收 |
| 本轮新增候选 | 图片、浮层、Grid/Table、主题事务测试 | 架构批准或跨后端完成 |

## 2. 双后端矩阵

| 黄金场景 | Native CPU | Native GPU | Web SSR | 真浏览器 | Linux/Mac/Windows 宿主 |
|---|---|---|---|---|---|
| 设置页/三主题 | 历史候选，未在本审计复跑 | 历史部分证据，未复跑 | 部分结构测试 | 未执行 | 未完成 |
| 数据表格/Grid/定位 | 候选能力测试 | 未覆盖 | 未映射新能力 | 未执行 | 未完成 |
| 创建表单/IME/撤销 | 候选能力测试 | 不适用 | 无 JS 表单子集 | 未执行真实 IME | 未完成 |
| Dialog/Menu/Popover | 候选能力测试 | 候选能力未完整 | 仅 role 映射 | 未执行 | 未完成 |
| 图片/SVG/DPI | 候选能力测试 | 候选一致性测试 | 未映射 | 未执行 | 未完成 |
| 滚动/虚拟化 | 候选范围查询测试 | 未覆盖 | 未映射 | 未执行 | 未完成 |

## 3. 本轮未运行项

- 未启动 QEMU/真实窗口；
- 未运行 appfw 全量 JVM、SSR、desktop、visual；
- 未建立 typed UI IR，因此没有“同一 IR 输入”可执行；
- 未执行 Windows/macOS 宿主；
- 未新增或重跑形式化证明。

## 4. 进入下一阶段的最低验收

1. 一份 typed IR fixture 同时编码为 Native frame 与 Web DOM/CSS；
2. 设置页、表单、表格、Dialog/Menu 四个场景；
3. Native CPU 全测、Native GPU 可运行场景；
4. Web SSR HTML 结构/ARIA/表单动作 golden；
5. 真浏览器截图和交互；
6. 差异表包含像素/结构/字体/交互/能力降级，不用单一“通过”概括。

未满足以上条件前，issue #3 的跨后端验证报告状态保持 `not_run`。

## 5. Rust Typed IR consumer 更新（2026-11-18）

Rust 引擎已新增 Typed UI IR v0.1 consumer profile，并以 appfw `codex/ir-v0@1e55d96` 的
`settings-page`、`data-table`、`dialog-overlay` 三份 fixture 做字节固定测试。

- `settings-page` 已通过真实 Native `SceneHostView` 构造、语义/几何查询和 CPU 非空帧输出；
- `dialog-overlay` 已通过真实 Native CPU overlay 查询和帧输出，overlay/position 仍保留为 candidate；
- `data-table` 的 Table/Row/Cell 语义已保留，并通过真实 Native CPU fixture 执行；
- `settings-page` 已通过 llvmpipe/Vulkan 的 Native GPU 执行与 CPU 对照：170/153600 像素不同，最大通道差 1；
- 身份 generation、owner/session/revision/permission 事件准入已有定向测试；活动帧 `Move` 保持焦点、指针按下和 TextArea/IME 编辑状态；Committed image resource 已接入真实 `SceneHostView.insert_image`。
- `HostRenderer::try_apply_ui_frame` 已验证暂停状态下 IR mapping 与 Native host 均保持原 revision，恢复后才共同提交。

这仍不是双后端黄金用例：Web SSR、真实浏览器、真实硬件 GPU、真实平台宿主和完整差异表
均未执行。跨后端状态继续为 `not_run`。
