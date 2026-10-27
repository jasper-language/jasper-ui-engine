# Skia CPU 接入预检（实验，非生产后端）

按用户 2026-10-24 裁决，Canvas 复用独立 Skia，不再扩展自写光栅器。本包只直接调用上游库验证构建和像素行为，不是 Jasper 主渲染路径或已完成六阶段验证的接入实现；不引入根 crate 的运行依赖。

固定 `skia-safe = 0.153.3`，采用默认功能加 `textlayout`，匹配上游已发布的 CPU 预编译组合；不启用 GPU。最初的无默认功能极简组合未提供匹配预编译产物（404），回退源码构建时缺 ninja，失败日志保留；没有把失败计通过。选择依据是实际 Cargo registry；上游 master 的 0.155.0 在本次 registry 查询中不存在，不能拿未发布 manifest 当已可用发行版。`skia-safe` 是 MIT，Skia 的 BSD 许可及构建产物依赖许可仍需随实际产物登记。

```sh
RUSTUP_TOOLCHAIN=1.99.1-x86_64-unknown-linux-gnu \
CARGO_TARGET_DIR="$PWD/target/skia-cpu-probe" \
CARGO_BUILD_JOBS=2 \
cargo run --locked --manifest-path experiments/skia-cpu-probe/Cargo.toml --bin skia-native-smoke -- evidence/skia-cpu-probe
```

输出 RGBA/PPM，断言矩形填充、嵌套无交集裁剪、透明混色和 RGBA 读回。绘图全部由 Skia 执行；本程序仅构造固定测试输入、检查结果和导出像素，不实现光栅化算法。不创建窗口、不请求 GPU、不加载字体；这不代表库在所有目标上不依赖系统构建库。

信任边界包括 upstream Skia C++、Rust FFI/绑定、下载的预编译产物及其构建依赖、工具链。本实验不是 Verus 证明，也不是正式的 Miri/Kani/Loom 覆盖；特别是 native FFI 不能凭纯 Rust 适配层模型检查冒充已被 Miri 执行。

尚未覆盖：Jasper 显示列表到 Skia 的正式映射、文字与宿主注入字体、预算/资源生命周期、实际窗口呈现、Vulkan 后端及 jasper-os 目标构建。3D 延后且不归此 UI Rust 工作线。下一步按真实预检结果推进主路径，保留软件回退，不将预检通过写成全部接入完成。

## 实测结果（2026-10-24）

Jasper 构建 VM 的 x86_64 Linux 目标已通过 `--locked` 实际运行，四类像素行为断言成功，产物见 `evidence/skia-cpu-probe/`（RGBA、PPM、PNG、源码/锁文件哈希报告、构建日志）。正式引擎后端和六阶段验收仍未完成；这是可运行的第三方接入预检。

Linux 预编译组合在链接时要求 FreeType/fontconfig 开发库，已仅在构建 VM 安装 `libfreetype-dev libfontconfig-dev`；实验不调用字体 API，最终本实验可执行文件的 ldd 列表不含这两个动态库，但不能据此推断文字功能或其他后端也无需它们。依赖图、链接库输出和编译器版本已归档。标准 Linux 预编译包不可直接当 jasper-os 可用产物。第三方资产 URL 与 GitHub 元数据 digest 已登记，未独立核对下载归档的实际 digest，不冒充供应链哈希验证完成。

## 真实引擎帧实验

```sh
RUSTUP_TOOLCHAIN=1.99.1-x86_64-unknown-linux-gnu \
CARGO_TARGET_DIR="$PWD/target/skia-cpu-probe" \
CARGO_BUILD_JOBS=2 \
cargo run --locked --manifest-path experiments/skia-cpu-probe/Cargo.toml --bin skia-engine-frame -- evidence/skia-cpu-probe
```

使用真实 Scene、Taffy 布局、paint、UiBatch encode/decode 后重放至 Skia；根依赖关闭默认特性，不调用 tiny-skia 或自写 raster::Canvas。固定 320×200 场景输出三个布局节点、十条 PaintOp、188 字节协议批，四个像素断言验证背景、面板、Canvas 内容和内容外裁剪。Clear/矩形/描边/线段/PushClip/PopClip 的绘图交由 Skia；实验拒绝文字、裁剪下溢/未配平和非首条 Clear，不静默忽略。

此适配实验不承诺任意输入/浮点异常、完整 Clear 语义、resize/发布生命周期或生产行为等价。固定尺寸、未接字体且没有实现 RenderBackend，不是默认后端切换。新增映射代码尚未完成六阶段验证，保持实验/未验收；禁止把 native 像素检查替代 Verus 证明，也不能用该实验为旧源文件增加全链完成数。

## 宿主显式注入字体的文字实验

在上一条 `skia-engine-frame` 命令末尾追加一个由宿主提供的合法字体文件路径，即启用文字测试。文件经实际 UiBytesSlot 安装和身份受检借用，再用 Skia Data::new_copy/FontMgr::custom_empty/new_from_data 从字节解析。没有调用系统 FontMgr 或字体族查找；custom_empty 的可用性取决于该目标的 FreeType 构建，不宣称所有平台或 jasper-os 都已支持。Rust 槽位自身只移动 Vec，但传给 Skia Data 时会复制，不能称整条字体路径零复制。

Linux 实测采用明确传入的 DejaVuSans.ttf，SHA 和发行版版权文件归档到 evidence/skia-cpu-probe。完整场景经四个布局节点、十一条实际 PaintOp、218 字节 UiBatch，把标题 Jasper UI 交给 Skia 绘制；与去掉 DrawText 的对照帧比较有 535 个像素变化，且全部位于标题区域。字形查询确认 J 有对应 glyph；使用后归还资源并确认旧 key 不能继续读取。无字体参数的旧路径仍拒绝文字。

这只验证固定 Latin 标题的字形绘制，没有使用完整段落 shaping，不宣称 CJK/RTL/字素/选区或布局测量与真实字体一致；原 SimpleTextEngine 仍近似。实验适配层和 native 字体解析未完成六阶段/实现级证明，正式 RenderBackend 仍未验收。既有纯 Rust 资源槽的证明不能扩大到 Skia 内部。首次错误地假设 custom_empty 的 family 计数必为零，实际为一；已删去无依据的该计数 oracle，保留失败日志，实际从字节解析和像素对照验收保持不变。
