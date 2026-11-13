# E7 动画 / 过渡执行器与 E8 二维变换

> 依据：`jasper-appfw/docs/引擎能力补齐规划.md` §1（动画两边各一半：Jasper 声明、引擎逐帧插值，Jasper 不发逐帧 patch）、§2 E7/E8、
> §4 第 3 步、§5 治理（2026-11-12 用户授权的冻结例外）；CONTRACT-v1 §2、§2.2、§3（T3 `env reduce-motion`、T4 `xstyle` / `hello`）。
> 分支 `appfw/e7-e8`（叠在 `appfw/e2-e10` 上，不合并进 main），2026-11-13。
> 只适用于原生引擎（`native-blitz-pending` 及以上）；旧原型引擎 `LegacyEngine` 忽略这些字段，也不声明这些能力。

## 1. 字段（`jasper_ui_engine::style::Style`）

两个字段都经 `ScenePatch::Insert` / `UpdateStyle` 的完整 `Style` 传递（`ScenePatch` 类型不变），默认值与改动前逐像素一致。
校验并入 `Style::extended_style_valid()`；原生路径对非法值**拒绝整帧**（`SceneUpdateError::NativeRejected`），不钳制。

| 项 | 字段 | 类型 / 默认 | 合法范围 |
|---|---|---|---|
| E8 变换 | `transform` | `Transform2D { translate_x, translate_y, rotate, scale_x, scale_y, origin_x, origin_y }`，默认 `IDENTITY`（平移 0、旋转 0、缩放 1、原点 0.5/0.5） | 平移 ±16384 逻辑像素；旋转 ±3600 度（顺时针为正，y 向下）；缩放 ±64；原点为边框盒比例 ±4 |
| E7 过渡 | `transitions` | `Vec<Transition>`，默认空 | 至多 16 条；见下 |

`Transition { properties: AnimatedProperties, duration_ms, delay_ms, easing: Easing, reduced: Option<ReducedTransition> }`：

- `properties`：位集，单项为 `BACKGROUND`、`COLOR`（文字色）、`BORDER_COLOR`（统一边框色与各单侧边框色）、`OPACITY`、`CORNER_RADIUS`、
  `SHADOW`（各层偏移/模糊/扩展/颜色）、`OUTLINE`（宽度/偏移/颜色）、`WIDTH`、`HEIGHT`、`TRANSFORM`；`ALL` 为全部。`MOVEMENT = WIDTH|HEIGHT|TRANSFORM`。
- `duration_ms`、`delay_ms`：0..=60000。
- `easing`：`CubicBezier { x1, y1, x2, y2 }`（x1、x2 ∈ 0..=1，|y| ≤ 8，允许回弹）或 `Spring { mass, stiffness, damping }`（均 > 0，上界 1000 / 100000 / 100000；弹簧忽略时长）。
- `reduced`：减少动态下非移动属性改用的时长与**贝塞尔**曲线；`None` 表示减少动态下立即生效。
- 同一属性被多条规则覆盖时以**最后一条**为准（同 CSS）。`Style::transition_for(property)` 给出生效规则。

构造器：`Style::transform(..)`、`Style::transition(..)`（追加一条）；`Transform2D::{translate, scale, rotate, scale_xy, with_origin, matrix}`；
`Transition::{new, spring, delay, reduced}`；`Easing::{LINEAR, EASE, EASE_IN, EASE_OUT, EASE_IN_OUT, cubic, spring}`。

## 2. JUI 2 `xstyle` 键与取值格式（`Style::apply_xstyle` 已实现）

数值为无单位十进制（不接受 `inf`/`nan`）；时间必须带 `ms`。出错时样式不变，返回 `XStyleError`；`Style::xstyle_pairs()` 输出规范形式，可逐条还原。

### `transform`

`none`，或下列函数各至多一次，以 `;`（规范输出）或空格分隔：

| 函数 | 含义 |
|---|---|
| `translate(<x>,<y>)` | 逻辑像素 |
| `rotate(<度>)` | 顺时针为正 |
| `scale(<s>)` / `scale(<sx>,<sy>)` | 倍数 |
| `origin(<fx>,<fy>)` | 变换原点，边框盒比例（默认 `0.5,0.5` 中心；`0,0` 左上角） |

