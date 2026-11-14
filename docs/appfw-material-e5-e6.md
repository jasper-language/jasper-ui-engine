# E5 材质（毛玻璃）与 E6 渐变

> 依据：`jasper-appfw/docs/引擎能力补齐规划.md` §2 E5/E6、§4 第 4 步、§5 治理（2026-11-12 用户授权的冻结例外）；CONTRACT-v1 §3、§3.1
> （T4 `xstyle` / `hello`）；`jasper-appfw/src/jasper/ui/theme/MaterialToken.jas`。分支 `appfw/e5-e6`（叠在 `appfw/e7-e8` 上，不合并进 main），2026-11-14。
> 只适用于原生引擎（`native-blitz-pending` 及以上）；旧原型引擎 `LegacyEngine` 忽略这些字段，也不声明这些能力。

## 1. 字段（`jasper_ui_engine::style::Style`）

经 `ScenePatch::Insert` / `UpdateStyle` 的完整 `Style` 传递（`ScenePatch` 不变）；默认 `None`，与改动前逐像素一致（无材质时绘制路径完全不变）。
校验并入 `Style::extended_style_valid()`，原生路径对非法值**拒绝整帧**（`SceneUpdateError::NativeRejected`），不钳制。

| 项 | 字段 | 类型 | 合法范围 |
|---|---|---|---|
| E6 渐变 | `background_gradient` | `Option<Gradient { shape, stops }>` | `stops` 2..=16（`MAX_GRADIENT_STOPS`），位置 0..=1 且不减；见下 |
| E5 材质 | `backdrop` | `Option<Backdrop { blur, saturation, tint, stroke, fallback }>` | `blur` 0..=128（`MAX_BACKDROP_BLUR`，高斯标准差，逻辑像素）；`saturation` 0..=8；`stroke` 宽 (0, 4096] |

- `GradientShape::Linear { angle }`：CSS `linear-gradient(<angle>deg)`，0 指向上、90 指向右（顺时针），|angle| ≤ 3600；渐变线按 CSS 跨越边框盒。
- `GradientShape::Radial { center_x, center_y, radius }`：中心为边框盒比例（±4）；`radius: None` 为 CSS 默认的到最远角椭圆，`Some(r)` 为半径 r 逻辑像素的圆（0 < r ≤ 4096）。
- 渐变画在 `background` 之上、边框之下，与背景同样按圆角边框盒裁剪（CSS `background-image` 语义）。
- `Backdrop`：节点背后已绘内容 → 高斯模糊（`blur`）→ 饱和度（`saturation`，CSS `saturate()`，1 不变、0 灰度）→ 叠加 `tint`（源覆盖）→ 可选内描边 `stroke`（沿圆角边框盒内侧）；
  全部按圆角边框盒裁剪。节点自身的 `background`、渐变、边框与子节点照常画在材质**之上**（所以发材质时 `background` 应为透明）。
  `fallback`：降级时改画的实底色；省略时降级画 `tint`（即「半透明实底」）。`Backdrop::fallback_color()` 给出实际回退色。

构造器：`Gradient::{linear, radial, circle}`、`GradientStop::new`、`Backdrop::new(blur, tint).saturate(s).stroke(w, c).fallback(c)`、
`Style::{background_gradient, backdrop}`。

## 2. JUI 2 `xstyle` 键与取值格式（`Style::apply_xstyle` 已实现）

数值为无单位十进制；颜色 `#rrggbb` 或 `#rrggbbaa`；出错时样式不变（`XStyleError`）。`Style::xstyle_pairs()` 输出规范形式，可逐条还原。

### `background-gradient`

```text
none
linear(<角度>,<色标>,<色标>...)
radial(<cx>,<cy>,<色标>,...)          # 到最远角的椭圆
radial(<cx>,<cy>,<半径>,<色标>,...)   # 半径为逻辑像素的圆
<色标> = <颜色> <位置>%               # 颜色与百分比之间一个空格
```

几何参数在第一个以 `#` 开头的参数之前。例：`background-gradient=linear(90,#ff0000 0%,#0000ff 100%)`、
`background-gradient=radial(0.5,0.3,#ffffff80 0%,#ffffff00 70%)`。取值含空格，宿主按 JUI 字段（制表符分隔、`wire::unescape`）取整值即可。
规范输出颜色总是 8 位十六进制。

