# 编译体积基线

## 最近一次体积快照（2026-10-23）

此为历史兼容执行链快照。2026-10-26 hello 已迁至根 Blitz 原生候选入口，要求 native-blitz-cpu-pending 与显式字体；下列旧命令/体积不能作为迁移后程序的当前基线。

Linux x86_64 构建虚拟机，Rust 1.99.1，release（thin LTO、codegen-units=1），CARGO_BUILD_JOBS=2。使用当次测量源码构建；后续源码变化不自动刷新此快照。源文件/锁文件哈希见 `evidence/size-current/source-hashes.json`。

| 产物 | 字节 | MiB | 配置 |
|---|---:|---:|---|
| 引擎 rlib | 4,416,886 | 4.21 | 默认特性，含可选 Canvas 默认实现，不启用 desktop |
| 最小 hello 程序 | 949,104 | 0.91 | 默认特性，未 strip |
| 最小 hello 程序 | 796,256 | 0.76 | 默认特性，strip 后 |
| 引擎 rlib（含桌面适配） | 4,587,186 | 4.37 | 默认特性 + desktop |
| 窗口示例程序 | 4,550,256 | 4.34 | 默认特性 + desktop，未 strip |
| 窗口示例程序 | 3,563,672 | 3.40 | 默认特性 + desktop，strip 后 |

窗口示例对应现有的色块启动窗口：winit + softbuffer + 软件渲染。此次只做编译与体积测量，没有将新 release 二进制启动验收计作已完成。

- rlib 是 Rust 中间库归档，包含对象与元数据，不是应用安装包；不能把 rlib 和可执行文件简单相加。
- 最小 hello 会受链接裁剪影响，不代表完整引擎所有功能都在 0.76 MiB 内。
- 窗口程序使用系统动态库；不包含完整文字/GPU/图片处理、应用资源、安装器及全部运行环境。
- 系统内置宿主将执行核心放在系统 UI 服务中；应用可不重复携带渲染器，但现有数据不能当作内置系统镜像或应用包体实测。
- 体积为磁盘文件大小，不是运行时 RSS、显存占用或压缩下载大小。形式化验证工具只在构建环境运行，不打包进引擎。

构建命令：

```bash
cargo build --locked --release --lib --example hello --example window --features desktop
cargo build --locked --release --lib --example hello
```

以上均通过项目构建虚拟机执行；strip 对证据副本进行，未修改原始可执行文件。原始数字见 `evidence/size-current/sizes.txt` 与 `sizes-default.txt`；系统库清单见 `window-ldd.txt`。default 与 desktop 两种构建必须分别记录，不混用其 rlib。

## 历史基线（2026-10-18，保留追溯）


> 测量环境：Linux x86_64，Rust 1.99.1，release profile（`lto = "thin"`, `codegen-units = 1`），
> 构建/测量使用 2 个核（`taskset -c 0-1`，`CARGO_BUILD_JOBS=2`）。
> 这是 **v0.1 原型**，不含窗口/GPU/字体/无障碍/图片/SVG。

### 当时的基线

| 产物 | 字节 | MiB |
|---|---:|---:|
| `target/release/libjasper_ui_engine.rlib` | 3,893,128 | 3.71 |
| `target/release/examples/hello`（未 strip） | 933,840 | 0.89 |
| `target/release/examples/hello`（strip） | 782,896 | 0.75 |

`size` 的段统计（未 strip 的 example）：

```text
   text    data     bss     dec     hex filename
 761,917   17,600     859  780,376   be858 target/release/examples/hello
```

### 当时的解释

- **rlib** 是 Rust 库归档（含对象/元数据），不是最终应用；它比可执行文件大是正常的。
- **example** 是当前最小可执行链路：Rust 运行时 + 本 crate + `taffy`；**没有** winit/wgpu/Vello/Parley/AccessKit/image/resvg。
- 后续接入真实的窗口/GPU/字体/无障碍/图片栈，最终应用会明显变大；不要把 0.75 MiB 当成“完整 UI 引擎”的体积。
- 平台/链接方式/调试符号/LTO 会改变数字；本表只做回归基线，不做跨平台承诺。

### 旧脚本

```bash
taskset -c 0-1 ./scripts/size-report.sh
```

脚本只构建 release 的 lib 和 `hello` 示例，并额外生成一份 strip 副本测量；不会修改仓库文件。

### 历史跟踪计划

- v0.2：命中测试/指针路由已加入；体积应基本不变（无重型依赖）。
- v0.3：Parley/winit/wgpu 引入后重新测，按“引擎 + 最小宿主”分别记录。
- 每次引入重型依赖，更新本表并在 `dependencies.md` 记录许可证/版本。
