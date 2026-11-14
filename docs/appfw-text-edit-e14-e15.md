# E14 场景内编辑态显示、E15 文字垂直对齐（分支 `appfw/e14-e15`，2026-11-14）

依据：`jasper-appfw/docs/引擎能力补齐规划.md` §2 E14/E15；契约 `CONTRACT-v1.md` §2、§2.1 H7、§3.1。
分支叠在 `appfw/e5-e6`（f1f3e0a）之上，按用户裁决 C 不合并 main。

## 1. E15 文字垂直对齐与行高

### 1.1 字段（`src/style.rs`）

| 字段 | 类型 | 默认 | 含义 |
|---|---|---|---|
| `text_align_vertical` | `Option<TextAlignVertical>`（`Top`/`Center`/`Bottom`） | `None` | Text 节点内容盒高于文字时文字的垂直位置；`None` = 不覆盖节点自身的 flex 对齐（默认靠上） |
| `line_height` | `Option<LineHeight>`（`Px(f32)` / `Multiple(f32)`） | `None` | 节点自身文字的行高；`None` = 字体正常行高 |

校验（`extended_style_valid`）：`Px` ∈ (0, 4096]（`MAX_EFFECT_EXTENT`），`Multiple` ∈ (0, 16]（`MAX_LINE_HEIGHT_MULTIPLE`），须有限。
两字段**只作用于 Text 节点**（容器上设置被接受但无效果，也不经 CSS 继承传给子节点）；**不参与过渡**（E7 规则不覆盖，变化立即生效）。

### 1.2 xstyle 键与能力名

| 能力（`hello` 令牌） | 键 | 取值 |
|---|---|---|
| `text-valign` | `text-valign` | `top` \| `center` \| `bottom` \| `none`（清除） |
| | `line-height` | `<数>px`（逻辑像素，如 `20px`）\| `<数>`（字号倍数，如 `1.5`）\| `none` |

其余写法（`middle`、`20em`、`0`、负数、超界、`nan`）→ `InvalidValue`，样式不变。`xstyle_pairs` 规范输出：`text-valign=center`、`line-height=20px` / `line-height=1.5`。
能力已加入 `style_capabilities()`（`EXTENDED_STYLE_CAPABILITIES` 末项）。

### 1.3 实现

`native_scene.rs::text_style_properties` 在基础属性之后写 CSS（后写者胜）；默认值什么都不写，**现有像素逐像素不变**（测试 `defaults_and_explicit_top_are_pixel_identical`）：

- `line-height`：`Px` → `<v>px`，`Multiple` → 无单位数。
- 垂直对齐移动 Text 元素 flex 容器里的**匿名文字盒**：列方向（默认）写 `justify-content`，行方向写 `align-items`，
  `display: block` 的 Text 节点改写为单项拉伸的列（文字排版与块相同）；`display: none` 不写。
- 元素自身的盒不动：布局尺寸、命中区、`node_bounds` 不变；内边距内的内容盒为对齐区域；圆角、阴影、变换照常（变换作用于整个节点，文字随之）。
- 文字高于盒时按 CSS 规则向两侧（center）或上方（bottom）溢出，由节点 `overflow` 决定是否裁剪。

## 2. E14 场景内编辑态

### 2.1 会话与焦点

- **可编辑节点**：`NodeKind::Text` 且语义角色 `TextField`、未禁用、源可见。
- 场景焦点落到可编辑节点（指针、Tab、`focus`、AccessKit Focus）即开始**编辑会话**：缓冲 = 节点场景文字（去掉控制字符），光标在末尾。
- 焦点离开（指针点别处、Tab、`focus`/`clear_focus`/`deactivate`）结束会话，**失焦提交**（见 2.3）。
- 帧使节点被删除、变为不可编辑（禁用、隐藏、改角色）时会话**静默结束、不提交**（Jasper 的帧为准）。
- 编辑缓冲只写原生文档的文字节点（`set_node_text` + 重新排版），**从不写源场景、从不消耗场景版本**（`snapshot_identity` 不变）；
  会话、待取事件随每帧的候选视图一起携带（`carry_edit_from`）。
- TextField 文字节点固定 `white-space: pre-wrap`，使编辑文本（含连续空格）与布局逐字节对应；普通文字不受影响。

### 2.2 输入