### `backdrop`

`none`，或下列函数各至多一次、至少一个，以 `;`（规范输出）或空格分隔：

| 函数 | 含义 | 省略时 |
|---|---|---|
| `blur(<σ>)` | 模糊半径（高斯标准差，逻辑像素，同 CSS `blur()`） | 0 |
| `saturate(<s>)` | 饱和度系数 | 1 |
| `tint(<颜色>)` | 叠加色 | 透明 |
| `stroke(<宽>,<颜色>)` | 内描边 | 无 |
| `fallback(<颜色>)` | 降级实底色 | 用 `tint` |

例：`backdrop=blur(20);saturate(1.8);tint(#ffffffb3);stroke(1,#ffffff33);fallback(#f2f2f7)`。
规范输出总是写出 `blur`/`saturate`/`tint` 三项，`stroke`、`fallback` 有值时追加。

### 过渡属性名

`transition` 键的属性表新增 `backdrop`（`AnimatedProperties::BACKDROP`）与 `gradient`（`GRADIENT`），`all` 现含二者：

- `gradient`：两端同类（线性/径向，径向时半径有无一致）且色标数相同时，角度/中心/半径、色标位置与颜色（预乘 sRGB）逐分量插值；否则立即。
  弹簧越界时色标位置钳制到 0..=1 并保持不减。
- `backdrop`：两端都有材质、描边有无一致且 `fallback` 相同时，模糊、饱和度、着色、描边宽与色插值；有↔无、描边有↔无、回退色不同时立即。

## 3. 能力声明（供宿主 `hello`）

`native::host::style_capabilities()` 在 E2–E10、E7/E8 九项之后追加：

| 能力名 | `xstyle` 键 | 前提 |
|---|---|---|
| `material` | `backdrop` | 渲染特性（`native-engine-cpu` / `gpu-pending`） |
| `gradient` | `background-gradient` | 渲染特性 |

`material` 的降级不是静默的：超出预算时引擎画回退色并在 `backdrop_report` 里列出（§5）。能力名沿用 Jasper 主题降级矩阵（`material`）；
`gradient` 为新名（矩阵中此行原为 `—`）。

## 4. 实现

### E6 渐变

投影为 Blitz CSS `background-image: linear-gradient(<a>deg, #rrggbbaa <p>%, …)` 或
`radial-gradient(farthest-corner | circle <r>px at <cx>% <cy>%, …)`；无渐变时为 `none`。Blitz 按背景绘制（`background-clip: border-box`，圆角路径），
与 E2 圆角、E4 透明度层、E8 变换自然协同；CPU 与 GPU 后端都支持 peniko 渐变画刷。

### E5 材质：在录制帧上合成

固定版 Blitz 把 CSS `backdrop-filter` 交给节点效果层 `push_layer(…, backdrop_filter)`（在节点外阴影之后、背景之前压入），
但 **`anyrender_vello_cpu` 与 `anyrender_vello_hybrid` 都忽略 `backdrop_filter`**。因此：

1. 投影：有材质的节点写 `backdrop-filter: brightness(3141592) brightness(c0) brightness(c1) brightness(c2)`——只是**标签**（节点 id 拆成三段 22 位整数，
   f32 精确），从不当作滤镜生效（`material::tag_css` / `decode_tag`）。无材质为 `none`。
2. 绘制（`native_scene::paint_frame`，四处渲染入口共用）：本帧没有材质时与改动前完全相同（直接 `blitz_paint::paint_scene`）；有材质时把整帧录入
   `anyrender::recording::Scene`，交 `material::compose` 改写后回放到真实场景，再画引擎叠加层（外框、文字装饰）。