合成顺序**固定**为 CSS 的 translate → rotate → scale（绕原点），与书写顺序无关。例：`transform=scale(0.96)`、`transform=translate(0,8);rotate(-2);origin(0.5,1)`。

### `transition`

`none`，或若干条规则以 `;` 分隔；每条规则内各段以 `:` 分隔：

```text
<属性>:<时长>ms[:<贝塞尔>][:delay=<n>ms][:reduce=<n>ms[/<贝塞尔>]]
<属性>:spring(<质量>,<刚度>,<阻尼>)[:delay=<n>ms][:reduce=<n>ms[/<贝塞尔>]]
```

- `<属性>`：逗号分隔的 `background` `color` `border-color` `opacity` `corner-radius` `shadow` `outline` `width` `height` `transform`，
  以及 `size`（= `width,height`）、`all`。
- `<贝塞尔>`：`linear` `ease`（默认）`ease-in` `ease-out` `ease-in-out` 或 `cubic(<x1>,<y1>,<x2>,<y2>)`（规范输出总是 `cubic(...)`）。
- `reduce=` 省略曲线时为 `linear`；`reduce` 只能用贝塞尔。

例（规划中的样例）：

```text
xstyle 12	transition=opacity,background:200ms:cubic(0.2,0,0,1):reduce=150ms;transform:spring(1,170,26)
xstyle 12	transition=all:120ms:ease-out;size:80ms:linear:delay=40ms
```

## 3. 能力声明（供宿主 `hello`）

`native::host::style_capabilities()` 新增两项（追加在 E2–E10 七项之后）：

| 能力名 | `xstyle` 键 | 前提 |
|---|---|---|
| `transform` | `transform` | 渲染特性（`native-engine-cpu` / `gpu-pending`） |
| `transition` | `transition` | 渲染特性，**且宿主驱动 `animate` 时钟**（见 §4）；不驱动时钟的宿主不应声明 |

## 4. 执行器语义

宿主 API（`SceneHostView` = `engine::Engine`，`HostRenderer` 同名转发）：

| 方法 | 作用 |
|---|---|
| `animate(owner, now_ms: f64) -> Option<AnimationTick { changed, running }>` | 把过渡推进到宿主时钟 `now_ms`（毫秒，单调；更早的时间按上次处理）并把呈现值写入原生文档。`HostRenderer::animate(owner, generation, now_ms)` 另经 Surface 代数准入 |
| `animating(owner) -> Option<bool>` | 有过渡在运行 |
| `set_reduce_motion(owner, on) -> bool` | 宿主「减少动态」环境 |
| `needs_frame(owner)` | 现为「有未绘变化 **或** 有过渡在运行」，因此 `frame_pace` 在动画期间每周期给出 `RenderNow` |

宿主循环：每个刷新节拍（vsync）若 `animating`，先 `animate(now)`，再按 `needs_frame` / `frame_pace` 渲染。

- **时钟**：引擎不读系统时间。首次 `animate` 即「接上时钟」；**接上之前所有过渡立即生效**（不驱动时钟的宿主与改动前行为完全一致，不会卡在起始值）。
- **启动**：一帧提交后，若某节点被 `UpdateStyle` 改变的属性被**新样式**的规则覆盖，则为该属性建一个通道。通道起点是**上次时钟采样时的呈现值**，
  在下一次 `animate` 时开始计时（该帧呈现起点值，等价于 CSS 在样式变化时刻开始过渡）。新插入的节点不做过渡（同 CSS 首次样式）；
  同帧删除后同 id 重新插入视为新节点；删除的节点丢弃其通道。
- **续接**：运行中目标再变时，从当前呈现值重新起通道（不跳变）；弹簧另**继承当前速度**（贝塞尔不继承）。目标未变的通道继续运行；
  新样式不再覆盖该属性时立即落到目标值（同 CSS 取消过渡）。
- **插值**：颜色在预乘 sRGB 中线性插值（从透明过渡时色相不偏）；阴影层数不同时以同类（inset 一致）透明层补齐，inset 不一致的层改为立即；
  宽高只在两端同为 `Px` 或同为 `Percent` 时插值（`Auto` 两端立即）；外框有无变化、单侧边框有无变化时立即。弹簧越界的中间值钳制到合法范围
  （透明度 0..1、长度 ≥ 0、缩放 ±64 等），极小缩放在投影时保持可逆（|s| ≥ 1e-4）。