| 来源 | 行为 |
|---|---|
| `text_input(text)`（键入字符、输入法提交、粘贴） | 替换选区插入；丢弃控制字符（单行）；结束组合 |
| `Backspace` / `Delete` | 删选区，否则删前/后一个**字素簇**；Ctrl 或 Alt 时按词 |
| `ArrowLeft` / `ArrowRight` | 按字素簇移动（Ctrl/Alt 按词），Shift 扩展；无 Shift 时有选区先折叠到对应端 |
| `Home` / `End` | 到首/尾，Shift 扩展 |
| Ctrl/Meta + `A` / `C` / `X` / `V` | 全选 / `Copy(选中文本)` / `Copy` + 删除 / `PasteRequested`（剪贴板钩子：宿主写/读剪贴板，粘贴经 `text_input` 回送） |
| `Enter` | 提交（若有变化） |
| `Escape` | 缓冲还原为节点场景文字（有变化时报 `Changed`） |
| 指针主键按下（未阻止默认行为）于会话节点 | 光标放到最近偏移（Shift 扩展）；按住移动扩展选区；抬起/取消结束拖动 |
| `set_preedit(Some(text), cursor)` / `None` | 在光标处显示组合文本（带下划线）；组合期间编辑键与指针定位交给输入法 |

键先照常交给 Framework 的键事件 sink；sink **阻止默认行为**时编辑器不处理。被编辑器消费的键不再触发角色激活。
所有偏移为 UTF-8 字节、字符边界。

### 2.3 事件发件箱（`take_edit_events`）

`SceneEditEvent { scene_version, node, kind }`，`kind`：

- `Changed(text)` —— 缓冲文本变化（光标移动不报）。`--live-text` 宿主把它转发为 `text-commit`。
- `Commit(text)` —— **JUI 2 `text-commit`**：Enter 或失焦，且文本 ≠ 节点场景文字且 ≠ 上次提交（Enter 后再失焦不重复）；payload 为完整文本。
- `Copy(text)`、`PasteRequested`、`CompositionCancelled`（会话结束或外部更新丢弃了进行中的组合，宿主应重置输入法）。

失焦提交后、Jasper 回显帧到达前，文字节点**保持显示已提交文本**（不闪回旧值）；下一个已应用帧显示帧里的文字。立即重新聚焦时从该文本继续。

### 2.4 外部 `text` 更新冲突规则（**以 Jasper 为准**）

编辑中帧更新同一节点的文字：

- 新文字 = 当前缓冲（回显，如实时文本模式）→ 保留光标与选区；
- 否则缓冲替换为新文字，**光标移到末尾**、清选区，进行中的组合丢弃并报 `CompositionCancelled`；之后以新文字为提交基准。

理由：Jasper 是唯一状态源（受控输入）——格式化、清空、校验修正都由它下发；保留旧编辑会让后续提交悄悄覆盖 Jasper 的更新，两边分叉。
回显等值保留光标，使实时文本模式下中间插入不跳光标。样式/语义帧（不改文字）不影响会话。

### 2.5 显示与闪烁

- 叠加层在文档之后、焦点外框之前绘制（`edit_overlays`），与节点文字同样受祖先及自身 overflow 裁剪、在 E8 变换坐标系中、乘透明度链：
  选区高亮 `rgba(51,132,255,96)`；预编辑下划线 1 逻辑像素、文字颜色、对齐到整像素；光标宽 1 逻辑像素、文字颜色，有选区时不画。
  空文本时排版零宽空格，使光标得到行高与 E15 对齐。CPU 与 GPU 路径共用。
- 闪烁由宿主帧钟驱动：`caret_blink(now_ms)` 亮 530 ms、灭 530 ms，返回 `next_toggle_ms` 供宿主定时唤醒；每次编辑/移动光标后从「亮」重新开始；
  **减少动态时常亮**（`next_toggle_ms = None`）。宿主从不调用时光标常亮。切换只使帧失效，不耗版本。

### 2.6 无障碍

`accessibility_tree` 中被编辑节点的 `value` = 编辑缓冲，新字段 `AccessibilityNode::text_selection = Some((anchor, caret))`（`value` 内 UTF-8 偏移）；
其余节点为 `None`。AccessKit 桥照常输出 `value`；AccessKit `TextSelection`（需 inline text box 子节点）暂缓（见暂缓登记）。

### 2.7 宿主接口（`Engine` = `SceneHostView`；`HostRenderer` 同名方法带 `generation`）