3. `compose`：按录制顺序遍历；遇到带标签的效果层时去掉标签，并按节点的**呈现**样式（含过渡中间值）与布局：
   - 圆角路径：效果层裁剪框即边框盒（在层坐标中），圆角经 `used_radii`（CSS 等比缩小）乘以「层单位/逻辑像素」比例，再经层变换到设备坐标。
   - 区域：设备包围盒外扩 ⌈3σ_dev⌉+1 像素并与视口相交；σ_dev = blur × 层比例 × √|det(层变换)|（含 DPI 与 E8 缩放；非等比缩放取几何平均）。
   - 背景：把**此前已改写的全部命令**（即节点背后已绘的一切，包括更早的材质）平移到区域原点，补上当时仍打开层数的 `pop_layer`，
     用 `vello_cpu` 光栅化为预乘 RGBA8；然后 CPU 模糊 + 饱和度。**工作分辨率**按设备 σ 选：σ < 4 全分辨率，4 ≤ σ < 16 二分之一，σ ≥ 16 四分之一
     （前缀直接按缩小比例光栅化，模糊用 σ/k，画回时双线性放大；σ ≥ 4 的高斯不含此上采样会丢失的细节——各平台材质实现的通行做法）。
   - 画回：层内依次追加 图像填充（设备坐标圆角路径，画刷平移到区域原点并放大 k 倍，`Pad`；k = 1 时最近邻、整数平移逐像素对齐，k > 1 时双线性）→ 着色填充 → 描边。
     节点背景、边框、子节点随后照常绘制。
4. 帧缓存、损伤、`needs_frame` 语义不变：材质随帧重算；未变化的视图仍复用上一帧。

### CPU 模糊算法（自研，`material.rs`）

- **可分离三遍盒滤波近似高斯**（SVG 1.1 `feGaussianBlur` 规定的近似）：d = ⌊σ·3·√(2π)/4 + 0.5⌋；d 为奇数时三遍居中盒宽 d；
  偶数时两遍宽 d（先左偏后右偏）加一遍居中宽 d+1；d < 2 不模糊。水平三遍 → 转置 → 「水平」三遍 → 转置回。
- 每遍用**滑动和**（u32 累加、四舍五入均值），每像素 O(1)，与半径无关：总工作量 = 6 遍 × 区域像素 + 2 次转置，O(W·H)；每行另有 O(左+右) 的启动项。
  额外内存 = 2 份区域缓冲。边缘按取边缘值（clamp）处理——区域已按 3σ 外扩，只在视口边缘起作用。
- 预乘通道作均值后仍 ≤ alpha（单元测试覆盖）。标准差误差：盒宽量化使 σ 偏小，σ ≥ 2 时 ≤ 15%（`box_passes_follow_the_svg_formula`）。
- **饱和度**：CSS/Filter Effects `saturate` 矩阵（0.213/0.715/0.072 亮度系数），在 sRGB 预乘值上直接计算（矩阵行和为 1、无偏移，可作用于预乘值），
  结果钳制到 0..=alpha。与主流浏览器一致在 sRGB 而非 linearRGB 中计算。
- **半径上限**：逻辑 σ ≤ 128（`MAX_BACKDROP_BLUR`，校验拒绝）；设备 σ（随 DPI/E8 缩放放大）在合成时钳制到 512（`MAX_DEVICE_SIGMA`），
  保证盒宽 < 2^11，使 32.32 倒数乘法的四舍五入除法精确（`reciprocal_rounding_equals_integer_division` 穷举验证）。盒滤波代价与 σ 无关，
  σ 只影响外扩区域面积与工作分辨率。
- 实现细节：每行三遍在两条行缓冲里连续完成（行留在缓存中），转置按 32×32 块进行。

### GPU 路径

两 hybrid 后端都不实现 backdrop filter，因此 GPU 路径与 CPU 路径**共用同一合成**：背景区域同样由 `vello_cpu` 光栅化、CPU 模糊，得到的图像作为普通图像画刷
交给 Vello hybrid 合成；其余命令由 GPU 光栅化。由此 GPU 帧的材质内部像素与 CPU 帧一致，差异只来自两光栅器的抗锯齿与图像采样：
`tests/native_style_effects_gpu.rs::gpu_paints_gradients_and_materials_like_cpu` 断言非边缘采样点各通道差 ≤ 3、边缘（圆角弧、描边、硬色标）差有界（≤ 64），
重复帧逐字节一致。单帧背景图像在下一帧上传完本帧图像后从图集释放（`GpuRenderer::release_images`），不随帧数累积。
这不是「GPU 上的 backdrop filter」：GPU 路径的材质代价仍在 CPU 上（与 CPU 路径同一预算），这一点显式标注于此；真正的 GPU 模糊需要 Vello hybrid 支持滤镜后再做。

## 5. 性能预算与降级（不静默）

`native::BackdropBudget { max_nodes, max_pixels }`，按绘制顺序计入：

