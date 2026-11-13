# 扩展样式：E2 圆角 / E3 阴影 / E4 透明度 / E9 单侧边框·外框·文字装饰 / E10 命中扩展

> 依据：`jasper-appfw/docs/引擎能力补齐规划.md` §2 E2/E3/E4/E9/E10（2026-11-12 用户授权的冻结例外）、CONTRACT-v1 §3 T4
> （JUI 2 `xstyle` 与 `hello` 能力握手）、`jasper-appfw/src/jasper/ui/theme/README.md` 降级矩阵。分支 `appfw/e2-e10`，2026-11-13。
> 只适用于原生引擎（`native-blitz-pending` 及以上特性）；旧原型引擎 `LegacyEngine` 忽略这些字段，也不声明这些能力。

## 1. 字段、默认值与校验（`jasper_ui_engine::style::Style`）

全部经 `ScenePatch::Insert` / `UpdateStyle` 的完整 `Style` 传递（`ScenePatch` 类型未变）。默认值使绘制与布局和改动前逐像素一致。
校验 `Style::extended_style_valid()`；原生路径对不合法的值**拒绝整帧**（`SceneUpdateError::NativeRejected`，构造时 `None`），不钳制。
所有长度为逻辑像素，上界 `MAX_EFFECT_EXTENT = 4096`。

| 项 | 字段 | 类型 / 默认 | 合法范围 | 布局 |
|---|---|---|---|---|
| E2 圆角 | `corner_radius` | `CornerRadii { top_left, top_right, bottom_right, bottom_left }`，默认全 0 | 每角 0..=4096；超过盒子时按 CSS 统一等比缩小 | 不影响 |
| E3 阴影 | `box_shadows` | `Vec<BoxShadow { x, y, blur, spread, color, inset }>`，默认空 | 至多 `MAX_BOX_SHADOWS = 8` 层；x/y/spread ∈ ±4096；blur 0..=4096 | 不影响 |
| E4 透明度 | `opacity` | `f32`，默认 1.0 | 0..=1（有限值） | 不影响 |
| E9 单侧边框 | `border_top/right/bottom/left` | `Option<BorderSide { width, color }>`，默认 `None`（沿用 `border_width`/`border_color`） | width 0..=4096 | **占布局**（与统一边框相同，border-box） |
| E9 外框 | `outline` | `Option<Outline { width, offset, color }>`，默认 `None` | width 0..=4096；offset ∈ ±4096（负值为内环） | 不影响，也不参与命中 |
| E9 文字装饰 | `text_decoration` | `TextDecoration { underline, line_through }`，默认 `NONE` | — | 不影响 |
| E10 命中扩展 | `hit_slop` | `Edges`（上右下左），默认 0 | 每边 0..=4096 | 不影响，不绘制 |

构造器：`corner_radius(r)`、`corner_radii(CornerRadii)`、`shadow(BoxShadow)`（追加一层）、`opacity(a)`、
`border_side(Side, Option<BorderSide>)`、`outline(Option<Outline>)`、`text_decoration(TextDecoration)`、`hit_slop(Edges)`；
`border_of(Side)` 给出某侧实际边框。

## 2. JUI 2 `xstyle` 键与取值格式（建议，`Style::apply_xstyle` 已实现）

数值为无单位十进制逻辑像素（不接受 `px`、`inf`、`nan`）；颜色 `#rrggbb` 或 `#rrggbbaa`。`apply_xstyle(key, value)` 出错时样式不变，
返回 `XStyleError::UnknownKey` / `InvalidValue`；`Style::xstyle_pairs()` 按固定键序输出与默认值不同的字段，逐条 `apply_xstyle` 可还原。

| 键 | 取值 | 示例 | 清除 |
|---|---|---|---|
| `corner-radius` | 一个数（四角）或四个数，顺序左上,右上,右下,左下 | `corner-radius=8`、`corner-radius=8,8,4,4` | `0` |
| `shadow` | `<x>,<y>,<blur>,<spread>,<color>[,inset]`，多层用 `;` 分隔（第一层在最上） | `shadow=0,1,2,0,#0000000d;0,4,8,-2,#0000001a` | `none` |
| `opacity` | 0..1 | `opacity=0.5` | `1` |
| `border-top` / `border-right` / `border-bottom` / `border-left` | `<宽>,<色>` | `border-bottom=1,#e5e5e5ff` | `none` |
| `outline` | `<宽>,<偏移>,<色>`（偏移从边框盒外缘算，负值向内） | `outline=2,2,#0a84ff80` | `none` |
| `text-decoration` | `underline`、`line-through` 或 `underline,line-through` | `text-decoration=underline` | `none` |
| `hit-slop` | 一个数（四边）或四个数，顺序上,右,下,左 | `hit-slop=8`、`hit-slop=0,4,0,4` | `0` |

## 3. 能力声明（供宿主 `hello`）

`jasper_ui_engine::native::host::style_capabilities()`（常量表 `style::EXTENDED_STYLE_CAPABILITIES`）。名称沿用 Jasper 主题降级矩阵：

| 能力名（`hello` 令牌） | 覆盖的 `xstyle` 键 |
|---|---|
| `corner-radius` | `corner-radius` |
| `shadow` | `shadow` |
| `opacity` | `opacity` |
| `border-side` | `border-top` `border-right` `border-bottom` `border-left` |
| `focus-ring` | `outline` |
| `underline` | `text-decoration` |
| `hit-target` | `hit-slop` |

宿主只应在启用了渲染特性（`native-engine-cpu` / `gpu-pending`）时声明前六项的绘制能力；`hit-target` 不依赖渲染。