```text
text_input(owner, version, text) -> bool
set_preedit(owner, version, text: Option<&str>, cursor: Option<(usize, usize)>) -> bool
take_edit_events(owner) -> Option<Vec<SceneEditEvent>>
edit_state(owner, version) -> Option<Option<SceneEditState>>   // node、text、anchor、caret、preedit、caret_rect（输入法候选窗定位）
caret_blink(owner, now_ms) -> Option<CaretBlink>               // visible、next_toggle_ms
```

键盘与指针沿用 `key` / `key_with_activation` / `pointer` / `pointer_input`。`text_caret` / `text_selection` / `text_offset_at` 在编辑期间作用于显示文本（缓冲 + 预编辑）。

## 3. 宿主对齐（`jasper-ui-host`，本步未改）

1. **键盘/IME 路由到引擎**：`WindowEvent::KeyboardInput` 的命名键照常经 `key_with_activation`；`event.text`（非 Ctrl/Meta、非控制字符）与 `Ime::Commit` 调 `text_input`；
   `Ime::Preedit(text, cursor)` 调 `set_preedit(Some(text), cursor)`（空串或 `Ime::Disabled` → `None`）。删掉宿主自己的 `edit` 缓冲、`edit_buffer`、`Backspace` 处理。
2. **去掉窗口标题回显**（`update_title`）；用 `edit_state().caret_rect` 调 `window.set_ime_cursor_area`。
3. **`text-commit` 时机**：每次键盘/指针/焦点/`text_input`/帧调用后 `take_edit_events`，`Commit(text)` → `event <id> text-commit <text>`，且同一次调用里先于 `focus out` 行发送；
   `--live-text` 时 `Changed(text)` 也发 `text-commit`。窗口失焦走 `deactivate`，引擎自动失焦提交。
4. **剪贴板**：`Copy(text)` 写系统剪贴板；`PasteRequested` 读剪贴板后 `text_input`。`CompositionCancelled` → 关开一次 `set_ime_allowed`。
5. **闪烁**：帧循环每拍（或在 `next_toggle_ms` 定时唤醒）调 `caret_blink`，`needs_frame` 为真再绘制；`env reduce-motion` 已经 `set_reduce_motion`，光标随之常亮。
6. `hello` 增加能力令牌 `text-valign`，透传 `text-valign`、`line-height` 两个 xstyle 键。

## 4. Jasper 对齐

1. 主题给固定高度控件（按钮、输入框、标签、分段控件）声明 `xstyle text-valign=center`（需要时加 `line-height`）；能力未声明时用上下 padding 近似并记降级。
2. **TextField 约定**：文本框必须是 `insert … text` 节点 + `sem role=text-field focusable=true`（`Ui.jas` 现状即如此）；引擎在该节点上就地编辑。
   提交语义不变（`text-commit` = Enter 或失焦、完整文本）。运行时收到提交后用 `text <id> <s>` 回显或改写；改写（与缓冲不同）会替换编辑中的内容并把光标移到末尾（§2.4）。
3. 占位符（hint）仍由组件用独立节点或 label 表达；引擎不画占位符。

## 5. 限制

- 单行：控制字符（含换行、制表）在输入时丢弃；文本超出宽度时不水平滚动跟随光标（`pre-wrap` 换行显示），长文本单行滚动待后续。
- 光标按逻辑顺序移动（双向文本的视觉移动未做）；无双击选词、无撤销/重做；上下方向键不处理。
- 选区颜色固定，尚无主题键。
- AccessKit `TextSelection` 与 `SetTextSelection`/`ReplaceSelectedText` 动作未接。

## 6. 测试

- `tests/native_text_valign.rs`（6）：xstyle 语法与往返/拒绝、默认与显式 top 逐像素一致、居中上下留白相等（±1px）且墨迹位移一致、底对齐、
  多行 + 行高 + padding、行方向与块节点、圆角/命中（文字上方仍命中）/变换。
- `tests/native_text_edit.rs`（10）：会话开始与版本不变、光标像素随输入移动、闪烁与减少动态、键盘编辑/选区高亮像素/剪贴板钩子/空文本光标、
  Enter/失焦只提交一次与 Esc、预编辑下划线像素与提交、指针定位与拖选、外部更新/回显/样式帧/禁用、删除静默结束、无障碍 value/选区、非文本框不编辑。
- `src/native_adapter/text_edit.rs` 单元（5）：字素簇删除、选区与按词移动、UTF-8 边界、预编辑映射、整体替换。