- 默认 `BackdropBudget::DEFAULT` = 每帧最多 **8** 个模糊材质、**2^17 = 131 072 工作像素**（模糊区域 = 盒 + 3σ 外延、视口内，按工作分辨率计）。
  依据：测量机上材质的全部开销（前缀光栅化 + 模糊 + 饱和度 + 画回）约 73 ns/工作像素（§7），2^17 约 9.6 ms，即 60 Hz 帧预算（16.7 ms）的一半多一点，
  留出文档本身的绘制时间。例：1920×1080 下 σ=30 的工具栏 1920×88 + 侧栏 280×992 共 46 710 工作像素（预算 36%）；σ=8 的全宽工具栏约 6.8 万。
  一个 σ=30 的全屏浮层约 16.6 万工作像素，会超出默认预算而降级——宿主在快机器上可调高。
- 超出：该材质（及其后超出者）改画回退色实底；`FALLBACK_ONLY` 从不模糊。光栅化失败同样回退。
- 报告：`Engine::backdrop_report(owner) -> Option<BackdropReport { blurred, degraded: Vec<(NodeId, BackdropDegradation)>, pixels }>`
  给出**上一个绘制帧**的结果（原因 `NodeBudget` / `PixelBudget` / `RasterFailed`）；`HostRenderer` 同名转发。
- 宿主可调：`Engine::set_backdrop_budget(owner, budget)`（改变时重绘），例如测得帧超时后切 `FALLBACK_ONLY`。

**测法**：`tests/native_material.rs::e5_frame_cost_measurement`（`--ignored`，优化构建）——1920×1080 窗口、玻璃工具栏 1920×88 + 侧栏 280×992、σ=30、饱和度 1.8，
取 5 次最好成绩，对比同一帧两材质都降级时的耗时；`material.rs::kernel_cost_measurement` 单测核心（2048×1024，σ 4/30/128）。
本机结果见 §7。

## 6. 过渡与变换下的行为

- 材质随每帧重算：E7 过渡（任何属性，包括背后内容、材质自身、节点位置）期间每个动画帧都重新取背景并模糊——动画帧的代价 = 静态帧代价。
- E8 变换：材质路径与区域都取层变换（含祖先变换）；模糊在设备坐标轴向上做（高斯各向同性，旋转不改变结果）；非等比缩放时 σ 取几何平均（近似）。
  变换节点照常命中，材质不参与命中。
- 透明度：材质画在节点效果层内，受节点及祖先 `opacity` 作用；祖先透明度层在取背景时视为已合成（背景 = 屏幕上已有的样子）。
- 嵌套材质：内层的背景包含外层已画好的材质。

### 已知限制

- 背景只包含 Blitz 文档中先于该节点绘制的内容；引擎叠加层（E9 外框、文字装饰）在文档之后绘制，不进入任何材质背景，且画在材质之上。
- 节点自身的外阴影在效果层之前绘制，因此出现在自身背景中（只影响边缘附近）。
- 视口边缘以取边缘值补齐（不是透明），与多数平台的边缘行为一致。
- Liquid Glass 的折射/高光不在本步（规划注明为远期）。

## 7. 测试与本机数据

| 位置 | 内容 |
|---|---|
| `src/style.rs` 单元 | `xstyle_gradient_and_backdrop`：三种渐变写法、材质全部函数与省略默认值、规范往返、清除、17 + 1 种非法值拒绝且样式不变、`backdrop`/`gradient` 过渡属性名 |
| `src/motion.rs` 单元 | `material_and_gradient_interpolate_only_between_like_shapes`：角度/色标位置/预乘色、模糊/饱和度/着色中间值；异类、色标数、描边有无、有↔无立即；弹簧越界色标保序 |
| `src/native_adapter/material.rs` 单元 | 标签往返（含 u64::MAX）与非标签滤镜；盒宽公式与 σ 误差；平坦色不变、阶跃边缘单调平滑、两轴对称、预乘不越界；CSS 饱和度矩阵 |
| `tests/native_material.rs`（无头 CPU） | 渐变 0/90/180 度方向与单调、硬色标位置、中间色标、径向中心与半径对称、最远角椭圆、圆角裁剪、透明度、旋转、色标过渡中间帧与结束；毛玻璃边缘单调平滑并随半径变宽、盒外不变、**中性材质（σ0、饱和度 1、透明着色）与无材质逐像素一致**、着色与饱和度 0/2 取值、圆角外不变、描边、子节点在上、预算降级（节点/像素）与报告、恢复默认后逐像素复现、无回退色画着色、非归属方；平移下材质位置与着色过渡中间帧；6 种非法值整帧拒绝 |
| `tests/native_style_effects_gpu.rs`（`gpu-pending`） | 渐变 + 圆角材质（模糊/饱和度/着色/描边）GPU 与 CPU 一致、重复帧逐字节一致、GPU 回退 |

