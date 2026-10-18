# 编译体积基线

> 测量环境：Linux x86_64，Rust 1.99.1，release profile（`lto = "thin"`, `codegen-units = 1`），
> 构建/测量使用 2 个核（`taskset -c 0-1`，`CARGO_BUILD_JOBS=2`）。
> 这是 **v0.1 原型**，不含窗口/GPU/字体/无障碍/图片/SVG。

## 当前基线

| 产物 | 字节 | MiB |
|---|---:|---:|
| `target/release/libjasper_ui_engine.rlib` | 3,724,296 | 3.55 |
| `target/release/examples/hello`（未 strip） | 933,312 | 0.89 |
| `target/release/examples/hello`（strip） | 782,368 | 0.75 |

`size` 的段统计（未 strip 的 example）：

```text
   text    data     bss     dec     hex filename
 761,381   17,600   1,387  780,368   be850 target/release/examples/hello
```

## 怎么理解

- **rlib** 是 Rust 库归档（含对象/元数据），不是最终应用；它比可执行文件大是正常的。
- **example** 是当前最小可执行链路：Rust 运行时 + 本 crate + `taffy`；**没有** winit/wgpu/Vello/Parley/AccessKit/image/resvg。
- 后续接入真实的窗口/GPU/字体/无障碍/图片栈，最终应用会明显变大；不要把 0.75 MiB 当成“完整 UI 引擎”的体积。
- 平台/链接方式/调试符号/LTO 会改变数字；本表只做回归基线，不做跨平台承诺。

## 复现

```bash
taskset -c 0-1 ./scripts/size-report.sh
```

脚本只构建 release 的 lib 和 `hello` 示例，并额外生成一份 strip 副本测量；不会修改仓库文件。

## 后续跟踪

- v0.2：命中测试/指针路由已加入；体积应基本不变（无重型依赖）。
- v0.3：Parley/winit/wgpu 引入后重新测，按“引擎 + 最小宿主”分别记录。
- 每次引入重型依赖，更新本表并在 `dependencies.md` 记录许可证/版本。
