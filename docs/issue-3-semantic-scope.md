# Issue #3 语义兼容范围（建议稿）

本文给出待裁决的 v0 范围。它不是“完整 HTML/CSS 兼容”声明。

## 1. 规范性原则

- **类型化声明为规范表示**；JUI 2/CSS 文本是传输或互操作格式。
- Framework 在进入 IR 前完成主题、状态、语言/密度和样式解析。
- Native/Web 只执行同一已合并 IR，不各自重新解释品牌主题。
- 未支持属性和非法值必须返回结构化问题或确定性回退，不允许静默忽略。
- HTML/CSS 规范用于建立共同词汇，不意味着引入 DOM、JS、浏览器事件循环或完整 CSS。

## 2. Element / 可访问性语义

### 2.1 强制 v0

| 语义 | 最小属性/事件 |
|---|---|
| Container/Region | children、布局、clip、scroll |
| Text | text、direction、font、selection |
| Button | label、disabled、focus、activate |
| Link | label、href resource、focus、activate |
| TextField/TextArea | value、selection、disabled/readonly、focus、text commit/IME、无障碍文字动作 |
| Checkbox/Radio/Switch | checked、disabled、focus、toggle |
| Slider | numeric value/range、step、disabled、change |
| List/ListItem/Table | children/rows/cells、selection、key identity |
| Image | resource id、decorative label/alt、fit/alignment |
| Dialog/Popover/Menu/MenuItem/Tooltip | layer、anchor、modal、dismiss、focus trap/restore、role |
| ScrollView | viewport/content extent、offset、virtualization window |

HTML tag、ARIA role 和 Native role 是 backend mapping，不是三份独立语义。

### 2.2 事件

- pointer down/move/up/cancel/leave、hover、scroll；
- key down/up、focus in/out；
- text commit、IME preedit/commit/cancel、clipboard copy/cut/paste request；
- activate/change/select、dialog dismiss request；
- accessibility action。

事件的规范目标是 typed semantic node + node generation + frame revision。
不得只凭可复用的整数 NodeId 把旧事件投递到新节点。

## 3. Style / Layout 范围

### 3.1 强制 v0

| 组 | 范围 |
|---|---|
| Box | width/height、min/max、padding/margin、border、box-sizing |
| Flex | direction、wrap（若补齐）、grow/shrink、align/justify、gap |
| Grid | explicit tracks、placement/span、gap、auto/implicit tracks |
| Position | static/relative/absolute/fixed/sticky、inset、z-order |
| Paint | color、background、gradient、border radii、shadow、outline、opacity、clip |
| Text | family/fallback、size/weight/style、line height、align、decoration、vertical alignment |
| Image | resource、object-fit/position、density、tint/currentColor |
| Motion | transition、spring/cubic easing、delay、reduce-motion |
| Material | blur、saturation、tint、stroke、fallback |
| Overlay | layer、anchor、avoidance、modal、scrim、focus/dismiss |

### 3.2 Framework 侧解析

- 选择器、层叠、继承、CSS variables、媒体/容器查询和主题切换由 Framework 在生成 IR 前解析。
- 组件只提交语义/state；品牌主题名不进入 Native Engine。
- 允许后续提供 CSS 文本导入/导出工具，但不把任意 CSS 字符串作为必需运行时入口。

### 3.3 明确不支持或后置

- 完整 CSSOM/选择器引擎、`@import`/JS 样式注入、浏览器专有布局；
- 完整表格模型/colspan/rowspan、复杂 fragmentation；
- 任意 Web 字体/第三方资源自动获取；
- Canvas/3D（Canvas 继续独立排期）；
- Web-only API 强行映射到 Native；
- 无界滤镜链和未预算材质。

## 4. 能力与差异等级

每个 typed 属性必须声明：

| 等级 | 含义 |
|---|---|
| `Required` | 双后端必须执行；不满足则 IR frame 被拒 |
| `Optional` | backend hello 声明后执行；否则确定性回退并报告 |
| `HostOwned` | 窗口、IME、剪贴板、系统无障碍等必须由宿主提供 |
| `WebOnly` / `NativeOnly` | 明确扩展，不伪装跨后端 |
| `Unsupported` | 结构化拒绝，不生成“近似成功” |

允许差异必须单独列出：字体光栅、亚像素舍入、系统控件 appearance、平台 IME、
GPU 纹理格式、滚动惯性。差异需有 golden case/阈值，不能只写说明。

## 5. 兼容性测试面

同一 IR 输入至少覆盖：

- 设置页、数据表格、创建表单；
- Popover/Menu/Dialog/Select/Tooltip；
- Flex/Grid/定位/滚动/虚拟列表；
- 三主题 × 浅/深 × 标准密度；
- 中英混排 + RTL、输入/编辑/选区/撤销；
- 缺失资源、无效属性、能力不足和预算降级。

每项分别报告：类型/契约、Native CPU、Native GPU、Web SSR、真实浏览器、平台宿主。