### 本机测量（2026-11-14）

环境：按资源约束只用 CPU 16–17 号核（该机混合架构，16–19 号为能效核，约 4.9 GHz 报告频率）、`nice -n 19`，另有 4 个 VirtualBox 虚拟机在其他核上满载；
优化构建（`cargo test --release`，为缩短构建关掉 LTO、`codegen-units=16`），取 5 次最好。数字应视为**偏保守**。

| 项 | 结果 |
|---|---|
| 内核：2048×1024 全分辨率模糊 + 饱和度（σ = 4 / 30 / 128） | 79.8 / 77.2 / 77.7 ms（≈ 37 ns/像素，与 σ 无关） |
| 整帧 1920×1080，工具栏 + 侧栏两材质（σ 30，饱和度 1.8），工作像素 46 710 | 9.4 ms；同帧两材质降级为实底 6.0 ms；材质开销 **3.4 ms**（≈ 73 ns/工作像素） |
| 优化前（全分辨率，逐字节边界检查版本），同一整帧，744 360 像素 | 64.4 ms（材质 58.3 ms）——促成了降分辨率与行内连续三遍的实现 |

命令：

```text
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
  taskset -c 16-17 nice -n 19 cargo test --release --locked -j 2 --features native-engine-cpu,accesskit-pending \
  --lib --test native_material -- --ignored --nocapture measurement
```

## 8. 给宿主与 Jasper 侧的对齐说明

- **宿主（`jasper-ui-host`）**
  - 宿主 `caps.rs` 的能力名与键都取自 `style_capabilities()`，默认声明全部：宿主把引擎依赖切到本分支后，`hello` 会自动多出 `material`、`gradient`，
    `declares_key` 自动认得 `backdrop`、`background-gradient`，无需改代码；但断言能力名列表的宿主测试需更新（追加两项）。`--caps` 可以只去掉 `material`。
  - `xstyle <id> backdrop=… background-gradient=…` 与其他键一样逐条 `apply_xstyle`、与基础 `style` 行合成一个 `UpdateStyle`；渐变值内含空格，按整字段取值。
  - 每帧渲染后可读 `HostRenderer::backdrop_report(owner)`；若 `degraded` 非空，按 H5 风格记入 `--gaps` 统计/诊断（这是显式降级，不是拒帧）。
    宿主测得帧超预算（如连续帧 > 16.7 ms）时可 `set_backdrop_budget(owner, BackdropBudget::FALLBACK_ONLY)`，并可据此不在下次握手中声明 `material`。
  - 渲染循环不变；材质不需要时钟。
- **Jasper（`EngineStyleMapper` / `jasper.ui.theme`）**
  - `MaterialToken.css()` 现输出 `blur(20px) saturate(1.8) #rrggbbaa`，需改为本约定：
    `blur(<blur>);saturate(<saturation>);tint(<tint.toHex()>);fallback(<fallback.toHex()>)`（数值不带 `px`）；需要高光描边时加 `stroke(1,#ffffff33)`。
  - 声明 `material` 后：节点基础 `style` 的 `background` 发**透明**（否则不透明背景会盖住材质），实底回退色放进 `fallback(...)`；
    未声明时维持现状（`background` = 回退色，记 Degradation）。HIG 规则（材质只用于导航/工具栏/浮层，内容区报错）仍在 Jasper 侧校验，引擎不检查用途。
  - 渐变：降级矩阵「渐变」行在宿主声明 `gradient` 后可改为已实现；Token 侧需新增渐变 Token 与 `background-gradient` 映射（三家族目前不使用渐变）。
  - 过渡：`transition` 规则可写 `backdrop`、`gradient`（如浮层出现 `backdrop:200ms:ease-out`）；`all` 现在也覆盖二者。