- **曲线**：三次贝塞尔按 CSS 定义（牛顿迭代 + 二分回退，端点精确为 0/1）。弹簧对每个分量用阻尼谐振子的**闭式解**
  （欠阻尼 / 临界（|ζ−1| < 1e-3）/ 过阻尼三种形式），无数值积分漂移、不产生 NaN。
- **结束判定（明确）**：贝塞尔在 `延迟 + 时长` 处结束；弹簧在**解析衰减包络** `A·e^(−λt)` 对所有分量都不超过容差的最早时刻结束，
  之后精确落到目标值。容差：颜色通道 0.25/255 级、透明度 0.5/255、长度与平移 0.01 逻辑像素、旋转 0.01 度、缩放与原点 1e-4。弹簧最长 60 s（之后直接落位）。
- **减少动态**：开启时运行中的通道立即结束；之后 `width` / `height` / `transform`（移动类）的变化**总是立即**；其余属性用规则的 `reduce=`
  替代（时长 + 贝塞尔，保留规则的延迟），没有替代则立即。Jasper 侧 `MotionToken.reduced()` 已在主题层改写；引擎这一层是兜底，保证
  减少动态下不出现位移/缩放动画，即使运行时发来了含 `transform` 的规则。
- **场景版本**：动画**不消耗、不改变**场景版本；Jasper 只发目标值。同一版本的查询在不同时刻可得到不同的呈现几何（尺寸动画中）。
- **零开销**：没有任何 `UpdateStyle` 带 `transitions` 且没有运行中的通道时，帧提交不做任何过渡工作（只克隆一个空状态）；`animate` 在无通道时
  只记录时钟；`needs_frame` 与改动前相同。

### `node_bounds` / 命中：按**当前呈现值**

选择呈现值而非目标值，理由：

1. 命中必须与像素一致——用户点的是看到的东西（按下缩放、滑入中的面板）。原生命中测试本来就在承载呈现值的同一个 Blitz 文档上进行，
   引擎侧的 E2 圆角切角与 E10 命中扩展也改为取呈现样式并在节点的变换坐标系中计算。
2. `node_bounds` 与无障碍树几何来自同一份布局；尺寸动画期间按呈现值重排，返回当前布局盒，与屏幕一致。
3. 按目标值会让动画期间点击落在「尚未到达」的位置，且需要第二份布局。

`node_bounds` 仍是**未变换**的布局边框盒（E8 变换不影响布局；文档 `api.md` 早已说明它「不是绘制/命中矩形」）。

### E8 变换的实现

- 投影为 Blitz CSS `transform: translate(Xpx, Ypx) rotate(Rdeg) scale(sx, sy)` 与 `transform-origin: fx% fy%`；恒等时为 `none`，
  因此无变换节点不建立层叠上下文、绘制与改动前完全一致。Blitz 在绘制与命中（逆变换采样点）中应用变换，布局不受影响。
- 变换节点按 CSS 建立层叠上下文：会在同级非定位内容之后绘制（CSS 语义；PSS 没有定位/层叠，只在溢出重叠时可见）。
- 引擎自绘的叠加层（E9 外框、文字装饰）用节点及其祖先的合成矩阵绘制；祖先 `overflow` 裁剪在有变换时逐层带各自矩阵压栈（无变换时仍合并为一个矩形，与之前一致）。
- E2 切角、E10 命中扩展的几何在节点的未变换布局坐标中计算（视图点经合成矩阵逆变换），修正后的采样点再变换回视图坐标交给原生命中。奇异矩阵（缩放 0）不命中。
- GPU 路径共用同一 anyrender 场景构造与 `paint_overlays`，同样生效。

## 5. 测试

