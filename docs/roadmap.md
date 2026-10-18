# 路线图（原型 → 可用）

> v0.1 是给宿主集成“意思意思”的原型；API 会改。下面按依赖顺序，不按日期。

## v0.1（当前）

- 场景树 + 增量补丁
- Taffy 布局 + 文本测量接缝
- 显示列表 + UiBatch
- 软件光栅 + Headless 后端
- 指针/触摸/笔事件模型
- 引擎流水线
- 文档/CI/Apache-2.0

**验收**：`taskset -c 0-1 cargo test` 全绿；Headless 示例输出正确。

## v0.2（可集成的最小闭环）

- ✅ 命中测试：布局盒 → 节点；root→target 路径
- ✅ 指针路由：多指/多笔、capture、cancel（v0.1 已落地原型）
- ✅ 语义树 + 焦点管理（宿主中立快照；AccessKit 后续）
- ✅ 参考手势层（tap/long-press/pan/pinch；可替换）
- ✅ 溢出裁剪 + 滚动偏移（wheel 逐级路由与残余 delta 拆分/边界 clamp/fling 惯性；滚动条几何/拖动/绘制已落地，仍无过冲回弹）
- ✅ Canvas 组件（记录式绘制、内容裁剪、`tiny-skia` CPU 渲染；文字排版后续接）
- 手势竞技场/输入法组合（下一步；手掌拒绝与键盘焦点路由已落地）
- 手势边界规范：Rust 原始事件 vs Jasper 手势语义
- 布局一致性测试：固定样例 + 百分比/min-max/gap
- 文本测量基线：CJK/换行/空行；明确近似范围
- ✅ 场景补丁原子批量 + 版本/stale 拒绝（v0.1 已落地）

**验收**：一个真实宿主能收到 pointer down/move/up 并命中节点；有跨平台布局样例。

## v0.3（真实文字与窗口）

- 真实文本栈：cosmic-text 测量已接（可选 `real-text`）；下一步字形光栅/图集缓存、光标几何与默认引擎切换
- winit 平台适配（桌面自带宿主）
- GPU 后端（wgpu/Vello）与软件后端降级
- 图片解码（`image`）与资源缓存
- 帧调度/按需重绘/局部失效（起步）

**验收**：桌面窗口真实显示文字/图片；GPU 失败能降级到软件；帧时间有基线。

## v0.4（内置宿主与 ABI）

- Jasper Native ABI v1（句柄/批量/错误/版本/线程）
- 内置宿主适配：系统 UI 服务 + `UiBatch` 通道 + 软件/GPU 后端
- AccessKit 无障碍桥
- ABI 一致性/模糊测试
- 诊断快照（布局/绘制/资源）

**验收**：外部 Jasper 模块只能经 ABI 驱动 UI；内置宿主不链接 winit/wgpu 也能跑软件路径。

## v1.0（稳定）

- ABI/API 稳定化与版本兼容矩阵
- 平台矩阵：Windows/macOS/Linux（后续移动）
- 性能预算/回归体系
- 完整触摸/笔/手势/IME 边界
- 文档/示例/贡献指南
- 长期维护与依赖升级策略

## 不在本仓库

- Jasper UI Framework/组件/状态/主题/动画/导航（Jasper 侧）
- UI IR/编译器（Jasper 侧）
- Web HTML/CSS/JS/WASM（Jasper Web 后端）
- 浏览器原生 JAS 执行（未来宿主能力）

## 会变的部分（提前说）

- 公开 Rust API 与 `Style` 字段
- `ScenePatch` 语义（事务/版本）
- `UiBatch` 操作码/线格式
- 文本测量实现（近似 → Parley）
- ABI 结构/句柄规则
- 后端 trait 的方法

原型阶段以“能跑通、可验证、可替换”为准，不承诺兼容。
