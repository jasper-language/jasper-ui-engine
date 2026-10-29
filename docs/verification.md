# 数学形式化证明与交付门槛

> 2026-10-23：数学形式化证明为硬性要求；测试、有限模型检查、Rust 内存安全与 AI 判断均不能替代实现级证明。

## 强制验证顺序（2026-10-23 用户追加）

**Verus + Z3 → Proptest → Miri → cargo-fuzz → Kani → Loom**。
旧代码允许后续补证；每轮推进框架新功能的同时，为一部分旧代码补齐这条链。新增或修改的实现须同步完成全链才算交付。性能不足记录为后续优化项，不阻塞当前推进，待 UI Rust 引擎内核完成后集中优化；正确性和安全性失败仍不能算通过。六项分别记录源码、规格、工具版本、配置、范围和结果。后项不能补偿前项失败，不把测试、fuzz 或有限模型检查当成实现级证明。

首个顺序入口：

```sh
python3 scripts/verify-ui.py --verus /absolute/path/to/verus
```

当前入口按六阶段执行：二十二个 Verus 证明入口（含资源退役、焦点清理及既有切片）、批解码及原样提取内核的属性/Miri 检查、两个真实 libFuzzer 目标、二十二个 Kani harness、Loom 调度场景。非零退出、缺少成功证据或运行期间源码变化均不算通过。`slice_complete` 只代表指定切片，`complete` 仍为 false，整个 UI 尚未验收。CI 已配置同一顺序入口，远端执行尚未发生。

初始属性集在 `tests/batch_properties.rs`：任意 0..4095 字节解码成功后字节往返、有效无载荷操作批的所有截断与尾部拒绝、恶意操作计数错误分类。每项默认 256 cases，保留 Proptest 默认反例持久化和 shrink；这不证明完整协议、无限输入或运行时额度。

Loom 当前执行实际生产函数体的调用侧调度场景：两个线程共享只读字节数组、各自持有游标和 Surface 行额度，交错读取/失败不推进/空读取，检查互不串扰。生产内核没有同步原语，因此这不证明锁实现、IPC 或 compositor 的并发协议；新增这些路径时须以真实实现替换同步原语并补模型。其余模块的逐项适用性审查仍未完成，不能扩大本切片结论。

## 证据状态

- `proved`：明确规格和信任假设下，机器验证器验证指定实现/性质；范围不能扩大；版本内核首次实际复核已通过，报告见下文。
- `model_checked`：仅指定模型与边界；不自动证明 Rust 实现。
- `tested`：具体测试覆盖，不是数学证明。
- `unknown / unsupported / not_verified`：尚不能通过对应证明门槛。

所有模块必须有证明义务或明确的信任边界。不能将第三方依赖整体当作公理，随后宣称整套 UI 正确。每次交付同时检查源码/规格/依赖是否使旧证据失效。

## 首批性质登记