## 4. 实现（原生路径）

| 项 | 实现 | 命中 | 无障碍 |
|---|---|---|---|
| E2 | Blitz CSS `border-*-radius`；背景、边框、阴影与节点自身 `overflow` 裁剪都按圆角路径（Vello/anyrender） | 引擎在原生命中之上排除被圆角切掉的角：采样落在目标（或其裁剪祖先）的切角内时，跳过该子树，事件交给其外最近的祖先 | 不变 |
| E3 | Blitz CSS `box-shadow`（外阴影 `draw_box_shadow`，内阴影同） | 不变（阴影不可命中） | 不变 |
| E4 | Blitz CSS `opacity`（`push_layer(alpha)`，整子树一层；0 时不绘制） | 不变（透明节点仍可命中，同 CSS） | 不变 |
| E9 单侧边框 | Blitz CSS 各侧 `border-*-width/color` 长属性（覆盖统一边框简写） | 不变 | 不变 |
| E9 外框 | 引擎在文档之后自绘环（偶奇填充的两个圆角矩形，圆角随边框圆角 + 偏移）；乘祖先与自身透明度；受**祖先** `overflow` 裁剪，不受自身 `overflow` 裁剪 | 不参与 | 不变 |
| E9 文字装饰 | 引擎在文档之后自绘：每个文字节点取自身与祖先的装饰（CSS 传播），用**装饰所在节点**的文字颜色；每个排版行一条，横跨该行字形串；下划线位置/粗细取字体度量，删除线在基线上方 ascent/3（同 Blitz） | 不变 | 不变 |
| E10 | 引擎在原生命中之上：采样不在任何真实盒内、原生目标是某扩展节点的严格源祖先、且不在其裁剪祖先之外时，把原生采样点修正到该节点内最近点（最近者胜，相同取绘制顺序靠后者）；报告给 Jasper 的位置仍是实际位置 | 真实盒总优先于邻居的扩展区 | 不变（`node_bounds` 仍是布局盒） |

为什么外框与文字装饰不走 Blitz CSS：固定版 Blitz 没有 `outline-offset`；文字装饰只在同一行内格式化上下文内传播，flex 元素（PSS
默认 `display: flex`）把文字包进匿名块后自身装饰不生效，也不会从容器传到子文字节点。二者因此由引擎绘制（`paint_overlays`），
**绘制在全部节点之上**：PSS 没有定位/层叠，兄弟节点只有在溢出时才会重叠，此时环/装饰线不会被后绘内容遮住（与多数原生工具包的焦点环一致）。

### GPU 路径

CPU（`render_cpu*` / `render_to_surface*` / 插件层帧）与 GPU（`render_gpu*`、`render_gpu_to_host_view`）共用同一 anyrender 场景构造：
E2/E3/E4/E9 边框经 Blitz 绘制，外框与文字装饰经同一个 `paint_overlays`，四处调用点都已接入，因此 GPU 路径同样生效。
GPU 像素断言见 `tests/native_style_effects_gpu.rs`（`gpu-pending`，需要 wgpu 适配器）：圆角、阴影、外框、透明度采样点与 CPU 帧一致，下划线两条路径都存在；2026-11-13 本机（Ubuntu，硬件 wgpu 适配器）通过。

## 5. 测试

- 单元：`style.rs`（默认值、校验边界、单侧覆盖、`xstyle` 全键解析/拒绝不变/往返、能力表与解析器一致）、
  `native_adapter/style_effects.rs`（圆角缩减、切角判定、扩展盒与夹取、外框环几何）。
- 无头 CPU 出图：`tests/native_style_effects.rs`——圆角处背景透出与裁剪、阴影在盒外且不改布局、多层/内阴影、透明度混合值与组透明度、
  单侧边框只在该侧、外框在盒外（偏移间隙、自身 overflow 不裁、祖先裁剪、内环、圆角环、祖先透明度）且不改布局、下划线与删除线存在且
  上下位置正确、容器装饰传到居中子文字且用容器颜色、hit-slop 扩大命中但布局与像素不变、真实盒优先与最近扩展胜出、圆角切角不命中、
  非法值整帧拒绝、能力声明。

## 6. 给宿主与 Jasper 侧的对齐说明

- **宿主（`jasper-ui-host`）**：`hello` 列出 `style_capabilities()` 的名称；收到 `xstyle <id> k=v...` 时对节点当前 `Style` 逐条
  `apply_xstyle`，任一条失败按 H5 显式拒帧；未声明的键按现有 `--gaps` 规则处理。`xstyle` 应与同一节点的基础样式行合成一个
  `UpdateStyle`（引擎每次接收完整 `Style`）。
- **Jasper（`EngineStyleMapper`）现有扩展键与本约定的差异**（映射时需改）：`radius=<n>px` → `corner-radius=<n>`；
  `border-<edge>=1px #hex` → `border-<edge>=1,#hex`；`hit-min=<n>px`（最小命中尺寸）→ `hit-slop`（引擎只接受各边扩展量，
  由 Jasper 按 `max(0, (min - 视觉尺寸) / 2)` 计算）；`shadow` / `outline` 由 `ShadowToken.css()` / `FocusRing.css()` 的 CSS 写法
  改为上表逗号格式；`text-decoration=underline` 已一致。`backdrop`、`transition`、`indicator`、`placeholder`、`scrim` 不在本步。
- 节点透明度（E4）：降级矩阵原为「不使用（预合成颜色）」，声明 `opacity` 后可直接发 `opacity`（禁用态、`opacity-50`）。
- 外框（`focus-ring`）声明后不再需要「改画成边框、内边距减线宽」的降级。