| 位置 | 内容 |
|---|---|
| `src/motion.rs` 单元 | 贝塞尔端点精确、标准曲线单调（1000 点）、线性恒等、对称中点、回弹有限；弹簧三种阻尼形式在 8 组参数 × 4 组初值下无 NaN、受包络约束、结束时刻位于容差内、初值复现、速度为位移导数；无时钟/无规则立即；线性透明度逐帧与续接（续接时呈现值不变）；弹簧续接位移与速度连续、动量继续、数秒内结束；减少动态（移动立即、替代淡入、无替代立即、开启时结束运行中通道）；插值形状（预乘色、四角、阴影补层、宽度、Auto 立即、inset 不一致立即）；延迟保持旧值、规则移除即取消、删除节点丢弃 |
| `src/style.rs` 单元 | `transform` / `transition` 键解析、关键字、默认曲线、`all`/`size`、规则覆盖顺序、规范往返、24 种非法值拒绝且样式不变、规则数上限；`Transform2D::matrix` 与 CSS 顺序一致（绕中心旋转、绕左上缩放） |
| `tests/native_motion.rs`（CPU 无头出图，模拟时钟） | 能力声明；无过渡时立即且与新建视图逐像素一致、恒等变换（任意原点）与无变换逐像素一致；未接时钟立即；透明度 25/50/75% 与背景色中间值、版本不变、结束后 `needs_frame` 为假；续接无跳变；平移过渡半程位置与命中随呈现值、`node_bounds` 不动；减少动态（移动一帧到位、声明的淡入、无替代立即、中途开启立即落位）；欠阻尼弹簧越过目标后精确停在目标；静态缩放（中心/左上）与旋转 45° 的像素与命中；变换不移动兄弟布局；外框与圆角切角随平移；删除后重插不动画；宽度过渡按呈现值重排 |
| `tests/native_style_effects_gpu.rs`（`gpu-pending`） | 平移带外框盒子 + 透明度过渡中间帧，GPU 与 CPU 采样一致 |

## 6. 给宿主与 Jasper 侧的对齐说明

- **宿主（`jasper-ui-host`）**
  - `hello` 追加 `transform`、`transition`（来自 `style_capabilities()`）。只有在渲染循环每个刷新节拍调用 `HostRenderer::animate(owner, generation, now_ms)`
    时才声明 `transition`；`now_ms` 用单调时钟（如 `Instant` 相对启动的毫秒，f64）。
  - 渲染循环：`if renderer.animating(owner) == Some(true) { renderer.animate(owner, gen, now) }` → 再按 `needs_frame` 渲染。
    动画期间 `needs_frame` 恒为真；`animating` 变假后回到按需渲染。动画不产生 `ack`、不改变版本。
  - 宿主读取系统「减少动态」设置并向运行时发 `env reduce-motion on|off`（T3，宿主 → 运行时）时，**先**调用
    `HostRenderer::set_reduce_motion(owner, on)` 使引擎兜底，再发 `env`；启动时按当前系统设置调用一次。
  - `xstyle <id> transform=… transition=…` 与其他 `xstyle` 键一样逐条 `apply_xstyle` 并与基础 `style` 行合成一个 `UpdateStyle`。
    注意：同一节点每次 `style` 行整体替换样式，**`transition` 必须随每次样式更新一起重发**，否则新样式没有规则、变化立即生效（同 CSS）。
- **Jasper（`EngineStyleMapper` / `jasper.ui.theme`）**
  - 现 `MotionToken.css()` 输出 `200ms cubic-bezier(x1,y1,x2,y2)`，需改为本约定：`<属性>:<时长>ms:cubic(x1,y1,x2,y2)`；属性列表由状态映射决定
    （悬停/按下 `background,color,border-color,opacity` 用 `motion.fast`，状态过渡用 `motion.standard`，出现/消失 `opacity,transform` 用 `motion.emphasized`）。
  - 弹簧（Apple 家族，`docs/UI-主题体系设计.md` §9 待开发项）可直接用 `spring(<质量>,<刚度>,<阻尼>)`，如 `transform:spring(1,170,26)`；
    减少动态替代用 `reduce=<≤150>ms/linear`（与 `MotionToken.reduced()` 一致）。`movement=true` 的 Token 映射到含 `transform`/`size` 的规则即可，
    引擎在减少动态下会强制这些属性立即生效。
  - 按下缩放等用 `transform=scale(0.96)`；降级矩阵中「过渡 / 动画驱动」「变换」两项在宿主声明能力后从「立即切换」改为「已实现」。
  - 进入/退出动画（插入即淡入、删除前淡出）**不在本步**：过渡只作用于已存在节点的样式变化；需要淡入时先插入初始样式、下一帧改目标值；
    淡出需 Jasper 在动画结束后再发 `remove`（或另立需求让引擎支持「退出过渡」）。