| 性质 ID | 规格/实现对象 | 应证明性质 | 当前证明状态 |
|---|---|---|---|
| UI-TXN-VERSION-001 | `scene.rs::next_scene_version` | 对所有 u64：小于最大值时返回严格增加 1 的版本；最大值时拒绝；无回绕/溢出 | 以 `evidence/version-kernel/report.json` 的实际结果和源码哈希为准；报告缺失/失配即 not_verified |
| UI-TXN-ADMISSION-001 | `scene.rs::decide_scene_version` 及版本计算依赖 | 陈旧版本优先拒绝；匹配空批不推进；匹配非空批耗尽拒绝；可推进时精确加 1 | proved，两个生产函数体同时验证，2 verified/0 errors；错误准入反例被拒；需匹配当前源码哈希 |
| UI-TXN-ATOMIC-001 | `Scene::apply_batch_at` | 非法/陈旧/耗尽失败保持旧场景；非空成功推进一次；空批不变 | not_verified；有单元测试，尚无整个事务证明 |
| UI-SCENE-ID-001 | `Scene::try_new` / `validate` / 插入/布局入口 | 全树 ID 唯一、父子关系与根约束保持 | not_verified；子树内部/跨树重复 ID 已检查，回滚与布局拒绝已有测试；遍历、BTreeSet、clone 与整个更新链尚未证明 |
| UI-DECODE-RANGE-001-END | `batch.rs::checked_read_end` | 对所有 usize 输入：成功终点等于起点+长度且在边界内；失败当且仅当读取非法；运算不溢出 | proved，生产函数体 Verus 1 verified/0 errors，错误终点反例被拒；证据需匹配源码哈希 |
| UI-DECODE-RANGE-001 | `batch.rs::Cursor::take` 与真实读取依赖 | 返回请求字节序列；成功精确推进；失败位置不变且错误为 Truncated；保留缓冲字节视图 | proved，原始 Cursor/方法/错误类型及读取依赖同场验证；反例被拒；不覆盖复合字段与完整解析器 |
| UI-DECODE-READ-001 | `batch.rs::take_bytes` 与真实端点依赖 | 返回精确原字节区间；成功位置精确增加 len；失败位置不变且范围确实非法 | proved，2 verified/0 errors；成功不推进/失败改位置两个反例被拒；需匹配源码与脚本哈希 |
| UI-DECODE-SCALAR-001 | `Cursor::u8/u16/u32` 与真实读取依赖 | 精确字节/小端数值；成功推进 1/2/4 字节；失败位置不变且错误为 Truncated；运算不溢出 | proved；错误字节/错误小端权重反例被拒；不含浮点与字符串 |
| UI-DECODE-HEADER-001 | `decode_header` 与真实读取依赖/协议常量 | 十字节帧头、魔数、版本、计数、上限/剩余载荷检查与错误分类顺序 | proved；错误计数/错误版本分类反例被拒；不含载荷解析与完整内存预算 |
| UI-DECODE-PAYLOAD-001 | `Cursor::bounded_bytes` 与真实整数/读取依赖 | 长度前缀准确、成功载荷精确且不超限、前缀不足/超限/载荷不足的精确位置与错误策略 | proved；绕过长度限制/取错长度反例被拒；不含 UTF-8 转换及全局内存额度 |
| UI-FOCUS-CLEANUP-001 | 既有 FocusManager::blur/order/focused | 清除焦点、保持精确遍历顺序、两个访问器精确 | 以 focus-cleanup-kernel 与顺序链为准；焦点遍历/输入取消事件与宿主通知未涵盖 |
| UI-RESOURCE-BYTES-001 | `UiBytesSlot` 五个方法与真实 gate | 不变量保持、精确 Vec 接管/返还、身份受检借用、失败不变与释放后空闲 | 以 bytes-slot-kernel 与完整顺序链为准；字体解析/全局注册/IPC 和在途取消未涵盖 |
| UI-KEY-MODIFIER-001 | 既有 `no_command_modifier` | 仅 control/alt/meta 决定命令修饰键门槛，shift/key/phase/repeat 不影响 | 以 command-modifier-kernel 与完整顺序链为准；不等于完整快捷键/键盘路由已证明 |
| UI-RESOURCE-GATE-001 | `resource::ResourceGate` 五个方法 | 单槽会话/槽位/代数准入、独占签发、撤销、失败保持与耗尽不回绕 | 以匹配源码的 resource-gate-kernel 与完整顺序链为准；宿主认证/唯一 gate、真实存储和在途取消未涵盖 |
| UI-ROLE-ACTIVATION-001 | `Role::is_activatable/activates_with_space` | 所有角色分类精确，Link 可 Enter 激活但 Space 不激活 | 以匹配源码的 role-activation-kernel 与完整顺序链为准；不证明键盘路由与宿主无障碍 |
| UI-CANVAS-IMPORT-001 | `raster::Canvas::from_rgba8` 和既有 width/height/pixels | 非零尺寸且精确完整布局才接管原字节；失败返还原字节；成功裁剪栈为空、访问器精确 | 以匹配源码的 `evidence/canvas-import-kernel/` 与六阶段报告为准；旧分配/resize、clone/绘图不随之闭合 |
| UI-PIXEL-PACK-001 | `surface::rgb8_word` | 精确 RGB 数值权重、高字节为零、无运算溢出 | 当前源码的 `evidence/pixel-pack-kernel/` 与全链证据；不规定原生字节序/alpha 混色 |
| UI-PIXEL-CONVERT-001 | `desktop::rgba_to_u32` | 所有长度的前置检查安全；成功全输出逐字精确、alpha 忽略；失败全目标保持；循环边界/终止 | 当前源码的 `evidence/pixel-convert-kernel/` 与全链；窗口/呈现生命周期不随之闭合 |
| UI-COLOR-CONSTRUCT-001 | `style::Color::new/rgb/with_alpha` | `new` 四通道原样；`rgb` 三通道原样且 alpha=255；`with_alpha` 只替换 alpha、三色通道不变 | proved，4 verified/0 errors，「rgb 非不透明」「with_alpha 忽略新值」两反例被拒；以匹配源码的 `evidence/color-construct-kernel/` 与六阶段报告为准；`blend_over`（浮点）不在范围 |
| UI-FONT-WEIGHT-001 | `native_fonts::font_weight_admits` | CSS 数值字重恰在 1..=1000 时准入（充要），越界拒绝而非钳制 | proved，1 verified/0 errors，拒绝最小值 1/排除最大值 1000 两反例被拒；以匹配源码的 `evidence/font-weight-kernel/` 与六阶段报告为准；CSS `font-weight`/`font-style` 映射与 Parley/fontique 字面匹配为第三方信任边界，由 `tests/native_font_weight_style.rs` 集成测试覆盖，不在证明范围 |
| UI-FONT-FAMILY-001 | `native_fonts::font_family_admits` | 族名恰为 1..=128 字节且每字节非控制字节、非 DEL、非 `"`、非 `\` 时准入（充要），不合规拒绝而非转义 | proved（数字见本切片六阶段报告），准入禁用字节/准入超长两反例应被拒；以匹配源码的 `evidence/font-family-kernel/` 与六阶段报告为准；族名须在注入字体栈已登记族中的查找、CSS `font-family` 映射与 Parley/fontique 按名选择及缺字回退为编排与第三方信任边界，由 `tests/native_font_family.rs` 集成测试覆盖，不在证明范围 |
| UI-GLYPH-SKEW-001 | `glyph_skew::upright_skew_x` | 输入字形变换六系数的 `f64::to_bits`：恰为纯水平斜切（a、d 的位为 1.0，b、e、f 为 ±0.0）时返回 c 符号位取反，否则 c 原样（充要；NaN 系数与非纯变换不变） | proved，4 verified/0 errors（含常量），「不取反」「平移斜切也取反」两反例被拒；以匹配源码的 `evidence/glyph-skew-kernel/` 与六阶段报告为准；Proptest/Kani/Miri/fuzz/Loom 以浮点域独立规格对照；转发绘制器 `UprightSyntheticItalic` 及其在 CPU、GPU、TextDisplayView 绘制路径上的接入为第三方 PaintScene 之上的编排，由 `tests/native_font_synthesis.rs`（CPU 与 GPU 竖笔画倾斜实测）覆盖，不在证明范围 |
| UI-FONT-ORDER-001 | `native_fonts::font_stack_count_admits/font_order_admits` | 字体栈数量 1..=16 准入充要；某族 id 仅首次出现时准入，先注入文件保持回退优先级 | proved，4 verified/0 errors，空栈准入/重复准入两反例被拒；以匹配源码的 `evidence/font-order-kernel/` 与六阶段报告为准；不涵盖字体解析、上游注册、族名排序与 Parley 回退选择 |
| UI-FRAME-REUSE-001 | `frame_schedule::frame_reuse_admits` | 仅当未失效且缓存宽、高与当前一致、长度等于准入紧凑长度时复用（充要） | proved，1 verified/0 errors，忽略失效/忽略尺寸两反例被拒；以匹配源码的 `evidence/frame-reuse-kernel/` 与六阶段报告为准；不涵盖各入口标脏的完整性（由集成测试覆盖）、缓存存储、绘制与呈现 |
| UI-DAMAGE-SPAN-001 | `frame_schedule::damage_span_extend` | 结果闭区间包含新下标；无旧区间时恰为该下标；有旧区间时覆盖旧区间且两端各取旧端点或新下标（最小包络） | proved，1 verified/0 errors，下端不扩展/空区间从零起两反例被拒；以匹配源码的 `evidence/damage-span-kernel/` 与六阶段报告为准；不涵盖逐行逐列扫描 |
| UI-DAMAGE-RECT-001 | `frame_schedule::damage_rect_admits` | 矩形非空且 x+宽 ≤ 帧宽、y+高 ≤ 帧高（宽整数语义，无溢出）时准入（充要） | proved，1 verified/0 errors，接受空矩形/忽略纵向边界两反例被拒；以匹配源码的 `evidence/damage-rect-kernel/` 与六阶段报告为准；不涵盖损伤扫描与写入循环 |
| UI-SURFACE-SEGMENT-001 | `surface.rs::write_surface_segment` 及只读准入依赖 | 成功当且仅当 Surface 几何合法、行号合法且 offset+片段长 ≤ 有效行长；成功恰写入该行 offset 起的片段，其余全部字节（他行、本行其余、padding）不变；失败目标完全不变 | proved，依赖同场 3 verified/0 errors，省略复制/忽略 offset 两反例被拒；以匹配源码的 `evidence/surface-segment-kernel/` 与六阶段报告为准；copy_from_slice 的 vstd 规格为信任边界；不涵盖跨进程发布 |
| UI-DAMAGE-WRITE-001 | `frame_schedule::write_damaged_frame` | 全部准入（Surface 几何、紧凑长度、矩形边界）成立时恰写入损伤内像素（Full 全部、Unchanged 无），其余字节不变；否则不写任何字节；持有上一帧的 Surface 经 `frame_damage` 报告的损伤写入后收敛到新帧 | Verus：N/A——枚举匹配加行循环与切片子区间未改写为可验形式；所用两个内核（damage_rect_admits、write_surface_segment）均已 Verus 证明。替代证据：Proptest（5×5 以内任意帧/改动/损伤、错 stride、短帧、短 Surface，含与 frame_damage 的端到端收敛）、Kani（1–2×1–2 帧、有无 padding、任意缓冲长度/字节/损伤种类/矩形）对照独立规格，另有 Miri/fuzz；不计入六阶段完成数 |
| UI-FRAME-TICK-001 | `frame_schedule::frame_tick_at` | 周期为零或 now 早于 origin 时为 None；否则结果 t 满足 origin ≤ t ≤ now、now − t < 周期且 (t − origin) 为周期整数倍，即包含 now 的刷新周期起点 | proved，1 verified/0 errors，不对齐（返回 now）/陈旧起点（返回 origin）两反例被拒；以匹配源码的 `evidence/frame-tick-kernel/` 与六阶段报告为准；Kani 为有界域（时钟取零或 u64::MAX 附近、now 与 origin 相差至多 2^16、周期至多 2^16，另一 harness 周期取 u64::MAX 附近），全宽符号取模超时，全域由 Proptest/fuzz 补充；不涵盖时钟来源与垂直同步 |
| UI-FRAME-PACE-001 | `frame_schedule::frame_pace` 及 FramePace 类型 | 周期为零为 None；无待绘变化为 Idle；首节拍前 WaitUntil(origin)；本周期未呈现为 RenderNow(本周期节拍)；已呈现为 WaitUntil(下一节拍)，下一节拍超出 u64 为 None；错过的周期不追帧 | proved，依赖同场 3 verified/0 errors，同周期二次绘制/无变化唤醒两反例被拒；以匹配源码的 `evidence/frame-pace-kernel/` 与六阶段报告为准；Kani 为有界域（差值与周期至多 255，另一 harness 周期取 u64::MAX 附近覆盖溢出）；不涵盖 needs_frame 标脏完整性、宿主时钟与绘制 |
| UI-TEXT-OFFSET-001 | `text_geometry::text_offset_admits` | 布局文本长度等于源文本长度、偏移 ≤ 源文本长度（含末尾）且为字符边界时准入（充要） | proved，1 verified/0 errors，忽略长度不符/排除文本末尾两反例被拒；以匹配源码的 `evidence/text-offset-kernel/` 与六阶段报告为准；布局文本与源文本的内容相等由调用方逐字节比较；Parley 光标/命中几何与坐标换算为第三方信任边界，由 `tests/native_text_geometry.rs` 集成测试覆盖，不在证明范围 |
| UI-TEXT-RANGE-001 | `text_geometry::text_range_admits` | 布局文本长度等于源文本长度、start ≤ end ≤ 源文本长度且两端均为字符边界时准入（充要）；等价于两端各自满足 UI-TEXT-OFFSET-001 且区间有序 | proved，1 verified/0 errors，接受反向区间/忽略终点边界两反例被拒；以匹配源码的 `evidence/text-range-kernel/` 与六阶段报告为准；Parley 选区几何与坐标换算为第三方信任边界，由 `tests/native_text_selection.rs` 集成测试覆盖，不在证明范围 |
| UI-GPU-READBACK-001 | `gpu_render::gpu_readback_layout` | 宽或高为零、超过 65535 时为 None（与 CPU 路径同一尺寸上限）；否则返回（填充行字节数，缓冲字节数），填充行为 256 的倍数、不小于 4×宽且小于 4×宽+256，缓冲字节数 = 填充行 × 高，无溢出 | proved，2 verified/0 errors（乘法上界经 nonlinear_arith 断言），去掉行填充/放宽高度上限两反例被拒；以匹配源码的 `evidence/gpu-readback-kernel/` 与六阶段报告为准；wgpu 拷贝、缓冲映射、设备与 Vello hybrid 渲染为第三方信任边界，由 `tests/native_gpu_render.rs` 集成测试覆盖，不在证明范围 |
| UI-GPU-DAMAGE-001 | `native_scene` 中 `render_gpu_with_damage` 及 GPU Surface 写入编排（`_with_damage`/`_damaged`） | 干净视图复用最近产出帧且不做 GPU 工作；新帧损伤相对最近产出帧（跨 CPU/GPU）；Surface 准入先于 GPU 工作；被拒/失败不写字节、不移动基准；写入失败丢弃缓存 | 六阶段均 N/A：编排代码调用 Blitz 绘制与 wgpu/Vello hybrid（第三方信任边界），不可在 Verus/Kani/Miri 下执行，也无可独立抽出的新判定；其判定全部委托给已证明内核 `frame_reuse_admits`、`frame_damage`（五步）、`write_damaged_frame`（五步）、`surface_span`/`rgba8_row_bytes`。替代证据：`tests/native_gpu_frame_damage.rs` 3 项（对照独立包围盒规格）、`tests/native_gpu_render.rs` 3 项，构建虚拟机 llvmpipe/Vulkan 实测；不计入六阶段完成数 |
| UI-TEXT-DISPLAY-FONTS-001 | `native_view::NativeTextView::new_with_fonts`/`new_with_context` 与公开 `TextDisplayView::with_fonts` | 字体栈准入（空栈、超过 `MAX_INJECTED_FONTS`、任一文件无字体族）在构建任何文档之前完成并返回 None；准入通过后以宿主顺序回退栈构建与单字体构造相同的视图树；单字体 `new` 行为不变 | 六阶段均 N/A：新代码是上游 Blitz/fontique 构造之上的编排（第三方信任边界），无新判定；准入与回退序全部委托给已证明内核 `font_stack_count_admits`/`font_order_admits`（UI-FONT-ORDER-001，六步）。替代证据：`tests/native_text_display_fonts.rs` 3 项（回退顺序帧对照、四类拒绝与满额重复、命中随回退布局），构建虚拟机实测；不计入六阶段完成数 |
| UI-ACCESS-ACTIONS-001 | `access_tree::access_actions` 及 AccessActions 类型 | 树中节点的 focus 动作当且仅当可聚焦且未禁用，activate 动作当且仅当为按钮且未禁用（与 focus/activate 入口的准入规则一致） | proved，2 verified/0 errors（含类型），禁用仍可聚焦/任意角色可激活两反例被拒；以匹配源码的 `evidence/access-actions-kernel/` 与六阶段报告为准；可见性剔除、遍历顺序、几何与焦点投影在 `native_scene` 中，由 `tests/native_accessibility_tree.rs` 集成测试覆盖（含对每个节点逐一核对 focus/activate 入口实际结果），不在证明范围 |
| UI-ACCESS-CHANGE-001 | `access_tree::access_change` 及 AccessChange 类型 | 新树中没有：旧树中有为 Removed、否则 Absent；只在新树中为 Added；两树都有时内容相等为 Unchanged、否则 Changed（相等性只在两树都有时起作用） | proved，2 verified/0 errors（含类型），把移除判为新增/忽略内容相等两反例被拒；以匹配源码的 `evidence/access-change-kernel/` 与六阶段报告为准；树差（按节点身份查找、内容比较）、回放与从根重建顺序在 `AccessibilityTree::update_since`/`apply` 中，由 `tests/native_accessibility_update.rs` 集成测试覆盖（真实引擎多次更新逐步与跨版本回放均等于新树、各类不一致更新被拒），不在证明范围 |
| UI-ACCESS-REQUEST-001 | `access_kit::access_request_admits` 及 AccessRequest 类型 | Focus 请求当且仅当目标报告 focus 时准入，Activate 当且仅当报告 activate 时准入，Unsupported 永不准入 | proved，2 verified/0 errors（含类型），用 activate 判 Focus、准入 Unsupported 两反例被拒；以匹配源码的 `evidence/access-request-kernel/` 与六阶段报告为准；AccessKit 动作映射、目标查找、角色/节点/TreeUpdate 转换与到 focus/activate 的路由在 `access_kit.rs` 其余部分，由 `tests/native_accesskit_bridge.rs` 对照参考 AccessKit 消费端的集成测试覆盖，不在证明范围 |
| UI-FRAME-DAMAGE-001 | `frame_schedule::frame_damage` | 同尺寸两帧的损伤恰为全部不同像素的最小包围矩形；无上一帧、尺寸或长度不符为 Full；逐像素一致为 Unchanged | Verus：N/A——切片分块迭代器与闭包查找循环不在当前 Verus 可验子集内，未改写为可验形式；替代证据：Proptest（5×5 以内任意改动、错宽、缺上一帧、短帧）与 Kani（2×2、2×1、1×2 全部像素子集与任意字节）均对照独立穷举包围盒规格，另有 Miri/fuzz/Loom；不计入六阶段完成数 |
| UI-SURFACE-VIEW-001 | `Rgba8SurfaceView` 构造、row、width/height/stride | 构造充要条件、精确存储输入布局与字节视图；元数据/行访问一致；行传输精确且失败目标不变；字段私有 | 以当前源码匹配的 `evidence/surface-view-kernel/` 与六阶段报告为准；外部资源权限未涵盖 |
| UI-SURFACE-WRITE-001 | `surface.rs::write_surface_row` 及只读准入依赖 | 成功精确写入有效行，保留其他行/padding/长度；失败目标完全不变；准入充要条件明确 | 以匹配源码的 `evidence/surface-write-kernel/` 和全链报告为准；不涵盖跨进程原子发布 |
| UI-SURFACE-BORROW-001 | `surface.rs::surface_row` 与 surface_span | 整体跨度有效且 row 合法才返回精确有效行切片，不含 padding；非法输入拒绝，无越界 | 精确规格与实现映射已登记，结果以当前源码匹配的 `evidence/surface-borrow-kernel/` 和全链报告为准；不覆盖外部会话归属 |
| UI-SURFACE-SPAN-001 | `surface.rs::surface_span` | 非空几何、stride 不重叠，精确末行有效字节终点；额度/溢出拒绝，无额外末行 padding 要求 | 规格与源码映射已登记；结果以匹配源码的 `evidence/surface-span-kernel/` 和六阶段报告为准；不含存储权限与生命周期 |
| UI-SURFACE-ROW-001 | `surface.rs::rgba8_row_bytes` | 非空 RGBA8 行精确四字节/像素、额度内才成功、无溢出；零宽与超额拒绝 | Verus 实现级证明及全链证据以匹配源码的 `evidence/surface-row-kernel/`、`evidence/validation-chain/` 为准；不是完整 Surface 验证 |
| UI-DECODE-BUDGET-001 | `UiBatch::decode` | 输入/操作/字符串/分配及解码开销满足声明额度 | not_verified；已在预分配前拒绝 op_count 超出剩余标签字节的帧，仍不是完整额度证明 |
| UI-INPUT-OWNER-001 | pointer/keyboard/host | 输入归属正确；断连/撤权取消；提交不重复消费 | not_verified；跨会话宿主尚无实现 |
| UI-TEXT-CLASS-001 | 旧 `text::is_wide/is_zero_width` | 对所有 Rust char 精确实现声明的范围表 | 以 text-class-kernel 和顺序链为准；只证明旧近似表的实现，不证明 Unicode 完整宽度、测量或字素/索引转换 |
| UI-TEXT-INDEX-001 | 后续文本编辑接口 | 字节/标量/字素簇/平台 IME 索引转换与选区合法 | not_verified；完整接口尚未实现 |
| UI-RESOURCE-LIFE-001 | 后续代际资源/宿主接口 | 旧/跨会话引用拒绝；释放/撤权后不可使用；额度保持 | not_verified；代际资源服务尚未实现 |
| UI-THEME-STATE-001 | 样式更新与 Jasper 语义接缝 | 纯风格切换保持约定的值/焦点/选区/身份/滚动锚点 | not_verified；完整语义接缝尚未实现 |
| UI-LAYOUT-HIT-001 | layout/paint/hit_test | 约定坐标/裁剪语义一致；文本命中结果合法 | not_verified；需定义浮点、字体与第三方布局边界 |

这份表是义务登记，不是已证明清单。系统内置与应用自带宿主都受约束；实际平台输入/无障碍/视觉验收仍需单独进行。

## 第一项实现级证明的执行方式

`scripts/verify-version.py` 从生产 `scene.rs` 唯一标记区域提取版本函数，保留原始可执行函数体，只向签名加入 Verus 返回值命名与后置条件；不复制另一份手写模型。使用固定 Verus 版本运行，要求实际报告非零 verified 义务及零 errors，同时检查退出码。

```
python3 scripts/verify-version.py --verus /absolute/path/to/verus
```

产物在 `evidence/version-kernel/`：生成的验证源码、完整工具输出和 report.json，记录源文件/函数/证明输入哈希、版本、命令、退出码和范围。源码变更必须重跑；单次结果不证明未来版本。错误实现（返回旧版本）另作反例，必须触发真实后置条件失败；解析错误不算有效反例。证据目录不进入发行包。

**只证明版本计算函数**，不证明 clone-on-write 事务、Rust 标准库、编译器、内核或整个 UI。信任边界包含提取/注解脚本、Verus/vstd/SMT 求解器、两种工具的 u64 语义对应与目标编译执行链。函数没有 assume/admit 或未证明的业务引理。此窄切片是建立真实证明链的起点，不是所有证明工具/架构已永久选定。

工具来源：[Verus 固定发行版](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.18.7d82994)、[对应安装说明](https://github.com/verus-lang/verus/blob/7d82994/INSTALL.md)。工具放构建环境，不能成为 UI 应用运行时依赖。

## 交付规则

- 每项记录规格、源码映射、前置/后置条件、覆盖、假设、工具/求解器、版本与可重复命令。
- 安全性与活性分开；活性必须列出公平性和宿主调度假设。
- 模型到实现必须有精化/契约义务；无法证明时明确未完成，不能以降低规格冒充原性质成立。
- 首批版本/解码/句柄/事务义务优先于复杂视觉特效；后续文字/主题/系统宿主同步提交证明。
- 单元测试、属性测试、fuzz、Miri、故障注入及真实设备验收作为补充，不能覆盖或改写证明状态。

CI 已新增独立 `version-proof` 作业，固定 Verus 发行版与 SHA-256、Rust 1.99.1，执行生产函数证明与反例拒绝并上传证据。作业配置已落地；本次未推送远端，不能宣称 GitHub Actions 已运行。

## 第二项切片：解码读取端点（2026-10-23）

同一脚本增加 `--kernel decode-range`；直接提取 `batch.rs::checked_read_end` 的原始函数体，只注解后置条件，不另写模型。此函数先检查 `pos <= total`，再检查 `len <= total - pos`，最后才计算 `pos + len`。Verus 验证所有输入上减法/加法安全，成功返回精确合法终点，失败确实对应非法范围；无额外输入前置条件。

```
python3 scripts/verify-version.py --kernel decode-range --verus /absolute/path/to/verus
```

证据目录 `evidence/decode-range-kernel/`。实际结果 1 verified/0 errors；把返回值改为 `Some(pos)` 的错误实现被同一契约拒绝。验证采用 Verus 默认 usize 模型，不宣称已做 32 位平台交叉编译或真实执行。实现层切片/游标状态、完整消息解析、资源额度和宿主隔离继续保留独立证明义务。

本轮构建虚拟机默认配置测试共 104 项通过（97 单元、1 smoke、5 布局、1 文档），含失败读取保持游标、完整文本批各截断前缀拒绝、恶意计数预分配前拒绝。版本内核因共用运行脚本变化已重验，仍 1 verified/0 errors，反例拒绝。CI 同时运行两项证明；远端运行尚未发生。

## 第三项切片：事务版本准入与节点身份检查（2026-10-23）

`Scene::apply_batch_at` 使用生产 `decide_scene_version` 决定四类版本处理结果。证明输入直接提取该枚举/函数，同时提取并验证其真实 `next_scene_version` 依赖；不把依赖当成外部已正确的假设。无输入前置条件，证明覆盖所有 current/expected u64 和 empty bool 组合：陈旧版本优先拒绝、匹配空批不推进、匹配非空批耗尽拒绝、其余精确推进一次。

```
python3 scripts/verify-version.py --kernel transaction-gate --verus /absolute/path/to/verus
```

实际结果 2 verified/0 errors，颠倒陈旧版本比较的反例被拒。记录在 `evidence/transaction-gate-kernel/`。此结论只覆盖版本决策，不覆盖补丁执行、树克隆、身份集合或最终赋值的事务精化证明。

同时修复插入仅检查子树根 ID 的缺口：新增 `try_new`、全树 `validate`，插入前检查全部后代与现有树 ID；批更新检查现有树，布局入口在建身份映射/测量文字前拒绝重复 ID。兼容 `Scene::new` 和公开 root 仍可构造非法状态；它们不是已证明的安全边界，直接 find/contains 的调用方须先验证输入。未重构为封闭的不变量类型之前，不声称任意时刻 Scene 都合法。

匹配版本的空批仍不推进版本，但现在也校验树；非法树即使空批也返回错误。陈旧版本与版本耗尽仍按版本决策优先返回。插入使用暂存树检查，故先移除再复用同一 ID 可行；不允许先插入重复 ID 再靠后续补丁消除歧义。

新增五项回归覆盖非法初始树、后代碰撞时回滚前序修改、新子树内部重复、同批先移除再插入与后代更新、直接修改 root 后布局前拒绝。默认测试 109 项通过（102 单元、1 smoke、5 布局、1 文档）。身份遍历本身为迭代实现，但其他递归布局/克隆仍需深度与额度治理；每次插入重建身份集合的成本尚待后续优化，未声称完成性能目标。

## 第四项切片：完整字节读取原语（2026-10-23）

生产 Cursor 的字节读取已统一委托 `take_bytes`，该函数包含合法范围判断、真实切片、成功位置提交，且不修改输入字节。不再仅证明一个算术终点就把切片与状态变化排除在证明外。

```
python3 scripts/verify-version.py --kernel byte-read --verus /absolute/path/to/verus
```

不设输入前置条件，按 Verus 的整数/引用/切片语义验证：

- Some：终点精确等于原位置+len，未越过原字节缓冲；返回字节序列恰为原缓冲的请求子区间，长度等于 len。
- None：位置保持原值，且起点/范围确实非法。
- 借助真实端点计算依赖的同时验证，检查读取与位置运算安全；不以手写模型替代生产函数体。

实际 2 verified/0 errors，两个生产函数的验证包括真实依赖，不能当成两项全模块证明。两个负向控制分别破坏成功推进与失败保持，均触发后置条件失败；参数、语法、类型错误不算反例成功。产物在 `evidence/byte-read-kernel/`。

本轮整理脚本为显式的内核、依赖、契约与反例表；依赖同场验证，报告记录依赖/输入源文件/函数/脚本哈希；提取失败前先撤销旧成功报告。引用规范使用固定 Verus 版本的 old/final 语义，依据 [可变引用迁移说明](https://github.com/verus-lang/verus/blob/7d82994/source/docs/migration-mut-ref.md)。vstd 的引用/切片规格与 Rust 编译/执行链仍为公开信任边界，不宣称已证明操作系统或硬件。

版本、解码端点、事务准入三项入口均已随脚本变化重验且反例被拒；默认 109 项测试通过，格式检查通过。CI 加入字节读取证明与两种反例。完整消息解析、复合字段失败时的状态策略、额度、节点唯一性与整个场景事务的证明继续保持未完成；不因本切片通过而关闭它们。

## 第五项切片：生产 Cursor::take（2026-10-23）

继续将 Cursor 本身纳入证明输入：从生产文件原样提取 Cursor 结构、take 方法和 BatchError 类型，同时验证真实 take_bytes/checked_read_end。原方法以显式 match 将 Some/None 转成 Ok/Truncated，消除此前未单独证明的错误映射边界。

```
python3 scripts/verify-version.py --kernel cursor-read --verus /absolute/path/to/verus
```

无输入前置条件；成功返回字节序列恰为原缓冲请求区间、长度恰为 len，位置精确推进；失败仅在范围非法时发生，位置不变且错误恰为 Truncated；缓冲的字节视图保持。规格证明字节序列、长度和位置，不额外声称已证明底层指针地址同一性或硬件内存实现。

实际返回 4 verified/0 errors，计数含依赖/派生实现检查，不代表四个模块或四项独立大性质。把合法读取改成错误、把 Truncated 改成 BadMagic 的两种错误方法体，均被后置条件拒绝。证据见 `evidence/cursor-read-kernel/`。

工具对生产 BatchError 的派生 Clone 发出“未生成规格”的警告，原始日志保留，未隐藏/抑制；此次读取路径不调用 Clone，故不依赖其契约，也不宣称已证明 BatchError::clone。vstd/求解器仍在明确的可信计算基内。

本轮全部五个入口重新运行、各自反例均被拒，源码/依赖/验证输入/脚本哈希一致；报告增记实际 Z3 4.16.0 版本与显式求解器覆盖环境。默认测试仍为 109 项通过，格式检查通过；未为纯重构重复新增实现镜像测试。CI 已纳入五个入口，远端执行仍未发生。

尚未闭合：u8/u32/浮点/字符串等字段解码的完整语义、组合读取失败时的状态策略、整批协议解析与预算、节点身份唯一性及整个场景事务、布局/绘制/输入/文字/宿主生命周期。Cursor::take 的完成不能替代这些项目。

## 第六、七项切片：整数解码与完整帧头（2026-10-23）

生产 `u8/u16/u32` 和 `decode_header` 均原样提取、带真实依赖同场验证，不以独立手写解析模型替代实现。

```
python3 scripts/verify-version.py --kernel scalar-read --verus /absolute/path/to/verus
python3 scripts/verify-version.py --kernel header-read --verus /absolute/path/to/verus
```

### 整数字段

证明 u8 是原位置字节，u16/u32 分别按 256 的幂权重得到精确小端值；成功推进 1/2/4 字节，失败只在范围不足/非法时发生，保持位置并返回 Truncated。加法、乘法及索引的安全义务同时验证，没有只证明结果而省略溢出。

固定 Verus 版本不直接支持 `u32::from_le_bytes`。没有添加 `assume_specification`，也没有给本项目函数使用 external_body；生产 u16/u32 改为两/四字节的显式有界权重表达，直接验证真实函数体。原生标准库转换作为独立运行测试对照，而非证明公理。浮点仍使用原有路径，尚无完整语义证明。

### 帧头

`UiBatch::decode` 在建立操作数组前调用 `decode_header`；其证明覆盖原始十字节帧头、字节读取依赖和真实协议常量。无输入前置条件，按既定顺序区分：

1. 不足十字节 → Truncated。
2. 魔数不符 → BadMagic。
3. 版本不符 → UnsupportedVersion，携带正确的原版本数值。
4. 操作数超过 MAX_OPS → TooLarge。
5. 操作数超过剩余载荷字节数 → Truncated。
6. 其他情况返回精确操作数；它不超过 MAX_OPS 和剩余载荷字节数。

魔数/版本/计数的 spec 函数是声明性规格，非另写的运行时解析器；没有未证明的业务引理。错误计数、把版本错误改报魔数错误两种实现均被后置条件拒绝。完整帧头通过不保证操作标签/绘制参数/字符串载荷合法，尾部字节与未知标签仍须主解码器检查。

本轮构建虚拟机格式检查通过，默认 115 项测试通过（108 单元、1 smoke、5 布局、1 文档）。新增 u16 全域 65,536 个值的原生转换对照、u32 每字节通道变化及边界对照、u8 全值、短整数失败状态、帧头错误优先级、合法帧头但非法载荷等回归。测试不是替代证明；u32 运行测试没有穷举全部 2^32 个值。

七个证明入口全部按最终源文件重验，各自反例被拒；结果与源码/依赖/脚本哈希一致。派生 Clone 警告仍保留、不进入读取路径的证明前提。当前尚未完成的是浮点/颜色/点/矩形/字符串载荷、完整操作循环及协议预算、节点唯一性、整个场景事务与其他 UI 子系统；不报告为全部已证明。

## 第八项切片：长度前缀有界载荷（2026-10-23）

字符串的字节读取已委托生产 `Cursor::bounded_bytes(max_len)`，先使用已证明的 u32 读取长度，再检查上限，最后借用对应字节；它不创建 Vec/String。原有 UTF-8 转换仍在该借用读取成功后执行，不把标准库转换假定成已完成的本项目证明。

```
python3 scripts/verify-version.py --kernel bounded-payload --verus /absolute/path/to/verus
```

无输入前置条件，验证真实函数体和全部读取依赖，准确区分以下状态：

- 长度前缀不足：Truncated，位置保持原值。
- 完整长度超过 max_len：TooLarge，只消费四字节前缀；即使实际载荷同时不足，也优先报超限。
- 前缀完整、长度未超限但载荷不足：Truncated，只消费前缀，未消费部分载荷。
- 成功：返回原缓冲中前缀之后的精确字节序列，长度等于前缀值且不超上限，位置精确增加 4+长度；空载荷也有定义。

这保持既有字符串错误状态，不强行把复合字段的失败策略改成“所有情况回退到字段起点”。主解码器在错误后丢弃其局部游标，不把失败字段继续解释为成功；完整操作循环的精化仍待证明。

实际 Verus 8 verified/0 errors，包含真实依赖和派生检查计数，不是八项独立模块证明。绕过 max_len、错误地读取零长度两个反例均被后置条件拒绝。证据见 `evidence/bounded-payload-kernel/`，派生 Clone 无规格警告继续保留且不作为当前路径假设。

构建虚拟机默认 118 项测试通过（111 单元、1 smoke、5 布局、1 文档），含偏移前缀、精确边界、零长度、缺前缀、超限与缺载荷优先级、u32 最大长度、非法 UTF-8 的既有错误状态回归。UTF-8 正确性仅有运行测试，不能标为已形式化证明。

八个入口均已用最终源码重验，反例全部被拒；源码/依赖/脚本哈希一致。CI 纳入新入口，远端尚未执行。待证明范围仍包括 UTF-8 转换/分配、浮点及几何载荷、完整操作解析/总体预算、节点唯一性与场景事务等。

## 六阶段入口首轮实测（2026-10-23）

- 现有八个 Verus + Z3 内核入口重验通过，所有负向控制被拒；范围仍是已登记内核，整个引擎形式化证明未完成。
- 新增三组 Proptest 通过（每组默认 256 cases）；Miri 专用两项检查通过，nightly-2026-10-17 的 Miri 组件已在构建虚拟机安装。
- 默认完整测试 123 项通过（111 单元、2 内存路径、3 属性、1 smoke、5 布局、1 文档）；格式及 diff 空白检查通过。
- 顺序入口到 cargo-fuzz 时按未实现非零退出，Kani/Loom 保持 not_run；未将已有工具安装状态冒充执行通过。实际报告和日志见 evidence/validation-chain/。
- 下一工作包：生产解码 fuzz 入口、种子/回归与预算；生产实现映射的 Kani harness；逐模块 Loom 适用性登记。整体证明缺口仍按前文登记推进。

## 原样实现的六阶段验证映射与范围

`verification/read-boundary/build.rs` 从生产 `batch.rs` 的 DECODE RANGE/BYTE READ、`surface.rs` 的 SURFACE ROW 唯一标记原样提取函数；不替换运行体、不手写一份算法。验证专用 crate 只提供调用适配与独立宽整数规格；不成为 UI 运行时依赖。Verus 同样提取这些区域并仅添加契约。提取脚本、适配、oracle、工具链及其依赖仍为信任边界，所有对应源文件/锁文件在报告中有哈希，归档 `production.rs` 可与生产区域核对。

| 阶段 | 当前切片范围与限制 |
|---|---|
| Verus + Z3 | 端点和精确切片/位置保持的既有证明；新增行长在所有 u32 宽度及 Verus usize 语义下精确成功/拒绝，无运算溢出；两种错误行长实现均被后置条件拒绝 |
| Proptest | 三组内核属性（端点、切片、行长）及三组完整批解码属性；每组 256 cases，可 shrink/保留反例；非穷举证明 |
| Miri | 原样内核的边界/溢出/空切片/失败保持与行额度；完整批解码字符串分配/截断/生命周期；非无限输入证明 |
| cargo-fuzz | `read_boundary` 对三个 64 位值与尾部字节缓冲变异，含行长检查；`batch_decode` 对成功帧做位模式往返，含 NaN；两个目标各 20 秒，输入上限 4096 字节、单次超时 5 秒、RSS 上限 1024 MiB，种子 12345。短期 fuzz 不是穷举或性能基准 |
| Kani | 十五个 harness：既有读取/场景版本/Surface，另含全 RGB 通道及 0..2 像素长度转换。具体数据/几何范围与展开界限见切片记录，保留展开断言；不把有界维度说成全域 |
| Loom | 实际纯函数、共享只读四字节缓冲、独立游标/额度的双线程调度；没有生产锁或系统服务可供验证，不声称系统并发协议已完成 |

新行长接口只返回数值，不进行分配；现有 Canvas 构造/resize 尚未接入新的完整 Surface 校验。本轮已增加高度/stride/总跨度的数值检查，但真实存储权限、代际/会话句柄及释放仍未闭合。这里没有引入 winit、POSIX、GPU 或系统字体要求。

工具依据：[Kani 用法](https://model-checking.github.io/kani/usage.html)、[Kani 安装](https://model-checking.github.io/kani/install-guide.html)、[Loom 原语与限制](https://docs.rs/loom/0.7.2/loom/)、[cargo-fuzz 使用](https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html)。工具版本与实际结果以日志、锁文件及报告为准；未运行的跨平台与 feature 矩阵不得视为通过。

本轮 Kani 首次纳入新 Surface 函数时发现旧 OUT_DIR 的提取缓存没有更新，编译失败；该次未记成功。顺序入口随后为 Kani 设置由全部源码/锁文件/验证脚本哈希决定的独立 CARGO_TARGET_DIR，并在同一次 Kani 调用中验证三个 harness，避免沿用不同源码的旧提取产物。最终结果须以修复后完整重跑报告为准。


## 本轮最终结果（2026-10-23）

新增 UI-SURFACE-ROW-001 与既有端点/字节切片两个函数，均完成六阶段限定范围验收；`evidence/validation-chain/report.json` 的 slice_complete=true、complete=false，全部源码与阶段日志哈希在宿主再次核对一致。

- 九个 Verus 入口重验，负向控制均被拒；新增行长 1 verified/0 errors。
- Proptest 两组 crate 共六个属性（各 256 cases）、Miri 三个检查、Kani 三个 harness、Loom 一个双线程场景通过。
- 两个 libFuzzer 目标最终分别执行 6,525,763、2,229,811 次，各运行 21 秒（配置 20 秒上限，由工具循环收尾）；未发现反例。不将这个数值当作性能验收或完整状态空间证明。
- 默认引擎测试 123 项通过；验证 crate 另有三个属性、一个 Miri 边界检查和一个 Loom 场景。格式与 diff 空白检查通过。CI 仅配置，尚未远端运行。
- 完整 UiBatch 仅有补充属性/内存/fuzz 测试，未获得全解码器 Verus/Kani/Loom 证明；版本准入等其余既有入口也不能因本次而声称已有六阶段覆盖。


## 2026-10-24：Surface 数值跨度与旧版本准入补证

新增 `surface_span` 规格：在行长/高度均正、stride 不小于行长时，成功当且仅当数学整数 `(height - 1) * stride + row_bytes` 不超容量，返回精确终点；否则拒绝。没有引入分配、系统调用或 GPU，未把数值容量当成真实存储权限。末行只要求有效字节，不要求末行 padding；因此支持带 padding 的宿主 Surface。

本轮原样提取区域扩展到 Surface SPAN 和既有场景 VERSION/TRANSACTION GATE。旧代码补证覆盖 next_scene_version/decide_scene_version，而不是整个 clone/apply/commit 事务。Proptest 新增版本及跨度两个属性：跨度有全 usize 随机值和有意构造的恰好足够/少一字节用例；版本同时生成陈旧与匹配版本。随机种子显式固定并写入阶段环境报告。

Kani 对版本准入保持所有 u64/bool 输入；新跨度 harness 的 row/height/stride 均为 u8 范围、capacity 为全 usize，限制为控制乘法 SAT 成本。机器字宽边界/乘法或加法溢出有 Proptest/Miri/fuzz 用例，全域实现级规格由 Verus 覆盖，不能把 Kani 有界范围说成全域。Loom 增加实际生产函数的调用侧跨度额度、版本耗尽/陈旧拒绝路径；不将纯函数的独立调用扩大为系统 Surface 共享锁或场景提交的并发证明。

`surface_span` 使用 Rust 标准 checked_mul/checked_add。其数学契约依赖固定 Verus vstd 的标准库规格，未新增本项目业务 assume 或 external_body；[对应版本的数值规格](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/std_specs/num.rs) 将这些标准库函数列为 assume_specification。此信任边界明确保留，不宣称已由本项目证明 Rust core 的 checked 运算实现；Kani/Miri/运行测试另行检查实际编译调用。

顺序入口现在自动检查 Kani 内容寻址目标目录中的全部 `out/production.rs` 哈希等于归档生产区域，缺文件/不符均失败；不再只在外部手工复核。旧阶段日志在执行前清理，避免失败时看到上轮遗留成功日志。


本轮最终实测（2026-10-24）：Surface SPAN Verus 1 verified/0 errors，两种错误实现均因后置条件失败被拒；十个证明入口均重验。完整六阶段顺序入口通过：引擎三组属性加验证 crate 五组属性（每组 256 cases，显式 seed 12345）、Miri 三项、两个 fuzz 目标（5,446,026 / 2,129,508 次；各配置 20 秒）、Kani 五个 harness、Loom 一个扩展调用侧场景。默认引擎 123 项测试和格式检查通过。宿主复核全部源码/日志/十个 Verus 报告的哈希一致，Kani 提取源哈希自动一致。

此次六阶段新交付范围为 surface_span；旧代码新增六阶段范围为 next_scene_version/decide_scene_version。现有 read-boundary 与 RGBA8 行长也重验；其余模块不据此升级状态。slice_complete=true 仅指这些切片，whole-engine complete=false。CI 已纳入新证明证据，远端尚未执行。数值检查尚未接入旧 Canvas 构造/resize，完整 Surface 内存/权限/资源生命周期仍未完成。


## 2026-10-24：真实 Surface 字节切片的只读行借用

新增 surface_row 对实际 &[u8] 完整校验声明的 Surface，然后返回指定行的有效字节。规格精确到 bytes 序列的数学子区间、长度、完整跨度及非法输入的充要条件；不会以“此行恰好可读”放过其余部分缺失的声明 Surface。

实现与 surface_span 同场 Verus 验证，导入固定 vstd 的 [lemma_mul_inequality](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/arithmetic/mul.rs)，证明合法行偏移不大于末行偏移，从而排除合法 Surface 下 checked 乘/加失败。初次没有该引理时，验证器不能推出这两个失败分支不可达，状态保留 not_verified；补充已证明的标准库定理后才允许重新验收。没有增加本项目 assume/external_body，也没有修改运行函数体去替代证明。

本轮旧代码对象为 Cursor::take：原样提取 Cursor、方法和 BatchError 类型；对精确字节、位置更新与 Truncated 错误分类增加属性、Miri、fuzz、Kani 与 Loom 调用侧检查。BatchError 派生 Clone 的 Verus 无规格警告继续保留，读取路径不调用 Clone，不能把 Clone 标成已证明。

新借用的 Kani 范围为 8 字节缓冲的任意内容、u8 行长/高度/stride、全 usize row，unwind=9 且保留展开断言；Cursor 为 4 字节任意缓冲、全 usize 起点/长度，unwind=5。全域借用性质由 Verus 证明。Loom 使用同一真实只读缓冲验证有 padding 的合法行、整体容量不足的拒绝及独立游标，仍不证明系统同步协议。缓冲的合法构造/借用生命周期依赖 Rust 编译器与引用语义；裸指针导入、IPC 资源和权限是另行未完成义务。


本轮最终实测：十一项 Verus 入口与反例控制按最终源码重验；Surface BORROW 及真实依赖 2 verified/0 errors，错起点/空切片反例被拒。固定顺序六阶段通过：Proptest 引擎三项加内核七项（各 256 cases）；Miri 三项；两个 fuzz 目标 4,380,550 / 2,887,247 次，各配置 20 秒；Kani 七个 harness；Loom 一个扩展场景。默认引擎测试 123 项和格式检查通过。

宿主复核十一份证明报告、全链全部源文件/日志哈希一致，Kani 生成源自动核对一致。此次新交付只读 surface_row，旧代码新增全链范围只含 Cursor::take（含精确 Truncated 与位置语义）；其他整数/字符串/完整解码循环不能据此声称全链闭合。slice_complete=true、整个 UI complete=false，CI 仅配置，远端未执行。


## 2026-10-24：Surface 行写入与旧整数字段补证

新增 write_surface_row 只接受与 active row 等长的源切片，在整个 Surface 和目标行合法时复制有效行。Verus 规格覆盖成功的充要条件、目标长度不变、目标行每一个字节等于对应源字节、其余所有字节完全保持、失败时整个目标序列不变。不是只证明了一个偏移计算，也不把普通内存拷贝解释成跨进程原子发布。

运行体原样提取，真实调用依赖 surface_row/surface_span 同场验证。固定 vstd 的 [copy_from_slice 标准库规格](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/std_specs/slice.rs) 是显式信任边界（assume_specification），没有新增本项目 assume/external_body。标准 copy 本身未被本项目作全域数学证明；实际编译路径由 Miri、Kani 与运行检查补充。Verus 对量词触发器的提示保留在原始日志，未静默抑制。

本轮旧代码补证为 Cursor 的 u8/u16/u32：原样提取 SCALAR READ，独立以逐字节移位构造小端值，检查精确位置推进及截断错误/位置保持；不是重写一份同权重表达替代生产体。原有 Verus 全域数值/溢出/位置证明仍按当前源码重验。

新增 Kani 行写入范围：8 字节任意目标、2 字节任意源、u8 几何、全 usize row，unwind=10；逐个检查目标全部字节的更新/保持。新增 scalar harness 为 8 字节任意内容和全 usize 起点，依次检查 u8/u16/u32，unwind=10。源长度与缓冲长度的无限范围仍由 Verus 规格覆盖，不夸大有限 Kani 范围。

Loom 扩展为两个独立可写输出的真实 row-copy 调用，并读取共同只读输入；检查不同输出行更新与原始共享缓冲不串扰。生产函数没有共享可变状态或同步原语，因此该阶段仅证明限定调用侧调度/隔离，不是锁、共享 Surface 服务或 compositor 发布协议的证明。非法别名/外部资源的权限和可变借用构造依赖宿主兑现 Rust 引用前提。


本轮最终实测：十二项 Verus 入口按最终源码与验证脚本重验，反例控制全部被拒；新增 Surface WRITE 与依赖 3 verified/0 errors（不是三项独立新模块）。固定顺序六阶段通过：Proptest 引擎三项加内核九项（各 256 cases）、Miri 三项、fuzz 两目标 2,067,266 / 2,813,322 次（各配置 20 秒）、Kani 九个 harness、Loom 一个扩展场景。默认引擎 123 项测试和格式检查通过。

当前源码/十二份证明报告/阶段日志哈希由宿主再次核对一致，Kani 生成源自动匹配。此次新交付为 write_surface_row；旧代码新补全链为 u8/u16/u32，其他未闭合项不升级状态。CI 已配置，未远端执行；whole-engine complete=false。


## 2026-10-24：受检只读 RGBA8 视图与旧帧头补证

新增 Rgba8SurfaceView 将输入字节与 width/height/stride、有效行长封装在私有字段中，仅受检构造器可创建。构造、三项元数据访问器及 row 方法全部纳入实现级证明；对象与行借用均不分配、不复制，不引入窗口/GPU/系统字体依赖。私有字段没有为了通过证明而公开。

Verus 的公共契约不能直接引用不透明类型的私有字段。最初因此被拒，未记为成功；现使用 public closed spec 的 ghost model 暴露逻辑上的字节序列/布局观察值，函数入口仅添加擦除的 reveal(model) 证明语句。运行语句与数据字段原样提取，没有替代算法或业务假设。ghost model 的实现、契约/reveal 注解脚本均有哈希，属于可审计映射/信任边界。生产 Rust 中没有新增 ghost 运行依赖。

旧代码本轮对象为完整 decode_header：原样加入协议常量与 header 函数，独立 oracle 使用标准小端转换和 v1 协议诊断顺序。覆盖短头、魔数、版本、超 MAX_OPS、计数与剩余字节及精确成功计数；不扩展为操作载荷/整个解码器/完整分配预算已证明。

新增 Kani：header 为任意 16 字节数组与长度 0..16（harness 显式 len<=16，只界定有限模型，不是生产函数前置条件）；view 为任意 16 字节数组、u8 width/height/stride 和全 usize row，均 unwind=17 并保留展开断言。全域规格由 Verus 覆盖。Miri 检查带 padding 的行、少一字节整体拒绝、极大宽度，以及视图对象释放后仍存活于输入切片的行借用。Loom 增加真实视图与帧头函数的隔离调用，没有虚构系统锁或 IPC 状态机。


本轮最终实测：十三项 Verus 入口最终重验、负向控制均被拒。新视图的五个方法与真实依赖合计 8 verified/0 errors（不是八个新模块）；错存宽度和错误行号的实现被后置条件拒绝。六阶段通过：Proptest 三项批解码加十一项内核属性（各 256 cases）、Miri 三项、fuzz 两目标 1,959,711 / 2,705,960 次（各配置 20 秒）、Kani 十一个 harness、Loom 扩展场景。默认引擎 123 项测试和格式检查通过。

宿主复核十三份证明报告、当前源文件与阶段日志哈希全部一致，Kani 实际生成源自动核对一致。新交付为 Rgba8SurfaceView，旧代码新增六阶段范围为 decode_header；操作循环/完整预算不随之关闭。CI 仅配置，未远端运行；whole-engine complete=false。

## UI Rust 源模块义务清册（2026-10-24 基线）

此表登记未闭合义务，不是验收通过表。Rust 编译/引用语义和已声明 vstd 规格是基础信任边界，不能由此推断业务实现正确。新增/变更模块须更新具体规格与证据；未列出的性质仍未完成。

| 模块 | 当前义务/边界 |
|---|---|
| `src/backend.rs` | resize/submit/present 的失败状态、资源生命周期及软件输出契约；完整证明未完成 |
| `src/batch.rs` | 完整解码/分配额度/浮点与 UTF-8；已列内核与 header 见单独证据，不能替代整体证明 |
| `src/canvas.rs` | Canvas 操作、命中、坐标/裁剪语义与资源限额；未形式化证明 |
| `src/canvas_tiny_skia.rs` | Canvas 到 tiny-skia 的语义映射/错误回退；第三方光栅实现需独立信任界限，未整体证明 |
| `src/desktop.rs` | 像素转换循环见 UI-PIXEL-CONVERT-001；其余窗口/输入/提交生命周期及 winit/softbuffer/平台调用未验证，不用于强制系统内置宿主 |
| `src/engine.rs` | 场景到布局/绘制/提交的组合与失败保持；未形式化证明 |
| `src/geometry.rs` | 浮点有效域、边界、相交和坐标运算；NaN/Infinity 策略尚待精化证明 |
| `src/gesture.rs` | 指针身份/时间阈值/取消与手势状态转换；未形式化证明 |
| `src/gpu_wgpu.rs` | GPU 枚举/选择桥接和外部调用边界；不代表 GPU 渲染已实现或验证 |
| `src/graphics.rs` | 适配器选择优先级、确定性降级与能力报告；未形式化证明 |
| `src/hit_test.rs` | 坐标/裁剪/可见性/Z 顺序与命中一致；未形式化证明 |
| `src/input.rs` | 事件归属、指针状态与取消；未形式化证明 |
| `src/keyboard.rs` | 两项角色分类有切片规格； 焦点/按键/文本/IME 归属和取消；未形式化证明 |
| `src/layout.rs` | 节点唯一性、坐标/布局/测量映射；Taffy 及字体测量不能整体假定正确 |
| `src/lib.rs` | 公开导出与 feature 组合接口；仅默认/所列验证配置已运行，完整平台矩阵未完成 |
| `src/paint.rs` | 显示列表顺序、裁剪平衡、文字/图元与样式语义；未形式化证明 |
| `src/platform.rs` | 宿主尺寸/DPI/重绘/光标契约与计数边界；未形式化证明 |
| `src/pointer.rs` | 捕获/悬停/路径/事件归属和失效取消；未形式化证明 |
| `src/resource.rs` | 单槽准入/退役与字节所有权方法有精确规格；真实资源存储、认证会话、全局唯一注册表、在途取消及并发发布仍未证明 |
| `src/raster.rs` | 所有权导入、三项既有访问器与 `pixel` 读取有切片证据；Canvas 分配、写入侧像素索引、裁剪、混色及旧 new/resize 仍未证明 |
| `src/scene.rs` | 整树 ID、clone/apply/commit 原子性；版本与准入切片见证据，整体事务未证明 |
| `src/scroll.rs` | 有限几何、范围夹取、链路传播和生命周期；未形式化证明 |
| `src/scrollbar.rs` | 滚动条几何、拖动映射与裁剪/命中一致；未形式化证明 |
| `src/semantics.rs` | Role 类型用于既有分类切片； 语义树与焦点路径/顺序/身份一致；未形式化证明 |
| `src/style.rs` | 单位/颜色/状态转换与主题保持接缝；完整主题系统未实现/证明 |
| `src/surface.rs` | 现有数值/借用/写入/视图函数见明确全链证据；外部句柄权限、代际及发布同步尚未实现/证明 |
| `src/text.rs` | 文字度量与索引/字素/IME 合法域；完整编辑索引接口未实现/证明 |
| `src/text_cosmic.rs` | cosmic-text/fontconfig/字体注入映射与信任界限；不代表文字光栅已实现或第三方已证明 |


## 2026-10-24：受检视图行传输与旧有界载荷补证

新增 Rgba8SurfaceView::copy_row_to：目标长度必须恰为 active row 长度且行合法，成功时目标序列精确等于源行，失败时目标长度和全部字节保持。目的缓冲由调用方拥有，不引入分配、外部句柄或系统依赖。修改后的视图内核六个方法全部重新证明和验证，不仅检查新增方法。

运行体仍原样提取，新增方法仅附加 closed ghost model 的 reveal 和契约；copy_from_slice 的固定 vstd 规格仍是已披露的标准库信任边界。新增跳过目的拷贝反例必须触发后置条件失败，不能以编译/语法错误算拒绝。

旧代码补证为 Cursor::bounded_bytes：无完整前缀时保持位置；完整前缀后若超 limit 优先 TooLarge（即使载荷也不足），或载荷不足时 Truncated，均只消费前缀；成功返回精确字节。没有把复合字段失败改成全部回退，也没有把借用字节解释成 UTF-8 已验证。

新增 Kani：8 字节任意缓冲、长度 0..8、全 usize 起点、全 u32 limit，unwind=9；视图传输为 16 字节任意源、8 字节任意目标、u8 几何和全 usize row，unwind=17。长度范围假设仅限定 Kani 模型，Verus 全域规格没有增加相应前置条件。Miri 检查带 padding 源的准确传输、短目标/非法行不修改，以及所有短前缀/缺载荷错误位置。Loom 检查共享只读视图向各自输出缓冲传输，保留失败隔离；并未实现/证明共享可写 Surface 的锁或 IPC 资源归属。


本轮最终实测：视图六个方法与依赖 Verus 合计 9 verified/0 errors（不是九个新模块），错误宽度/错误行/跳过传输三个负向控制均被后置条件拒绝；十三项 Verus 入口按最终源码重验。六阶段通过：Proptest 批解码三项加内核十三项（各 256 cases）、Miri 三项、fuzz 两目标 1,636,863 / 2,908,295 次（各配置 20 秒）、Kani 十三个 harness、Loom 扩展场景。默认引擎 123 项测试与格式检查通过。

宿主复核源码/所有证明报告/阶段日志哈希一致，实际 Kani 提取源自动匹配。新增/修改视图方法全链闭合，旧 bounded_bytes 本轮补齐全链；这不证明 UTF-8、浮点、操作循环或整体 UI。CI 仅配置、远端未运行，whole-engine complete=false。


## 2026-10-24：整段软件呈现像素转换循环

新增宿主无关 rgb8_word 以有界算术产生数值 0x00RRGGBB，精确等于既有位通道语义。旧 desktop::rgba_to_u32 改为该核心函数与显式 while；长度检查先比较比例再乘四，检查自身不溢出，错误前没有任何写入。保留空输入/空输出成功、错误不改输出、忽略 alpha 的既有语义。

不是仅证明打包 helper：生产转换函数整体及真实依赖同场验证，循环不变量覆盖已写前缀的每个 RGB、长度关系和索引范围，decreases 证明终止。源码运行语句原样提取；生成输入只加入契约与循环不变量/终止度量，不重写算法、不使用业务 assume/external_body。wrong-success/reject-empty 两种实现必须触发后置条件失败，不能用工具错误冒充反例拒绝。

Proptest/Kani oracle 使用位移/OR 而非生产乘法表达；对完整输出和错误保持逐项比较。Kani 新增全 u8 RGB 通道、两像素模型（任意八字节输入与两个任意 u32 输出，输入/输出长度分别 0..8/0..2，unwind=9）；全长度循环性质由 Verus 证明。Miri 对所有小长度组合、颜色边界和不同 alpha 的等价性检查，Loom 用同一实际转换函数处理各自独占输出；不声称 GUI 系统锁或呈现原子性已证明。

直接 native desktop 五项原有测试已在构建虚拟机通过；顺序入口将其放入 Proptest 阶段作为附加集成检查，因此提取验证之外也编译真实 desktop 模块绑定。CI 仅 Linux 测试环境安装桌面构建依赖，不把它们加到核心或系统内置宿主的强制依赖。

本轮还补齐 fuzz 证据追踪：fuzz 阶段保留实际初始语料快照及逐文件哈希（不只记录仓库 seeds），崩溃 artifact_prefix 指向 evidence/validation-chain/fuzz-artifacts 的各目标子目录，确保失败输入随 VM evidence 回传。保留固定随机 seed、完整命令和终态语料；fuzz 仍不是穷举证明或性能基准。


本轮最终实测：十五个 Verus 入口按最终源码重验，反例全部被拒；新 pack 1 verified/0 errors，旧转换循环加依赖/派生检查 4 verified/0 errors（不是四个新模块）。完整顺序六阶段通过：Proptest 批解码三项与内核十五项（每组 256 cases），并实际编译 desktop 特性和通过其五项原有测试；Miri 三项、fuzz 两目标 843,003 / 2,788,620 次（各配置 20 秒）、Kani 十五个 harness、Loom 扩展隔离场景。默认引擎 123 项测试通过。

宿主再次核对十五份证明报告、所有输入源/阶段日志/Kani 生成源哈希一致。两个 fuzz 初始语料快照分别 87 / 306 个文件，逐一匹配记录哈希；潜在崩溃输入会直接保存在可回传的 evidence 路径。本次未发现反例。CI 配置已同步，但远端未执行；实际窗口呈现/输入生命周期以及整体 UI 仍未闭合。

## 下一切片准入规格：Canvas 分配与 resize（2026-10-24，待实现）

本轮核查原有十五个入口对应的源码、六阶段日志及 Kani 实际提取源码哈希均匹配；审计快照为 `evidence/canvas-next-slice-audit.json`。这只是证据完整性复核，没有新增实现级证明，也没有重跑六阶段。

`raster::Canvas::new` 仍直接计算 `width as usize * height as usize * 4` 并分配；`HeadlessBackend::resize` 仍经同一路径重建。已证明的 Surface 行/跨度校验尚未接入这两处，不能据其推断 Canvas 分配安全。下一切片必须处理完整入口及错误传播，不能仅加一个不被调用的尺寸工具后宣称修复。

精确反例用任意精度整数计算，不申请巨量内存：64 位下 width=height=2^31 时字节数为 2^64，超出 usize；width=2^31、height=2^30 时字节数为 2^63，虽未超 usize，但超出 Vec<u8> 的 isize::MAX 容量限制。32 位下 width=height=2^16 同样超出 usize。这些是现有表达式的算术风险见证，不是已执行的分配/跨平台测试，也不声称可造成 Rust 内存不安全。

待实现规格（Rust 内部切片设计，不冻结 Jasper 语言 API）：

- 显式区分几何非法、宿主预算不足、表示/分配器容量超限和实际分配失败；成功字节数严格等于数学整数 4×width×height，且不超过宿主预算、usize::MAX 和 isize::MAX。旧 new 的零尺寸归一化行为需明确兼容；新的受检入口不得隐式混用归一化与拒绝策略。
- 先完成全部数值准入，再尝试分配。预算充足不代表分配必定成功；分配器成功/失败是明确环境边界，不允许公理化“总能分配”。无需依赖系统字体、GPU、窗口或 POSIX。
- 受检 resize 成功才替换旧存储/尺寸；任何准入或分配失败保持旧像素、尺寸和裁剪状态。现有 RenderBackend::resize 无返回值，错误传播契约尚未实现；不能仅吞掉错误或 panic 后标记验收。
- Verus 先证明真实计数实现，再处理 Vec 分配/零初始化/状态替换的标准库规格与实现映射；遇到库规格不支持则维持未完成。Proptest 覆盖零/边界/预算，Miri 覆盖小缓冲生命周期，fuzz 严格限制实际分配预算，Kani 记录小内存界限与全域算术差别。Loom 若仍无生产并发路径，登记调用侧隔离适用性，不冒充呈现同步证明。

本轮没有修改上述生产实现；Canvas 与后端 resize 的证明义务仍为 not_verified。性能与大尺寸吞吐优化继续延后，数值安全和失败保持属于当前正确性门槛。

## Canvas 所有权导入：从数值校验接入真实光栅对象（2026-10-24）

新增 `Canvas::from_rgba8` 接管调用方合法拥有的 Vec；复用真实行/跨度检查并要求精确 packed 长度。它是现有软件绘图对象的真实构造入口，无像素复制/分配；不是继续增加未接入的 Surface 数值工具。旧 `new` 的实际分配、resize 的错误传播仍保留未完成，未通过业务假设绕过分配证明。

生产 Canvas/Point/Size/Rect 的类型定义原样提取，不用虚构的字段替身。只证明导入与 width/height/pixels 三个既有访问器，不证明浮点几何方法、派生 Clone、绘图或旧分配入口。私有字段由 closed ghost model/reveal 描述，运行体原样保留；注解另加入两条经 nonlinear_arith 实际证明的乘法等式/单调关系，没有新增业务 assume 或 external_body。初次规格 nat/int 类型不匹配、后续缺少乘法引理时均未计通过；修复证明注解，没有降低准入条件。

规格覆盖所有 u32 宽高与合法 Vec：成功当且仅当非零尺寸且原长度精确等于 4×width×height；成功存储原字节和尺寸、空裁剪栈；失败返还原字节。Rust Vec 移动、引用/切片、Vec::new/len/视图及现有 checked 运算的 vstd 标准库规格属于信任边界，不声称本项目证明了分配器。运行验证另检查成功与失败均保留原存储指针/容量；该地址身份检查不是新增的 Verus 指针契约。保留“不支持非 Copy Clone 的自动规格”工具警告，本入口没有调用 Clone。

六阶段使用同一提取生产代码：新增属性覆盖任意宽高、随机缓冲及合法小布局/短一字节；Miri 同时执行真实 crate API（包括导入后 clear 的兼容性测试，不把它算 clear 的证明）；fuzz 对不超过既有输入上限的字节 Vec 进行完整尺寸和合法单行检查。Kani 新 harness 为 16 字节任意内容、长度 0..16、全 u32 宽高，unwind=17；不声称穷举所有存储长度。Loom 在实际导入入口上检查共享只读源、各自拥有 Vec 的成功/失败隔离，没有生产同步原语，不能扩张为跨进程存储/发布证明。

本切片完整顺序链已实际通过：十六个 Verus 入口及负控制最终重验，新导入/三访问器与依赖/类型检查为 12 verified、0 errors；错宽度、丢弃错误缓冲的实现被后置条件拒绝。属性集十六项×256 cases，另保留根批解码三项及真实 Canvas 两项、desktop 五项检查；Miri 五项（根读取二、提取内核一、真实 Canvas 二）；Kani 十六 harness；Loom 一项。默认引擎全部 125 项及格式检查通过。两 fuzz 目标各配置 20 秒，实际 772963/2457417 runs，无反例，非性能基准。源码/十六份证明报告/阶段日志/Kani 生成源映射哈希一致；CI 同步配置但未远端执行。

## 第三方暂用与 Rust 替换政策（2026-10-24 用户裁决）

功能开发优先复用 Chromium 生态可独立集成的成熟 UI 部件；第三方内部作为显式未证明依赖登记，允许先集成推进，不宣称整个 UI 已证明。Canvas 停止继续自研光栅化，优先 Skia 的 Rust 接入方向。自有新增/修改 Rust 接入以及后续从上到下的替换仍执行完整六阶段门槛。性能优化继续延后；自有正确性失败不能以第三方信任边界名义放行。版本、许可、宿主适用性、接口与未证明范围须可追溯。

## 非 Canvas 主线：UI 资源槽与旧键盘分类

新增 ResourceGate 的四个生产方法原样提取，closed model 保留私有状态；没有业务 assume/external_body。四方法与派生检查为 5 verified/0 errors，重复存活签发与错误代数两个负控制被后置条件拒绝。单槽身份匹配和代数不回绕有精确规格，宿主认证、唯一 gate 生命周期和实际资源销毁/在途取消仍是独立义务，详见 resource-admission.md。

同轮补旧 Role::is_activatable 与 activates_with_space 两个分类器，原样提取 Role 和两个运行体；3 verified/0 errors，错误 Link/Space MenuItem 分类被拒。不把分类函数扩张为完整键盘、焦点或无障碍接入已证明。

属性/Miri/fuzz/Kani 用同一生产提取映射；资源 oracle 使用独立宽整数/状态元组。审查时修正了测试 oracle 把任意 key 都误认成旧代 key 的错误：可预测的未来身份恰与新代相等时，属于当前匹配 key；不靠 key 保密保证权限。首次运行已主动中止并保留未完成记录，未计通过，正式顺序链须从头重跑。Loom 只调用无内部并发的真实资源状态机，验证独立会话调用隔离；未构造虚假生产锁，不证明注册表或共享存储同步。

最终从头执行的十八入口完整顺序链通过：属性集十八项×256 cases，根批三项/Canvas 两项/资源两项与 desktop 五项保持通过；Miri 七项；Kani 十八 harness（新资源为全 u64 身份/代数、bool 存活状态、任意 key 的连续实际方法调用；角色为全 u8 映射到全部角色，unwind=18）；Loom 一项。默认引擎 127 项和格式检查通过。两 fuzz 各配置 20 秒，实际 773413/2560367 runs，无反例，不作性能基准。十八份证明报告、源/驱动/阶段日志及 Kani 实际生成源映射哈希一致。CI 同步入口和证据上传，未远端运行。此轮未扩展 Canvas，既有图像检查仅作为基线重跑。

## 实际字节资源与旧修饰键谓词

UiBytesSlot 将此前单槽 gate 接入实际 Vec 所有权。四个生产方法及 gate 同场验证，9 verified/0 errors；错误空存储安装与错 caller 读取反例拒绝。closed 模型维护私有状态，new 建立 payload/live 一致性，其他方法在这一已建立的不变量下保持它，没有业务 assume/external_body。不把单槽生命周期扩张为宿主认证、全局唯一注册表或异步在途取消。Vec 移动、Option/as_ref/take 与切片语义属于固定 vstd/Rust 标准库信任边界。

同轮为既有 no_command_modifier 补证，原样提取 KeyEvent/KeyPhase/Modifiers 与运行体；精确证明 Control/Alt/Meta 使该谓词为 false、Shift 不影响该谓词。字符串字段没有参与谓词，KeyEvent 的非 Copy Clone 自动规格警告保留，未调用 Clone。最初负控制定位同时命中契约与运行体，脚本拒绝生成成功报告；改为精确唯一运行体定位，不把定位错误算有效负控制。

属性与 Miri 检查安装失败/重复安装不覆盖当前内容、错误会话/槽位/旧代读取和释放拒绝、原字节/存储身份保留、合法借用后释放/重装；运行存储地址检查不是 Verus 新增的指针契约。Kani 新字节资源 harness 使用 8 字节任意内容、长度 0..8、全 u64 session/slot/caller，unwind=17；任意长度由实现级证明承担。Loom 共享实际 UiBytesSlot 的只读访问，调用线程结束且唯一拥有者取回后再 release，记录调用侧适用性，不冒充生产锁/IPC 同步证明。

最终二十入口顺序全链通过：新字节资源/真实 gate 合计 9 verified/0 errors；旧命令修饰键谓词及派生检查 4 verified/0 errors，负控制均被真实后置条件拒绝。属性集二十项×256，Miri 九项，Kani 二十 harness，Loom 一项；真实字节资源 API 两项亦加入顺序入口。默认引擎 129 项、desktop 五项及格式检查通过。两个 fuzz 目标各配置 20 秒，实际 667998/2423170 runs，无反例，非性能基准。二十份证明报告、源码/驱动/日志及 Kani 实际提取源全部匹配；CI 已同步未远端执行。未扩展 Canvas 专项；UiBytesSlot 尚未接上字体解析、系统资源服务或 IPC。

## 旧文本范围分类与字体注入实验的分界

旧 is_wide/is_zero_width 的真实函数体直接提取，以独立逻辑比较规格验证全部 Rust char，2 verified/0 errors；遗漏宽字符末段与补充变体选择符范围的负控制被拒。该规格只描述原有近似范围表，既没有证明 SimpleTextEngine 浮点测量，也没有证明表符合完整 Unicode 宽度/字素规范，不替代 UI-TEXT-INDEX-001。属性/Miri/fuzz/Kani 的独立十进制区间表和边界字符检查继续属于补充，不是数学证明的替代。

同轮复用 UiBytesSlot 在隔离 Skia 场景实验中向 custom_empty 字体管理器显式提供内存字体并绘制 Latin 标题；这一 native 功能预检独立记录，不计为自有正式适配层的六阶段完成项。Skia Data 复制/字体解析/native FFI 的未证明边界保留，未把 Rust 槽位所有权证明扩张为字体安全或排版正确。Canvas 不增加专项功能，正式默认后端尚未切换。

最终二十一个入口顺序链全部通过：旧文本分类器 2 verified/0 errors，两个负控制均拒绝；属性二十一项×256、Miri 九项、Kani 二十一 harness、Loom 一项。新 Kani 采用任意合法 Rust char、unwind=10，不将无效 UTF-32 数值算作合法字符。默认 129 项、desktop 五项及格式检查通过，fuzz 各配置 20 秒实际 892290/2857452 runs，无反例，非性能基准。源码/二十一报告/驱动/日志/Kani 生成映射一致；CI 同步未远端执行。字体 native 实验另有源快照、输入字体及版权/SHA、像素对照与日志，仍未验收为正式后端。

## 宿主退役与旧焦点清理

ResourceGate/UiBytesSlot 增加永久 retire 方法；修改后两个内核全部方法重新验证，不仅新函数。退役把 gate 置为耗尽/非存活并取回原字节，之后不能重新签发或安装；不把它扩张为全局会话关闭、在途取消或 native 销毁。新负控制分别尝试重置退役代数及略过 gate 退役，必须由后置条件拒绝。

同轮补旧 FocusManager::blur/order/focused 三方法：真实 NodeId 和 FocusManager 类型原样提取；精确清除焦点并保持原遍历序列/返回视图。Verus 5 verified/0 errors，错误清除与错误顺序视图负控制拒绝；不证明构造 focusables、focus_next/prev、KeyboardRouter 事件或取消通知。非 Copy Clone 自动规格警告保留，本切片未调用 Clone。属性/Miri/fuzz 对任意顺序（含重复/焦点不在顺序中）检查总行为；Kani 记录有限 Vec 长度，任意长度由 Verus 证明。Loom 只调用真实无内部并发方法，不虚构生产锁。

焦点 Kani 初轮未通过：旧测试构造/对照使用动态 iterator/多次 Vec collect，CBMC 报内存耗尽；逐元素对照的第二次诊断仍失败，均未计通过，初轮整链在 Kani 停止且没有进入 Loom。失败报告/生产提取/测试源码与诊断日志保留在 evidence/focus-kani-initial-oom。第三次诊断改为预分配四个 NodeId 的确定容量缓冲后 truncate 至 0..4，再调用相同真实 getter/blur；逻辑输入仍是四个任意 u64 身份、长度 0..4、任意 Option<u64> 焦点，unwind=17 不变，未缩减该逻辑域、未 stub 生产实现或跳过断言。该固定容量模型与原构造的分配布局不同，明确不声称覆盖任意 allocator capacity；任意 Vec 的逻辑性质由 Verus 承担。诊断通过后必须从头跑完整顺序链，不能用单 harness 成功覆盖原整链失败。

最终重新从头执行的二十二入口顺序链通过：ResourceGate 五方法与派生为 6 verified/0 errors，UiBytesSlot 五方法加 gate 为 11 verified/0 errors，旧焦点三方法及派生为 5 verified/0 errors；全部负控制拒绝，特别包含退役后重置代数/略过退役。属性二十二项×256、Miri 九项、Kani 二十二 harness、Loom 一项通过。默认129项、desktop五项、格式检查通过；fuzz 各20秒实际561307/2638065 runs，无反例，不作性能基准。当前源码/二十二份报告/驱动/日志及 Kani 实际提取映射一致；CI同步未远端执行。初轮 Kani 失败的整链没有被覆盖为成功，已另存失败快照；新完整运行独立记录成功。

### UI-KEYBOARD-FOCUS-001：真实路由器焦点取回

新增 keyboard-focus 提取真实 KeyboardRouter 存储和 focused/blur/take_focus 三方法，依赖已证明的 FocusManager 存储、getter 与 blur；不使用替代路由实现或业务 assume。私有规格把路由器映射为原遍历序列与可选焦点：take_focus 精确返回调用前目标，调用后焦点为空且完整顺序不变；原两个委托同时补证。丢失返回目标、略过清理两个负控制必须触发后置条件失败。未覆盖 refresh、focus、route、文本/组合路由或整会话取消。

属性输入包含任意 u64 节点、None/Some 焦点、重复节点与目标不在顺序中；生产构造可达性是更小集合，规格不依赖这些前提。Kani 固定分配四元素后截断至逻辑长度 0..4，覆盖全 u64 节点和可选焦点；任意长顺序由 Verus 规格承担。Miri/fuzz 与 Loom 直接调用同一提取方法；Loom 仅验证线程各自拥有的真实路由器，生产无锁，不冒充跨线程共享路由或 IPC 取消证明。指针不变由运行断言补充，非新增 Verus 指针契约。

2026-10-24 本切片已完成完整顺序重跑：依赖与派生共 Verus 9 verified/0 errors，两项负控制均因后置条件失败被拒；全链二十三个 Verus 入口、二十三属性×256、Miri 九项、二十三 Kani harness、Loom 一项通过。两 fuzz 目标各 20 秒，实际 484957/2526933 runs，不代表穷尽或性能达标。二十三报告与当前源码/驱动哈希一致，六阶段日志哈希一致，Kani 实际生成体与归档 production.rs 一致；全引擎 complete 仍为 false。KeyboardRouter/FocusManager 的非 Copy Clone 规格警告保留，所验三方法未调用 Clone。

### UI-INPUT-RETIRE-001 与 UI-FOCUS-SELECT-001

input-retire 原样提取新宿主关闭方法，组合真实 gate.retire 与 router.take_focus，约束旧焦点精确返还、焦点清空、遍历序列不变，以及 gate 保留会话/槽位并设为 MAX/非存活。略过退役和丢弃原焦点的负控制必须被拒。归属配对、每个输入入口强制准入、IME/指针拥有者与取消投递不在此规格之内；当前 API 不强制这些调用方条件，因此不能升级为完整输入隔离或会话关闭证明。

同轮补证 FocusManager::focus：仅当请求节点在原遍历顺序内时设置焦点，否则精确返回 NotFocusable(请求节点) 并保持所有状态，成功也保留顺序。最初原 contains 写法未能证明后置条件，证据保留在 evidence/focus-select-initial-unproved/，未计通过。随后把生产检查改成相同成员语义的显式 u64 ID 扫描；原样提取实际循环，以已检查前缀不含目标、全序列/原焦点保持、下标上界及递减量建立任意长度规格。不增加业务 assume 或替代执行体，不据当前错误日志臆测 contains 的具体求解失败原因。

运行对照使用原生切片 contains（与生产显式扫描不同），覆盖请求在表内及不在表内、空表、重复 ID、最大值、原焦点不在表内。Kani 仍采用固定容量4后截断至逻辑长度0..4的实际 Vec，全部ID/请求/原焦点元数据任意；input-retire 的 gate 会话、槽位、代数、存活状态任意。非 Copy Clone 规格警告保留，所验方法不调用 Clone。Loom 只对线程分别拥有的实际 gate/router 执行关闭步骤，不引入生产不存在的锁，不声称共享路由或 IPC 并发取消已证明。

2026-10-24 两切片已完整重跑验收：focus-select Verus 8 verified/0 errors，input-retire 连同真实依赖 16/0，两者各两项负控制均拒。全链二十五 Verus 入口、二十五属性×256、Miri九项、Kani二十五 harness、Loom一项均通过。fuzz各20秒实际468682/2329230 runs，仅为此配置证据；默认cargo test共131项、desktop专项5项及格式检查通过。25报告/66源码哈希/六阶段日志与Kani实际生成体映射一致；全引擎 complete=false，未以窄切片充当整体证明。

### UI-HOVER-UPDATE-001 与 UI-HOVER-RETIRE-001

原样提取真实 HoverChange、HoverTracker 存储及 hovered/update/clear，规格精确约束状态和返回序列：目标不变为空；变化时按 Leave(旧目标)→Enter(新目标)，两端缺失时不生成该端事件；clear 等于退至 None。生产旧 Option<NodeId> 等值分支首次未能建立后置条件，保留 evidence/hover-update-initial-unproved/，不计通过；之后改为 None/Some 显式分支和 u64 字段比较，运行语义保持，未对派生相等性补业务 assume。未据失败日志断言其具体内部原因。

新增 retire_hover 组合真实 gate.retire 和 clear，永久关闭准入、精确取回原目标 Leave 并清空状态；错误事件种类/略过状态更新/略过准入关闭/丢弃 Leave 四个负控制必须被拒。不证明宿主配对、全入口准入、事件投递、capture/press/gesture/IME 取消。HoverTracker::new/Default/Clone 没有纳入此三方法补证，不声称整个模块已证明；派生非 Copy Clone 警告保留，所验方法未调用 Clone。

独立运行情况表覆盖 old/target 的 None/Some、相同/不同及精确事件顺序，再调用真实退役检查 gate 永久拒绝、悬停清空和连续重复关闭无事件；Kani 任意 Option<u64> 两目标和全 u64 gate 元数据/布尔状态，unwind=10；Miri 与 fuzz 调用同一提取实现。Loom 在线程各自持有的真实 tracker/gate 上执行，无新增生产锁，不声称共享可变路由或 IPC 取消已验证。Vec 分配/释放及 Rust 标准库规格属于现有 TCB，未证明全局资源额度或分配成功性。

2026-10-24 最终完整顺序重跑通过：hover-update连同类型/派生 6 verified/0 errors，hover-retire及真实依赖 13/0，各两负控制均因后置条件失败被拒。全链27个Verus入口、26项属性×256、Miri9项、26个Kani harness及Loom1项通过（一个运行生命周期入口同时覆盖两个新增证明入口）。fuzz各20秒实际459672/2706147 runs；默认cargo test共132项、desktop专项5项及格式检查通过。27报告、66源码哈希、六阶段日志和Kani真实生成体/归档映射一致；全引擎complete=false。

### 指针 map 取消路径的证明预检（未完成）

在修改真实 PointerRouter/PointerTracker 前，对固定 Verus 版本做了 BTreeMap 查找规格可用性对照：primitive-probe 使用 u64 key，1 verified/0 errors；pointer-id-probe 原样提取生产 PointerId 及派生，查找后置条件未证明（1 verified/1 error）。源码/命令/工具版本/日志哈希位于 evidence/pointer-map-feasibility/。两份仅是隔离可行性探针，均未加入生产代码或完整六阶段交付，不增加已验收入口数；失败不能计为通过，也不能解释成实际 map 查找行为错误。

固定版本的 [vstd BTreeMap 规格](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/std_specs/btree.rs) 明确要求 key 满足比较规格；[比较规律](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/laws_cmp.rs) 将相等、偏序与全序的一致性纳入条件。下一步须为真实 PointerId 比较实现建立对应规格与规律，再在真实 capture/tracker 上证明删除、清空和原目标取消意图；不能给自有 key 加未经证明的 obeys_cmp 假设，或以仅 u64 的模型冒充 PointerId 生产映射。此预检不宣称已经确认唯一失败原因，也不宣称工具整体不支持 BTreeMap。

后续隔离候选已验证可行：以相同 u64 身份表示、显式 PartialEq/PartialOrd/Ord 执行体及对应 spec extension，建立比较规律后实际 BTreeMap::contains_key 后置条件通过，6 verified/0 errors；破坏运行 eq/cmp 的两负控制分别以 5 verified/1 error 被拒。没有对自有比较体使用 assume_specification/external_body。spec extension 参照固定版本 [vstd 数值类型映射](https://github.com/verus-lang/verus/blob/7d82994/source/vstd/std_specs/num.rs)，候选独立证明其自身运行实现和规律，不把标准库的受信规格写成自有代码已证明。

完整候选、两个负控制及实际 Verus/Z3 版本、命令、源/日志哈希见 evidence/pointer-map-feasibility/manual-report.json。生产 PointerId 仍是原派生实现，尚未替换；不能把候选成功用于证明当前生产 map。Proptest/Miri/fuzz/Kani/Loom 均未给该候选完成验收，production_integration_complete=false、six_stage_complete=false；已验收入口数保持27。下一步在真实生产提取链中迁移比较实现及补齐全链，再接真实捕获取消。

### UI-POINTER-ORDER-001 与 UI-CAPTURE-QUERY-001

PointerId 生产派生相等/偏序/全序改为同一 u64 身份的显式实现，保留 Copy/Clone/Debug/Default/Eq/Hash；证明体从实际源码提取，spec extension 与比较规律一起验证，不给自有比较方法加 assume。两负控制破坏实际 eq/cmp，必须实际后置条件失败。先前 manual-report 仍是历史隔离候选，不能覆盖本次正式提取与完整六阶段要求。

新 captured_target 与旧 has_capture 使用真实 PointerRouter.captures BTreeMap，返回精确成员关系和保存的原目标；实际 PointerRouter、Capture、PointerTracker、PointerEvent 及其字段类型一并提取，未用替代 map 或空路由器模型冒充生产布局。其他旧 tracker/route/命中函数未纳入证明。BTreeMap 标准库规格和其 key 比较前提属于显式 TCB/证明义务，未声称证明 std 内部树算法或整个指针服务。派生非 Copy Clone 规格警告保留，查询不调用 Clone。

属性/Miri/fuzz 对实际比较和 map 查询作独立 u64 情况对照，覆盖重复 key 覆盖、缺失 key、空 map 和捕获目标为 None；Kani 比较为任意三个u64（unwind17）；查询使用真实空map/固定键7的单键map，查询ID及可选目标为任意u64（unwind5）。两键/重复键的任意元数据仍由Proptest/Miri/fuzz覆盖；双键Kani实验未通过，不能把单键结果当作该实验已完成。实现级规格不限制 map 大小，有限模型不代替任意 map 的 Verus 结论。Loom 使用各线程独立拥有的实际 router/map，无生产共享可变锁，不声称取消/IPC 并发已完成。

本次捕获查询全链尚未完成。初轮Kani以unwind33构造两个任意u64键的实际map时显著膨胀，被明确停止，未计通过；unwind17诊断也被停止；unwind5和数组构造unwind3诊断均报告CBMC内存耗尽。失败/停止日志位于evidence/capture-query-unwind33-stopped/，没有把降低unwind本身算成功、关闭展开断言、减少两键/任意u64逻辑输入域或以模型替代实际map。

当前诊断只对Kani夹具使用mem::forget，排除查询完成后的std map析构：被验证的两个生产查询都只接受&self、返回bool/Copy目标，不拥有/销毁map；真实构造及查询仍执行，全部查询断言保留。原生、Proptest、Miri和Loom仍执行夹具析构；std析构不属于本查询契约的Kani覆盖，不能据此宣称map资源生命周期已经验证。此诊断暂未完成，不因缩小到正确的借用契约范围就自动计通过；仍须成功后从头重跑全部六阶段。

最终单键借用诊断已通过：1176检查零失败、51不可达，2.2秒；实际std map构造/两个生产查询均执行，没有stub或改写查询体。Kani夹具结束时forget仅排除查询契约之外的std析构，原生/Miri/Loom夹具无forget并真实析构。该结果是空map/单键代表布局的有限验证，不能替代任意map的Verus证明，也不能当作先前双键实验通过。需要从头执行全链后才计本次正式交付。

2026-10-24 最终重新从Verus起完整顺序执行通过：pointer-order及派生/规律 5 verified/0 errors，capture-query及真实依赖 18/0，四项负控制拒；29个Verus入口、28属性×256、Miri9、28个Kani harness、Loom1通过。fuzz各20秒实际381676/2352649 runs；默认133项、desktop专项5项及格式检查通过。

Kani查询验收范围明确为空map/固定键7的单键map、任意u64查询及任意Option<u64>目标、unwind5；比较harness仍为三个任意u64。它覆盖两个生产查询的存在/缺失/有capture但目标None等分支，绝不将前述双键失败模型计为通过；任意键/任意map的实现级数学性质仍由未减弱的Verus规格承担。双键、重复键与任意元数据的原生Proptest/Miri/fuzz保留；std map析构不属于查询的Kani覆盖。该有限模型与信任边界必须随交付保留，不能扩张为指针生命周期已证明。

当前29报告/66源码哈希/六阶段日志/Kani实际生成体映射核对一致；source-snapshot.zip归档所有66个已哈希源码及清单，逐项复核，audit.json记录快照SHA与Kani明确边界。此前失败/停止快照仍保留，未拼接成功；全引擎complete=false。

**正式验收裁定：捕获查询仍未完成。** 上述全链成功仅对应明确记录的空表/单键受限配置，不替代原先双键Kani目标，也不因此降低验收门槛。双键模型仍失败/未完成，所以不把新captured_target标记为已交付。Verus任意map证明、比较内核补证和受限全链证据保留，后续继续解决双键验证；全引擎完成状态仍为false。

## 双键捕获查询 SMT 诊断（2026-10-24 06:35 UTC 轮次）

保持原双键/重复键、任意 u64 查询及可选目标、真实 BTreeMap 和正常析构，在隔离 crate 中改用 Kani `--solver z3` 重试；unwind 5，300 秒超时退出 124。日志仍在符号执行/展开阶段，未获得完成的性质或展开断言结果，不能算通过，也不能断言 Z3 不支持此实现。工具版本、源码/日志哈希及完整配置保存在 `evidence/capture-query-smt/report.json`；生产实现和全链配置未修改。捕获查询正式验收仍未完成，受限单键诊断不替代双键目标。下一步应定位真实 std 容器构造/析构的符号展开成本，不通过关闭检查、假容器或缩小键域绕过门槛。

### 验收入口防误报（2026-10-24 06:45 UTC）

#12 未变化。本轮未修改生产 Rust；修正 verify-ui.py 的结果语义：六阶段受限检查即使全部成功，也只设置 scoped_checks_complete；原双键 Kani 义务仍登记在 acceptance_pending，slice_complete 保持 false，进程退出 2。当前强制待验项须在原目标真正通过后连同证据更新，不能因单键诊断通过自动解除。直接执行实际报告尾部的失败门禁检查通过，证据为 evidence/capture-query-smt/acceptance-gate-check.json；没有重跑六阶段，旧 report.json/source-snapshot.zip 是原脚本版本的历史证据，不能作为当前脚本的验收结果。下一实现仍需解决双键真实容器验证；Canvas 排程不变。

### 双键诊断的展开与求解阶段定位（2026-10-24 11:02 UTC）

#12 无变化，生产 Rust 与上次历史报告逐项对照未变，只有验收脚本含已记录门禁修正。继续在隔离 crate 使用原任意双键/查询/可选目标及正常析构，降低展开界至 2 只用于故障定位，没有当作验收。Z3 运行到 169122 个表达式步骤、简化后 4121 个 VCC，随后在 SSA 转 SMT2 时出现 CBMC status 6 / map::at 内部错误；相同配置换回 cadical 报内存耗尽。两次均 0 个成功 harness，不能据此宣称性质成立、失败反例成立或展开界充分。证据与源码/日志哈希见 evidence/capture-query-unwind2/report.json。此前中断的临时夹具首次重试出现无效 manifest，重新传输并核对远端源码哈希后才执行这些诊断，未改生产代码或清理其他项目缓存。

当前不能仅靠切换求解器或降低全局展开界消除问题。下一轮需针对容器构造/析构的具体循环建立有证据的逐循环界或独立等价夹具，保留任意双键和全部检查；在此之前不追加同配置长时重试、不计新增 UI 功能交付。Canvas 仍最后，不变更验证要求。


12:00 UTC 指针主线预检：当前生产 Rust 与受限历史链源码一致、#12 未变。按真实 Kani CLI 支持，以同一任意双键/查询/目标、正常析构和 unwind5，尝试仅改变 CBMC 指针对象编码位数为8；90秒仍停在库符号展开并超时，未获性质/展开断言结果，不能算通过。证据在 evidence/capture-query-object8/report.json。此结果表明不能仅凭此参数解决当前构造路径成本；不改变生产容器/输入域，不关闭检查，后续不重复同参数长时重试。取消实现及全六阶段尚未交付。


12:10 UTC 指针验证诊断：#12 未变，生产 Rust 未修改。保留任意双键/查询/可选目标和真实构造/析构，以 CBMC 数组 UF 编码与解引用缓存重试 unwind2 诊断，仍在命题规约阶段内存耗尽，0 harness 成功。未关闭展开断言、未用该界冒充充分界、未缩小键域；日志/配置/源哈希在 evidence/capture-query-arrays-uf/report.json。已查固定安装版本的 BoundedArbitrary BTreeMap 实现，同样通过有界 native insert 构造，没有证据它能消除成本，不再无依据重复换生成器。后续需要逐循环/构造路径定位或保持语义的真实实现调整后重新全链，不能把诊断当作输入取消功能完成。


12:20 UTC 定位进展：#12 未变。隔离对照进一步剥离 UI：两个任意 u64 key、任意查询、任意 Option<u64> 值的原生 BTreeMap 插入/查询/正常析构仍报内存耗尽；值附带空 Vec 的对照同样失败。改用连续 Vec 只做同域查询（保留嵌套空 Vec、正常析构）则 Kani 0/260失败、0.303秒通过。证据和两版源快照见 evidence/capture-shape-control/report.json。对照不证明生产 PointerRouter，也不证明完整 map 插入/删除/次序等价，原双键义务保持未完成。

下一工作包据此转向评估实际生产捕获存储的可证明表示，而非继续调相同 BTreeMap 求解参数。只有真实实现迁移、保持重复 key 覆盖/原目标/路径/数量及取消语义、且原任意双键功能目标在实际新实现上完成全六阶段后才可验收；旧实现失败仍保留，不用 Vec 玩具模型为未改动的 BTreeMap 冒领通过。性能变化登记后续优化，不降低正确性目标。本轮未改生产 Rust、未新增已验收功能。UI 主线→Canvas→jasper-os 的最新顺序保持。


12:31 UTC 捕获存储候选：#12 未变。开始真实可迁移表示的候选实现 experiments/capture-slots/slots.rs，暂未导出到生产 crate。原样提取当前 NodeId/Capture 类型，运行体只增加证明注解，constructor/len/first-match position/borrowed get 得到 Verus 7 verified/0 errors（含类型依赖），错误索引反例 6/1 后置条件拒绝；无业务 assume/external_body。继承 Capture 的 Clone 无规格警告保留，未调用或计为已证明。候选源码、生成体、固定工具版本、日志与哈希保存在 evidence/capture-slots-candidate/report.json。

这仍不是存储迁移完成：插入覆盖、删除、次序/唯一性、生产路由集成以及后五阶段尚未完成；原双键功能目标和原生产 BTreeMap 未通过状态保持，不用候选部分证明给当前生产实现冒领通过。下一步补候选写入/删除及同域完整验证，再决定真实迁移。未新增正式 Rust 交付，未接 OS、未扩 Canvas。


12:41 UTC 捕获存储候选写入/删除：#12 未变，生产 Rust 未改。候选新增 insert/remove，精确证明重复 key 返回原 Capture 并只更新对应条目、新 key 追加、删除返回原值/其余序列保持、缺失删除不变；此前构造/计数/借用查询仍通过。当前 Verus 9 verified/0 errors（含类型依赖），错误索引/错误覆盖返回/错误删除位置三反例均8/1后置条件拒绝。std::mem::replace 原方案因工具缺规格未采用，未添加业务 assume；采用真实 Vec remove+insert，首次序列等价后置条件未证，随后仅添加擦除的外延相等证明闭合，规格未减弱，两次失败快照保留。证据在 evidence/capture-slots-candidate/report.json。

仍属未导出候选：唯一性/完整 map 等价闭合、生产迁移、后五阶段与原双键目标都未完成；不把9个验证计数冒充新增交付或当前 BTreeMap 已通过。下一步对照真实候选运行体建立完整链与唯一性，再进行真实迁移；排程不变。


12:57 UTC 捕获存储候选唯一性闭合：internal #13 暂无评论，生产 Rust 未改。对真实候选运行体增加擦除的唯一键规格与三个序列引理，证明空构造建立唯一性、同键覆盖/新键追加/删除均保持唯一性，同时保留原有精确序列与原 Capture 返回值后置条件。固定 Verus/Z3 实测 12 verified/0 errors（包含三个纯证明引理与类型依赖）；错误查询索引、错误覆盖返回、错误删除位置均以后置条件错误被拒绝。没有加入业务 assume 或 external_body。最初 private open spec 可见性错误已修正，未减弱性质；继承 Clone 缺规格警告仍保留。当前证据与源码/生成器/日志哈希见 evidence/capture-slots-candidate/report.json，上一版9/0快照保留在 evidence/capture-slots-before-uniqueness。

该结果只闭合候选可达状态的唯一性，尚未完成显式 map 抽象对应、生产迁移和后五阶段。原生产 BTreeMap 的任意双键 Kani 义务仍未通过；没有新增正式交付。下一步继续候选实际多步操作的行为对照与全链验证，再接入真实路由/取消生命周期，不把候选证明数量视为功能完成率。UI 主线→Canvas→jasper-os 的顺序保持。


13:11 UTC 捕获候选后续验证：#12 未变、#13 暂无评论。新增独立 verification/capture-slots 验证 crate，直接 include 真实候选运行体并从生产标记提取 NodeId/Capture；未迁移生产 BTreeMap。依次完成两项 Proptest 各256例（任意双键/重复键/查询/删除与多步 BTreeMap 行为对照，含目标和非空路径）、1项定向 Miri（边界/重复覆盖/删除/正常析构）、cargo-fuzz 361823次/21秒，无发现失败。源、锁文件、工具配置、日志及快照在 evidence/capture-slots-chain/report.json；首次中断的模糊测试不计通过，Miri 日志丢失后已在最终 fuzz 前重跑，未用缺失证据冒充成功。

Kani 仍未通过：真实候选任意双键/查询/可选目标、覆盖返回/删除/另一键保持/正常析构，unwind5，默认 cadical 在命题规约时耗尽内存；相同范围 Z3 到 SSA→SMT2 时 CBMC status6/map::at，均0成功harness。未关闭检查、未缩键域、未跳过 Drop、未换成模型容器。Loom 因前序失败未进入；候选本身无并发、仅私有 Vec 与独占可变借用，其适用性依据已写入 crate README，生产宿主共享路径以后另审。新验证入口仅为候选实验，不算生产新增交付或六阶段通过。下一步须定位真实 Vec 插入/删除/析构中的 Kani 成本与工具故障，保留原功能目标；不能因为测试成功消除该门禁。Canvas 与 jasper-os 排程不变。


13:21 UTC 评审与候选 Kani 定位：#13 新增公共文本/输入基础设施下沉评审（issuecomment-6384584040），已在 docs/input-method-boundary.md 记录为长期设计输入；UI 作为公共协议消费者，不接管终端/公共服务/内核实现，不冻结候选 API 或更改排程。Scintilla/Lexilla/Keyman/Rime 的最终退出与 Skia 可替换但另行评估已分开登记。

保持真实捕获候选运行体不变，把原双键完整功能目标拆为查询/覆盖与删除/另一键保持两个 Kani harness，并移除仅为断言构造的临时期望 Capture。两个均保留任意 u64 key/query/可选目标、真实 Vec 构造/查询/删除及正常析构，无假容器、无缩键域、无 Drop 跳过。符号表达式从合并目标91366步降至删除38642/查询31294，但两个仍在求解时耗尽内存，0/2成功；查询出现中间 SAT 输出但未完成性质报告，不据此宣称业务反例或充分展开。当前诊断快照/哈希在 evidence/capture-slots-partitioned/report.json；原完整链报告为其原源码的历史证据，不用于给变更后的 harness 冒领通过。生产 Rust、功能门槛与未验收状态不变；不再重复同拆分配置长时求解。

## 桌面资源保护（用户于 2026-10-24 明确要求）

所有重型编译、Verus/Z3、Proptest、Miri、cargo-fuzz、Kani/Loom 继续只在 Jasper 构建虚拟机运行，禁止在宿主机回退执行。新增统一入口 `scripts/verify-in-vm.sh <command> [args...]`：仅对 UI 验证串行加锁，虚拟机内以 systemd 用户 scope 限制整棵进程树 MemoryMax=3 GiB、MemorySwapMax=0、TasksMax=128、Cargo jobs=1、180秒超时及10秒终止宽限。不调整共享虚拟机配置或其他项目队列，也不因验证内存不足自动提额；资源不足记为未完成。

轻量资源门禁预检在虚拟机内读取对应 cgroup，实际 memory.max=3221225472、memory.swap.max=0，证明上限生效；未为验证门禁而启动重型测试。检查时宿主机与虚拟机均无本线残留求解/编译进程。后续不得直接使用无此 scope 限额的旧验证命令进行重型运行。虚拟机仍消耗物理宿主内存，本限制用于控制本线峰值，不声称虚拟机意味着零宿主影响。


13:41 UTC 受限 VM 验证：#13 仍为已记录的同一条评审评论。此前 raw CBMC 诊断输出指向验证夹具 value() 的 std in_place_collect 路径，不能据此断言生产缺陷或完整反例成立。本轮仅将夹具 Vec<u64>→Vec<NodeId> collect 改成逐元素 push，保留所有路径值，候选运行体与生产代码未改。通过新的受限 VM 入口按序重跑：Proptest两项各256例及定向测试通过、Miri一项通过、10秒 fuzz完成；随后任意双键 query 的 Kani 仍在命题规约阶段超时，整体退出124，未获完成性质报告。未重跑删除harness或Loom，不算全链完成。

新证据/源快照/哈希见 evidence/capture-harness-loop/report.json；执行硬限额3GiB、swap0、jobs1、180秒，无宿主重型运行或自动扩额。退出后检查虚拟机无残留编译/求解进程，可用内存约7.3GiB。当前受限资源下仍未验收，后续不重复相同 query 配置长时重试；下一步须分离实际构造/查询/析构支持边界或调整有证明支撑的真实实现，不以夹具改动抹去原双键目标。


13:51 UTC 指针主线补证：#13 无新增评论；不重复运行已超时的 Kani 配置。本轮为真实候选方法建立双键组合 Verus 入口，实际调用 new→insert(a,x)→insert(b,y)→get(q)，键/查询完整u64，Capture目标和路径任意。精确证明首次空返回、重复键覆盖返回存在性、条目数1/2、q匹配后者优先并返回对应原目标，否则缺失。基于既有实现级精确序列契约，通过擦除的存在见证与索引实例化完成，不改变运行体、不添加业务 assume/external_body。VM限额内 Verus13/0，错误目标负例12/1后置条件拒绝；初次未证明版本保留。证据、源码快照及哈希在 evidence/capture-composition/report.json。

这是新增组合形式化证据，不是用 Verus 代替 Kani：Kani双键门槛仍未完成，完整map抽象/删除组合/生产迁移仍待做，生产 BTreeMap不因此算通过。未新增已验收功能、未扩Canvas或OS，不因补证数量宣称总体完成率。后续继续生命周期主线，并针对实际工具支持边界选择可验证实现，避免无依据长时求解与扩大内存。


14:01 UTC 验证夹具边界定位：#13 同一评审评论无新增。仅将 Kani 输入构造改为直接 Capture{target,path:Vec::new()}，彻底排除路径转换 helper；实际候选运行体、完整u64双键/查询/可选目标、重复覆盖及正常析构仍保留。3GiB受限 VM 内60秒诊断仍退出124，21495符号表达式步、336简化VCC后未完成命题规约。证据见 evidence/capture-direct-fixture/report.json；无成功harness、无充分展开结论，不用fixture改动宣称生产或候选通过。此方向不继续盲目换参数重跑。

后续工作拆分：捕获存储迁移保留未验收门禁，但不让这项旧代码补证单独冻结其他UI功能；转向可独立验收的 UI 输入目标准入/焦点文档版本绑定，复用现有已验证 ResourceGate 的会话/槽位/代数机制，避免再造同类 gate。新切片仍须完整六阶段后才能交付；不提前宣称捕获取消、迟到输入隔离或完整生命周期完成，不扩展到公共文本/输入服务或 OS。恢复捕获存储工作前需要有具体实现/工具支持定位依据，而非继续同形求解。


## 用户排程裁决：暂缓捕获存储 Kani 诊断（2026-10-24）

用户明确倾向跳过当前未通过的诊断，待功能完成后的整体绞杀阶段再做。本线据此暂停当前捕获存储双键 Kani 及其同形参数/夹具重试，保留全部历史失败、超时和源码证据，状态为 deferred_unverified，不计通过或删除义务。未验收的 Vec 捕获存储候选不迁入生产；继续使用现有 BTreeMap 捕获实现，相关旧代码证明欠账和信任边界保持显式。

该延期仅针对当前捕获存储诊断/补证工作包，不代表普遍取消新增/修改 Rust 的 Verus+Z3→Proptest→Miri→cargo-fuzz→Kani→Loom 门槛。UI 输入归属、焦点/文档版本、生命周期等可独立切片继续推进；不能因此宣称捕获取消或整体框架完成。确需修改旧捕获实现时，单独登记变更范围、原有未证明依赖及新代码验证，不用该延期掩盖新增正确性问题。

恢复条件：UI 功能闭环后的整体替换/绞杀阶段，或用户明确重新安排；届时先建立源匹配的原任意双键/查询/可选目标、正常析构目标，再在受限 VM 内执行，不通过缩键域、跳过析构或模型替代降低验收要求。Canvas→jasper-os 的后置顺序、UI Rust 范围和 VM3GiB资源约束均保持。

## UI-INPUT-IDENTITY-001：独立输入身份准入切片（2026-10-24 14:11 UTC）

新增 src/input_identity.rs 并导出模块：可信当前 UI 目标/代数/文档/版本与票据逐维匹配，同时调用原 ResourceGate 的会话/槽位/代数准入，关闭后拒绝旧票据，重新签发也不恢复旧代数。函数不分发事件，不自行管理宿主身份或取消外部任务。新函数完整输入域规格 Verus+Z3 10/0（含既有 gate 与类型依赖），忽略 gate/文档版本两项负例均拒绝。初次负例定位模式同时命中规格与运行体，runner 拒绝生成结果，已修正为唯一运行体模式后完整重跑，未降低性质。

按序实跑 Proptest256例并逐一翻转八维身份、Miri20组边界生命周期组合、10秒 cargo-fuzz、有完整u64上下文和u8错配掩码的 Kani（1 harness，0/125失败）。最终 Loom 源码适用性审查为不适用：该纯谓词只读取不变标量/独占 gate，实际状态转换需 &mut，无线程/原子/内部可变性/队列或同步路径；不构造假并发模型，后续异步宿主接入单独审查。重验了旧 ResourceGate 的真实 issue→revoke→reissue→retire 组合路径；不宣称所有旧代码已补齐。

完整证据、锁文件、各阶段日志、工具/配置/范围和源码快照见 evidence/ui-input-identity-chain/report.json，形式化细节在 evidence/ui-input-admission/report.json。集成后无默认特性113项库测试通过；所有重型步骤只在VM内，3GiB/swap0/jobs1，未重启用户已延期的捕获存储诊断。此切片验收不等于六类入口均已接入、完整捕获取消/生命周期或整个UI已完成。

## UI-INPUT-BINDING-001：持久 UI 绑定（2026-10-24 14:21 UTC）

新增 UiInputBinding 五方法，持有既有 ResourceGate 和当前上下文：空闲绑定/拒绝不变、逐维准入、精确关闭/旧票据不能关闭后继、永久退役。规格覆盖完整输入/状态值；Verus15/0含依赖，两负例（丢失发布上下文、忽略关闭上下文）均拒绝。初次公共函数规格引用私有 spec 可见性失败已保留；仅修正 ghost 函数可见性后重跑，不改变业务规格或代码。没有业务 assume/external_body。

依次执行属性256例、Miri边界生命周期、10秒65字节种子 fuzz、Kani两harness成功，最后审查真实同步实现的 Loom 适用性为不适用（独占 &mut、无异步队列/回调/原子/内部可变性）；不制造空并发模型。源码/锁文件/完整日志/哈希在 evidence/ui-input-binding-chain/report.json，形式化证据在 evidence/ui-input-binding/report.json。补强旧 ResourceGate 的真实组合不变性：拒绝绑定或关闭不改变租约，关闭重开后旧票据无权关闭新绑定，退役不可重新签发。无默认特性集成库113测试通过，全部重活在受限VM，未重试已延期捕获存储诊断。

这是同步单绑定对象的发布与失效契约，不是完整宿主并发协议或外部任务取消；认证、全局身份不复用、焦点路由集成、真实在途取消仍待做。旧 UiInputAdmission 报告为其当时源码的历史快照；当前新增状态机有本轮完整证据，不拿旧整文件哈希充当新方法证据。

## UI-INPUT-PAYLOAD-001：借用负载准入桥（2026-10-24 14:31 UTC）

新增 AdmittedUiInput 私有字段与payload/target读取、UiInputBinding::admit泛型桥：成功精确关联原负载与已准入目标，失败拒绝，泛型T不暴露第三方/编辑器私有结构。Verus18/0含依赖，绕过准入负例拒绝。首轮公共规格访问私有字段的 opaque 错误已保留，改用闭合 ghost model accessor，未开放生产字段或减弱契约。

依次完成属性256例、Miri边界借用生命周期、10秒65字节种子fuzz、Kani三个harness通过（0/233、0/168、0/125失败），最终Loom源审查仍为无并发实现而不适用。额外根集成测试使用六类真实InputEvent负载并检验旧票据/关闭/后继准入；不是声称所有旧路由器已接通。外部Rust编译负例对“仍在消费已准入负载时retire”精确返回E0502，保留日志；生命周期正确性依赖Rust借用检查/编译器，与Verus值规格的边界明确分开。全部重活仅在3GiB受限VM执行，未重跑延期捕获诊断。

证据/源码快照/锁文件与哈希：evidence/ui-input-payload-chain/report.json；形式化细节：evidence/ui-input-payload/report.json。补强旧ResourceGate/绑定组合及既有规范化事件的准入回归。真正的焦点/路由消费者接入、事件内容校验、异步队列与外部任务取消仍未完成；宿主管理关闭不能强制走已关闭的客户输入门禁。

## UI-KEYBOARD-ADMISSION-001：实际键盘消费者与绑定关闭（2026-10-24 14:41 UTC）

真实KeyboardRouter新增route_admitted和retire_input_binding：按当前焦点核对准入目标、返还原负载借用，关闭时先禁用绑定再提取/清空焦点且保留遍历顺序。Verus28/0含旧焦点/ResourceGate等依赖，两负例拒绝；继承类型Clone缺规格警告保留，未调用或算作证明。新规格覆盖任意泛型负载、焦点状态及遍历序列。补强旧FocusManager清空/元数据与KeyboardRouter取回焦点等代码的真实关闭组合，不扩大为全路由证明。

依次完成属性256例、Miri边界、10秒fuzz、Kani4harness（0/323、0/233、0/168、0/125失败；消费者harness2项不可达检查如实保留）。Kani使用真实类型、空遍历Vec与任意焦点ID；任意序列保持由Verus覆盖，不用空Vec结果冒充全域。最终Loom仍无实际并发路径而不适用，Rust独占借用属显式可信边界。真实Scene/SemanticsTree/KeyboardRouter集成测试2项通过；早期夹具with_child方法拼写错误已改成实际child并重跑，初始编译日志保留。额外编译负例确认消费返回负载期间take_focus精确E0502。

证据及源码快照/哈希：evidence/keyboard-admitted-chain/report.json，形式化细节evidence/keyboard-admitted-input/report.json。全部在3GiB受限VM；用户已延期的捕获存储诊断未运行。此新入口已经使用实际焦点消费者，但未将兼容的原始路由自动加门禁；宿主焦点转移仍须关闭/轮换绑定，实际IME/指针取消、角色/内容校验、异步队列尚待推进。

## UI-INPUT-REPLACE-001：绑定轮换/拒绝保持（2026-10-24 14:51 UTC）

UiInputBinding::replace同步组合真实accepts→close→bind；先检查代数耗尽，避免关闭MAX最后有效租约后签发失败。精确规格覆盖全部状态/输入：成功代数+1和新上下文、拒绝整个gate/current不变；旧回调不能关闭后继。Verus16/0含依赖，删除耗尽前置防护负例拒绝。下一目标是把该轮换组合进真实焦点选择，目前不声称已同步KeyboardRouter焦点。

依次通过属性256例（随机代数及强制1/MAX-1/MAX）、Miri边界、10秒种子fuzz、Kani5harness；最后Loom源码审查仍为独占同步路径不适用。边界夹具直接构造真实生产私有结构的任意代数，而非迭代2^64次或用替代模型；方法路径仍为真实实现。补充旧ResourceGate极限代数与关闭/重新签发拒绝保持验证。根集成3测试通过，新增实际CompositionEvent旧版本/新票据准入与错误所属方拒绝保持。

完整源码/日志/配置/哈希在evidence/ui-input-replace-chain/report.json，形式化细节evidence/ui-input-replace/report.json；全在3GiB受限VM，未运行延期捕获诊断。身份全局不复用、跨对象焦点提交、外部任务取消及并发宿主仍在边界之外。

## UI-KEYBOARD-TRANSFER-001：跨对象焦点/绑定转移（2026-10-24 15:01 UTC）

新增FocusManager::can_focus只读资格预检和KeyboardRouter::transfer_input_focus：先核对原焦点归属与新目标成员资格，再轮换绑定并提交焦点；成功含代数+1/新上下文与焦点，失败两个对象全部保持原状，代数MAX亦然。预检与提交之间无回调且持有独占访问，已有FocusManager::focus精确实现规格保证最终选择成功；未用assume解释忽略的Result。

资格预检Verus7/0、转移Verus30/0含依赖，拒绝成员/跳过焦点提交负例均拒绝；重新验证旧focus选择与顺序保持。依次完成属性256例、Miri0/1/MAX边界、10秒种子fuzz、Kani6harness成功；新harness用真实两项Vec<NodeId>、任意/重复键与全部u64状态，unwind3保留检查，不把该界当成任意长度证据（后者由Verus覆盖）。最终Loom真实同步独占路径不适用，异步宿主仍需独立证据。

根集成4测试通过，新增实际Scene/SemanticsTree的非法目标/错误所属方拒绝保持、焦点+文档转移、旧回调拒绝及新Text负载消费。完整快照/锁文件/日志/哈希：evidence/keyboard-transfer-chain/report.json；形式化细节：evidence/focus-eligibility/report.json与evidence/keyboard-input-transfer/report.json。全部限额VM运行，无延期捕获诊断。初始绑定发布、Scene节点代数管理、原始低层API自动门禁及实际IME/指针取消仍未完成。

## UI-KEYBOARD-OPEN-CLOSE-001：初始发布/正常关闭/重开（2026-10-24 15:11 UTC）

真实KeyboardRouter新增begin_input_focus和close_input_focus：初始焦点与空闲输入绑定配对提交，关闭精确活动归属后返还完整旧UiInputTicket作为取消意图。拒绝保持两个对象；正常关闭可在同一持久对象重开，旧回调不能关闭后继。Verus30/0含旧focus/gate依赖，省略初始焦点提交/关闭清焦两负例均拒绝，旧Clone规格警告不算已证明。资格及遍历顺序无界精确规格仍保持。

依次完成属性256例（强制0/MAX-1/MAX）、Miri0/1/MAX边界、10秒种子fuzz、Kani7harness通过；新harness使用真实私有结构、完整u64代数与两项Vec顺序，unwind3保留检查，不宣称有界顺序代替任意长度证明。最终Loom独占同步实现不适用，宿主并发/取消投递另验。根集成5测试通过，新增实际Scene的初始拒绝/发布→转移→完整旧归属关闭→目标代数重用重开→旧回调拒绝→新Composition负载消费闭环。

完整证据、源码/锁文件/日志/哈希：evidence/keyboard-open-close-chain/report.json，形式化细节evidence/keyboard-input-open-close/report.json。补强旧焦点选择/清理及ResourceGate闭环组合；仍未证明旧原始事件路由、Clone、Scene代数分配。所有重活在限额VM、延期捕获诊断未执行。返回的是身份精确的取消意图，不是外部任务已取消、回执已完成或完整编辑器/IME闭环。

## UI-COMPOSITION-IDENTITY-001：UI组合视图身份与旧取消隔离（2026-10-24 15:21 UTC）

新增UI专属UiCompositionSlot/Token，存储完整输入归属与非回绕组合序号，不实现通用输入法服务、词库、文本或像素存储。begin只为有效焦点票据且空闲槽位签发，accepts联合当前绑定和原归属/序号；cancel认证调用会话并匹配完整旧token，可在焦点已关闭后清理旧UI元数据；retire返回完整旧取消关联并永久停用。即使同一焦点票据下连续组合，旧序号也不能取消新组合。

Verus22/0含既有gate/绑定依赖，取消重置序号/忽略文档身份负例均拒绝。依次通过属性256例、Miri0/1/MAX边界、10秒65字节种子fuzz、Kani8harness；最后Loom真实局部同步状态路径不适用。有限标量harness在清理后调用retire，不把此结果冒充活动retire覆盖：任意活动retire由Verus规格证明，另有真实根集成活动retire测试。根集成共6测试使用Scene、KeyboardRouter、CompositionEvent验证同焦点重开序号、旧取消拒绝、焦点关闭拒绝晚到输出、关闭后旧token清理、新归属重开及完整退役关联。

证据/源码/日志/锁文件/哈希：evidence/ui-composition-chain/report.json；形式化细节evidence/ui-composition-identity/report.json。补强旧gate与输入绑定关闭/重开的真实组合及旧CompositionEvent负载集成，不扩大为原始IME路由/索引已证明。宿主认证与同视图槽位唯一/持久生命周期为显式信任边界，不得在旧回调存在时重置序号。元数据清理不等于外部编辑器文本已清空或原生任务已取消，输出适配/投递/回执仍待实现。所有重活仅限额VM，延期捕获诊断未运行。

## UI-COMPOSITION-RANGE-001：现有组合事件的文档字节边界（2026-10-24 15:31 UTC）

实际UiCompositionSlot新增admit_composition，联合当前归属/组合序号与CompositionEvent文档替换区间校验，返回原事件借用；拒绝UTF-8标量中间、逆序和越界，不修改文档。宿主仍负责票据文档/版本对应当前快照；这不是预编辑光标、字素编辑或原生IME接通。

按序完成Verus25/0含依赖及两语义负例拒绝、两属性各256例和边界测试、Miri短Unicode文档所有索引对、10秒种子fuzz（1863757轮）、Kani9harness全通过；新harness覆盖五固定文档与任意usize，不冒充无限Unicode证明。无限str范围由Verus规格覆盖，显式信任固定vstd的UTF-8/切片规格，不把标准库内部当成自有已证明代码。最后逐模块审查Loom不适用：此路径无真实并发实现，不造模型。根集成7测试通过，新增中文/emoji实际文档拒绝半标量、原负载消费、同焦点重开与关闭后旧结果拒绝。既有CompositionEvent边界和旧gate/绑定生命周期同步补验，原始无门控路由仍未证明。

evidence/ui-composition-range-chain/report.json保留源码快照、工具/锁文件、范围和日志哈希，形式化细节见evidence/ui-composition-range/report.json。所有重活仅限额VM；捕获双键诊断维持延期，Canvas和系统接入未提前。仍待原生编辑/IME输出与取消投递闭环。

## UI-PREEDIT-ADMISSION-001：预编辑局部坐标的独立类型（2026-10-24 15:52 UTC）

针对实际桌面契约冲突，新增独立PreeditEvent，text与可选cursor仅指预编辑文本自身；不复用CompositionEvent的文档替换坐标。admit_preedit联合完整归属/组合序号及局部UTF-8标量边界返回原负载借用，None不被伪造为全选。新类型没有派生Clone实现。此切片只为下一步真实生产端迁移建立明确消费边界，DesktopInput仍未迁移，不宣称已修复端到端IME。

Verus证明精确实际运行体及依赖，缺省光标拒绝/跳过身份两负例必须触发规格失败；自有无业务assume。标准库String::as_str与vstd UTF-8规格为显式信任边界。属性使用任意Unicode与任意usize并以str::get作为独立局部区间oracle；Miri覆盖短中文/emoji/组合字符；种子fuzz和Kani均保留旧身份、焦点、组合、文档范围harness补验。新Kani范围为三固定预编辑文本、任意usize和Some/None，不替代任意Unicode形式化证明。Loom逐模块不适用：局部独占状态与不可变文本借用，无真实线程/原子/队列/回调。

匹配源码、日志、工具配置、有限范围、最终检查结果及快照统一见evidence/ui-preedit-chain/report.json；未覆盖桌面生产转换、通用输入服务、文档编辑事务或原生取消投递。所有重活仅限额VM，延期捕获诊断未运行。

## UI-DESKTOP-IME-NORMALIZE/DELEGATE：工作树修复未完成验收（2026-10-24 16:02 UTC）

实际winit Ime四变体转换改为独立Preedit局部坐标，None/空文本保留；原方法委托并保持全状态。形式化纯转换19/0，含状态委托20/0；遗漏cursor/commit及丢弃委托结果反例分别拒绝。固定winit0.30.13枚举声明与Cargo源码逐字校验并保留哈希和Apache许可，真实桌面测试仍用实际winit依赖。旧PointerId比较性质及输入/焦点/组合harness补验保留；旧派生Clone不冒充已证明。

Proptest→Miri→种子fuzz通过后，Kani实际状态路径遇到标准库Instant::now→clock_gettime不支持，10通过/1失败，641未定。最初未设局部unwind的memcmp展开已主动停止并保留日志，随后unwind8仍保留展开断言并暴露真实时钟问题，没有降低验收。最终Loom状态not_run_pending_prior_stage；同步无并发的源码适用性分析仅为候选依据。工作树修改未交付/未提交，整体acceptance_pending，不能把纯函数有限结果替代失败状态路径。全证据与源快照evidence/desktop-ime-chain/report.json；重活仅3GiB/swap0/jobs1/180s VM，未触及延期捕获诊断。

## UI-NORMALIZED-PREEDIT-001：实际规范化输出到UI准入（2026-10-24 16:12 UTC）

独立纯宿主路径normalize_ime→实际InputEvent::Preedit→admit_normalized_preedit已接通。新方法直接消费真实规范化变体，联合当前归属/组合序号和局部UTF-8范围准入原事件借用，拒绝Text提交等其他变体；None/空预编辑保持原意，不转文档坐标。此生产API不实例化带指针时钟的DesktopInput，不修改原状态封装包的验收状态。

依次Verus46/0含实际生产转换/旧gate、绑定、组合、文档范围和PointerId依赖；忽略归属/接受非预编辑两负例拒绝。属性各256例/Miri/10秒种子fuzz通过；明确选择Kani normalized_preedit_pipeline单harness497检查通过（有限三文本、任意usize与None、unwind8保留断言），包含真实绑定关闭和组合重开。没有把未执行的旧状态时钟harness算通过：它仍在desktop-ime-chain记录unsupported与acceptance_pending。最后逐模块审查本纯路径Loom不适用，无线程/原子/队列/回调。

实际winit+Scene+KeyboardRouter根集成9测试通过，新增中文局部选区/None/半标量拒绝、提交变体拒绝、空文本清除意图、同归属新序号和关闭后迟到结果拒绝。证据evidence/normalized-preedit-chain/report.json包含源码/日志/锁文件/工具和依赖声明映射。未宣称原生视图已清除、取消任务已投递或完整IME服务完成。旧状态宿主clock_gettime问题保留未完成，本轮未重复运行该诊断，也未改内存上限。

## UI-PREEDIT-VIEW-001：实际拥有预编辑呈现文本（2026-10-24 16:22 UTC）

新增UiPreeditView/Frame，实际拥有String、cursor及完整组合token。apply从真实InputEvent::Preedit移入数据，验证失败保持整个旧帧；current只返回借用；clear在精确调用会话/归属/序号匹配时删除拥有的数据，允许绑定关闭后清理且拒绝旧组合清除后继。不是正文编辑或像素绘制，宿主须为每个目标隔离view并在关闭/取消后清理再渲染。

按序Verus50/0含实际转换、旧gate/绑定/slot及PointerId依赖；省略帧更新/忽略序号负例拒绝。属性各256例、Miri实际String替换/释放、10秒种子fuzz通过；明确单harness owned_preedit_view_lifecycle Kani612检查通过（三输入文本、任意usize与None、固定后继中文、unwind8保留断言）。最后Loom真实模块独占Option/String及借用，无并发路径，记录不适用。真实winit/Scene/KeyboardRouter根集成10测试通过，新增旧文本保持、后继替换、旧/外会话/文档取消拒绝、关闭后精确删除UI拥有文本。

证据evidence/ui-preedit-view-chain/report.json保留源快照/依赖映射/工具/有限范围/日志哈希。状态宿主时钟Kani缺口仍单独pending，未重跑、未算通过；本切片无时钟实例化。原生编辑器/像素清除、自动关闭联动、取消投递/回执与候选呈现仍待完成。所有重活仅限额VM。

## UI-PREEDIT-CANCEL-001：组合槽位与自有文本同步取消（2026-10-24 16:32 UTC）

新增真实cancel_preedit把已有slot.cancel与view.clear连成独占同步双对象操作。取消资格不通过时两对象保持原样；通过时槽位失活，只清除同一完整归属/序号的帧，保留不同关联的帧；支持绑定已关闭后的精确UI清理，不代表原生任务取消、IPC事务或像素清除。

按序Verus51/0含旧实现依赖，省略自有文本清除/取消拒绝仍清帧两反例均拒绝；属性各256例、Miri实际拥有文本释放、10秒种子fuzz通过；独立Kani paired_preedit_cancel_lifecycle 559检查通过（三文本、任意错配标志、真实私有帧错配状态、unwind8保留断言），旧gate/绑定关闭/slot重开同步补验。最终逐模块Loom不适用，无真实并发路径。实际winit/Scene/KeyboardRouter根集成11测试通过，覆盖错会话拒绝同时保持、取消清除、重开后旧取消不清后继、焦点关闭后的成对清理。

证据evidence/ui-preedit-cancel-chain/report.json保留范围/源码快照/依赖映射/工具/日志哈希。状态宿主时钟缺口仍单独pending，本轮未重复该诊断或延期捕获诊断；自动焦点关闭联动及外部投递仍待接通。全部重活只在限额VM。

## UI-KEYBOARD-PREEDIT-CLOSE-001：焦点/绑定/组合/文本四对象关闭（2026-10-24 16:42 UTC）

真实KeyboardRouter新增close_input_preedit，预检活动组合归属，再调用已有close_input_focus及cancel_preedit。拒绝时四对象保持；成功清焦点、关闭输入绑定、取消组合槽位并清除匹配UI文本，返回完整旧token。忽略内部取消bool由预检和未改变的槽位/调用方规格保证，不假定成功、不做错误后部分提交。无活动组合时应使用普通close_input_focus。

依次Verus67/0含旧实际实现依赖；省略预编辑清理/焦点拒绝前先取消两个负例拒绝。属性各256例、Miri真实String释放、10秒种子fuzz通过，独立Kani focus_preedit_close_lifecycle 689检查通过（三文本、真实双节点Vec、错会话/文档/当前焦点、关闭重开与旧关闭拒绝，unwind8保留断言）；任意焦点顺序长度和文本仍由形式化规格覆盖。最后Loom逐模块真实同步四独占借用路径不适用。实际winit/Scene/KeyboardRouter根集成12测试通过。

证据evidence/keyboard-preedit-close-chain/report.json包含快照/工具/依赖映射/有限范围/日志哈希。旧focus关闭、gate/绑定、slot取消和view清理作为依赖及真实组合补验。此为UI自有状态关闭，不宣称原生任务/IPC取消回执或像素更新；事件循环失焦/IME Disabled触发接入仍待完成。状态DesktopInput时钟Kani缺口保留pending，未重复诊断，未提高VM资源限额。

## UI-PREEDIT-LIFECYCLE-001：真实宿主禁用与失焦事件区分（2026-10-24 16:52 UTC）

真实normalize_ime不再丢弃Enabled/Disabled，新增规范化ImeEnabled(bool)。活动组合的handle_preedit_lifecycle将Focus(false)连接四对象关闭，将ImeEnabled(false)连接slot/view成对取消且保持键盘焦点及输入绑定，避免输入法禁用后普通键盘文字也失去目标；Enabled/Focus(true)/其他事件及归属拒绝保持状态。旧事件或错会话不能取消后继。

按序Verus68/0含实际生产转换与所有旧依赖；禁用时错误清焦点/忽略禁用归属两负例拒绝。属性各256例、Miri、10秒种子fuzz通过；独立Kani preedit_host_lifecycle 678检查通过（三文本、真实单节点焦点Vec、使能/禁用/重开/旧禁用拒绝/失焦关闭，unwind8保留断言），最终Loom实际同步路径不适用。真实桌面6单元测试和根集成13测试通过，新增从真实DesktopInput::translate/winit WindowEvent进入规范化事件及UI状态分发；通用绑定准入夹具补含新生命周期变体。

证据evidence/preedit-lifecycle-chain/report.json保留源码/工具/依赖声明映射/有限范围/日志哈希。标准库时钟Kani缺口仍单独pending，没有把实际桌面原生测试当成Kani通过；原始整个translate/OS事件循环尚未形式化证明。此入口要求活动组合token，无组合的失焦仍由普通关闭处理，原生取消投递/回执及像素更新待接入。全部重活仅限额VM。

## UI-INPUT-LIFECYCLE-001：无组合/活动组合统一生命周期（2026-10-24 17:02 UTC）

handle_input_lifecycle先校验当前输入票据再读取slot.current_token，防止旧事件借用后继组合token。无组合且无呈现的失焦走普通focus/binding关闭；活动组合复用既有禁用/失焦处理，返回旧票据及可选组合取消关联。无组合却有遗留呈现时保持四对象拒绝，显式清理后才允许普通关闭。只读元数据不视为授权。

依次Verus70/0含实际getter、旧关闭/分发/gate/slot/view依赖；吞掉成功普通关闭结果/跳过外层票据校验反例拒绝。属性各256例、Miri、10秒种子fuzz通过，独立Kani unified_input_lifecycle 757检查通过（三文本、任意禁用/失焦选择、真实单节点Vec、重开旧票据拒绝、旧非成对取消产生的真实孤立呈现拒绝及显式清理，unwind8保留断言）；最后Loom逐模块真实同步路径不适用。实际DesktopInput/winit根集成14测试通过，覆盖普通失焦、活动重开旧票据、禁用后保留键盘输入、后续无组合失焦关闭。

证据evidence/input-lifecycle-chain/report.json保留源快照/工具/依赖映射/有限范围/日志哈希。旧原始translate/OS循环未整体形式化证明；外部取消投递/回执、真正编辑器/像素更新和应用循环归属装配待完成。状态DesktopInput时钟Kani缺口单独pending，未重跑未算通过，捕获诊断维持延期；全部重活仅限额VM。

## U1成熟文字布局/绘制实验：尚未验收（2026-10-24 17:22 UTC）

experiments/skia-cpu-probe新增独立skia-text-paragraph原生实验，使用既有textlayout依赖，不自研光栅或扩展Canvas。DejaVu单字体基线完成拉丁/组合字符/阿拉伯布局与绘制，显式发现中文2缺字；双显式字体加入Noto CJK SC TTC索引2后，固定混合样本无缺字，使用同一Paragraph对象布局/测量/绘制并检查重绘一致和窄布局变化。PNG已检查，ASCII0..6范围仅验证返回非空盒，**不宣称UTF-8/UTF-16/字素映射、选区、光标或全部bidi性质通过**。

旧UiBytesSlot实际安装/错误会话拒绝/释放后旧key拒绝在两个原生字体资源中重验；这只是原生组合证据，不是新增数学证明或补证包已完成。新自有Rust实验的Verus+Z3、Proptest、Miri、cargo-fuzz、Kani、Loom全部明确not_started_not_passed，不能拿原生通过替代。第三方Skia/FFI作为未证明TCB，仍须验证自有接入逻辑并正确记录不支持范围；正式后端/输入闭环仍未完成。

证据evidence/skia-text-paragraph/report.json及evidence/skia-text-paragraph-cjk/report.json包含源快照、锁文件、真实原生产物/日志/图像和字体许可/哈希。CJK字体Files:*为SIL OFL1.1，copyright中GPL-3+仅适用于未引入的debian/*打包脚本，未把整个包一概称为宽松许可。重活限额VM，未运行时钟或延期捕获诊断。


### 2026-10-24：恢复实施——本地文本帧接入代码（未验收）

已实际新增 src/text_skia.rs（显式 skia-text-pending feature，默认关闭）：显式字体
注入、真实 Taffy 测量、最终宽度 Paragraph 准备、源 Scene 持续不可变借用、
私有 CPU RGBA 消费。未修改 UiBatch 协议，既有 Engine::render 尚未迁移。
VM 原生接入 7 测试通过，场景修改借用编译失败用例 1 通过；默认关闭新 feature
的回归 139 项通过。已检查输出 PNG。初次编译类型错误已修复并保留失败日志。
证据 evidence/local-text-frame-pending/report.json 与源码快照。

这次为实际接入实现，仍非已验收交付：自有新增 Rust 六阶段均未完成，所有
LTF-01..06 证明义务登记 verification/local-text-frame/README.md；原生测试不
冒充顺序门槛。旧 surface 数值辅助的已有证明不覆盖新增调用链。实际编辑提交、
IME候选/取消交付、光标选区闭环仍未完成；jasper-os 接入继续排在 Canvas 之后。


### 2026-10-24 19:02 UTC 触发轮：文本帧存储准入实现与验证推进

生产 LocalTextRenderer::prepare 现调用 text_frame_storage，沿真实旧 Surface
行长/跨度函数完成数值准入。限定实现级 Verus 3/0、两个语义反例拒绝；Proptest
3测试（属性各256例）、Miri7边界、fuzz 4,431,914次/11秒通过。Kani同一完整
u128参考性质先CaDiCaL后一次Z3均在VM命令上限内未结束，退出124；无通过声明，
Loom阶段未到达。证据 evidence/local-text-storage-chain/report.json。

重构后真实新模块7个原生接入测试再次通过。完整文本帧/所有权/队列/原生FFI的
自有Rust证明仍待完成，整个新feature仍默认关闭、未验收，不能把这次数值子片
当文本或输入闭环完成。旧Surface函数原样纳入本次证明与测试，但无新增旧代码
完整补证交付声明。未改其他项目或在宿主执行重型测试；两次有界求解后本轮
不继续堆叠诊断重试，不降低性质或更改预算冒充通过。


### 2026-10-24 19:15 UTC 触发轮：继续落实文本帧安全边界（未验收）

实际代码已为本地渲染器和帧增加私有Rc幻影标记，约束安全Rust跨线程移动/共享；
新增原生绘制参数预检，拒绝NaN/无穷、负描边和矩形端点溢出。零宽描边不再
被Skia解释为hairline，与已有软件路径跳过零宽保持一致，未增加Canvas功能。
VM中9个原生测试、3个借用/线程编译失败检查通过；移除零宽保护的原生语义
负例确实像素断言失败，恢复源码后通过，VM/宿主源码哈希相同。

新增保护仍未获实现级数学证明，LTF-07/08已登记，不计六阶段完成。旧三函数
数值核心已机器核对与既有证明原样一致，映射记录在
local-text-frame-guard-pending/storage-proof-mapping.json；旧Kani超时保留，本轮
不重复求解、不降性质。完整模块仍默认关闭、未验收；输入闭环未完成。


### 2026-10-24 19:35 UTC 触发轮：验证环境故障与持续实现

已收取上一轮文字命中验证会话55788：退出124，仅有“开始编译”日志，无通过
结果；新有界只读SSH探测亦返回255、服务器无响应。未在宿主重试，也不修改
其他项目任务/VM服务。UI验证脚本新增外层300秒连接/准备/拉取上限，内部180秒
和3GiB/swap0/tasks128/jobs1不变；本地bash语法检查通过，不冒充Rust验证。

继续写入实际文字命中边界text_hit_offset，并准备原样源码提取的Verus精确
规格/两个负例及属性、Miri、fuzz、Kani专用入口。仅生成证明候选，VM未恢复前
六阶段均未执行，不能算通过。快照/运行状态位于local-text-hit-pending。
新feature保持默认关闭、未验收。下一轮先轻量核查VM，恢复后从Verus开始，
不重复旧存储Kani超时诊断，不把测试改到宿主，也不接管其他项目排期。


## 2026-10-27：UI2 无头验收测试与 Canvas::pixel 全链补证

### 本轮新增/修改代码的六步登记

| 代码 | 步骤 | 规格 / 范围 | 命令 | 结果 |
|---|---|---|---|---|
| `tests/native_cpu_acceptance.rs`（仅测试） | Verus/Z3 | N/A：文件只含验收断言与像素采样辅助，无生产逻辑可提取；被测对象是固定版本第三方渲染器与原生适配器（后者义务另册） | — | N/A；替代证据：仅拉丁字体注入的负例使 4 项中 3 项按预期失败 |
| 同上 | Proptest / Miri / cargo-fuzz / Kani / Loom | N/A：测试辅助不进入发行代码；被测渲染路径是大型第三方依赖（Blitz / Vello CPU），Miri 与 Kani 不可行；测试无并发 | — | N/A；替代证据同上，另有同帧重绘逐字节一致断言 |
| `src/raster.rs` `Canvas::pixel`（移入独立标记区，`Color::new` 改为等价结构体字面量） | Verus/Z3 | UI-CANVAS-PIXEL-001：越界返回 None；在 `from_rgba8` 建立的紧密长度不变式下，返回值四通道等于 `(y*w+x)*4` 起四字节；索引运算无溢出 | `verify-version.py --kernel canvas-pixel` | proved：18 verified / 0 errors；反例「越界返回 Some」「绿/蓝通道交换」均以后置条件失败被拒 |
| 同上 | Proptest | 宽整数独立规格 `assert_canvas_pixel`；1..8 尺寸、任意坐标、边界坐标 | `cargo test --manifest-path verification/read-boundary/Cargo.toml properties::`（`PROPTEST_RNG_SEED=12345`） | 通过（29 项） |
| 同上 | Miri | 4 种尺寸 × 5×5 坐标（含 `u32::MAX`）原样提取代码 | `cargo +nightly miri test … memory::` | 通过 |
| 同上 | cargo-fuzz | `read_boundary` 目标新增按输入长度构造的 1×n / n×1 画布读取 | `cargo fuzz run read_boundary`（20 秒，种子 12345） | 通过；本轮两目标 217301 / 1815980 次 |
| 同上 | Kani | `canvas_pixel_read`：1..3×1..3 任意字节、任意 `u32` 坐标 | `cargo kani --manifest-path verification/read-boundary/Cargo.toml` | 29 项 SUCCESSFUL（新增 1 项） |
| 同上 | Loom | 共享只读画布跨线程读取像素一致 | `schedules::shared_immutable_buffer_independent_cursors` | 通过；函数本身无同步原语，仅覆盖调用侧共享只读 |
| `src/style.rs` `Color` | — | 仅新增 `COLOR TYPE` 提取标记，无语义变化 | — | 作为 canvas-pixel 依赖参与上述各步 |

假设：`pixel` 的前置条件（像素缓冲长度 = 宽×高×4）由已证明的 `from_rgba8` 建立；`Canvas::new`/`resize` 建立该不变式仍未证明。整链报告 `complete=false` 的原因不变（capture-query 任意双键 Kani 原义务未解决）。


## 2026-10-29：UI19 合成斜体/粗体刻画测试与 Color 构造全链补证

### 本轮新增/修改代码的六步登记

| 代码 | 步骤 | 规格 / 范围 | 命令 | 结果 |
|---|---|---|---|---|
| `tests/native_font_synthesis.rs`（仅测试） | 六步 | N/A：只含验收断言与像素采样辅助，无生产逻辑；被测对象是固定版本第三方绘制器（Blitz / Vello CPU）的合成行为 | — | N/A；替代证据：真斜体字面作方向锚点（竖笔画右倾 +4 列），直立 0 列 |
| `src/style.rs` `Color::new/rgb/with_alpha`（移入独立 impl 与 `COLOR CONSTRUCT KERNEL` 标记区，语义不变；补欠账） | Verus/Z3 | UI-COLOR-CONSTRUCT-001 | `verify-version.py --kernel color-construct` | proved：4 verified / 0 errors；两反例均以后置条件失败被拒 |
| 同上 | Proptest | 独立规格 `assert_color_construct`，任意五个 u8 | `cargo test … properties::`（`PROPTEST_RNG_SEED=12345`） | 通过（46 项） |
| 同上 | Miri | 5 组边界通道值（0/1/127/254/255）原样提取代码 | `cargo +nightly miri test … memory::` | 通过 |
| 同上 | cargo-fuzz | `read_boundary` 目标以输入前四字节构造 | `cargo fuzz run read_boundary`（种子 12345） | 通过；两目标 147186 / 1585253 次 |
| 同上 | Kani | `color_construct_exact`：五个任意 u8 全空间 | `cargo kani --manifest-path verification/read-boundary/Cargo.toml` | 51 项 SUCCESSFUL（新增 1 项） |
| 同上 | Loom | 共享只读输入跨线程构造一致 | `schedules::shared_immutable_buffer_independent_cursors` | 通过；函数无同步原语，仅覆盖调用侧 |

假设：无。整链报告 `complete=false` 的原因不变（capture-query 任意双键 Kani 原义务未解决）。
