# iOS / iPadOS 宿主

iOS/iPadOS 参考宿主位于 [`ios/`](../ios/)；构建、打包与限制见
[ios/README.md](../ios/README.md)。

同一份二进制同时服务 iPhone 与 iPad：bundle 的 `UIDeviceFamily` 为 `[1, 2]`，
`UISupportedInterfaceOrientations~ipad` 允许四个方向。平台无关规则集中在
`ios/src/model.rs`，在构建机上直接单测。

## 生命周期

winit 的 iOS 后端在 `EventLoop::run_app` 内调用 `UIApplicationMain`。宿主把 UIKit 通知映射为
`model::Lifecycle`：

| UIKit 事件 | 宿主动作 |
|---|---|
| `applicationDidBecomeActive` | 首次 `Activate`，之后 `Resume`（使用保存的后继 Surface 代数） |
| `applicationWillResignActive` | `Suspend`，释放 drawable/device/queue/window |
| `applicationDidEnterBackground` | 无（已在 `Suspend` 时释放） |
| `applicationWillEnterForeground` | 无 |
| `applicationWillTerminate` | `Terminate`，退休原生视图 |

重复或乱序通知是幂等的：不会重复暂停或恢复。`model` 的 7 个单测覆盖完整循环、重复通知、
终止后不再恢复、后台直接激活、iPad 安全区换算、非法内缩与主触点移交。

## 输入与安全区

- 触摸经 winit `Touch` → `DesktopInput` → `Engine::pointer`；宿主维护稳定主触点，
  首个触点结束前保持 primary，结束后交给最早剩余触点。
- `Viewport` 由物理尺寸、scale 与安全区内缩推导逻辑内容矩形，根场景使用内容矩形；
  整个视口仍是引擎 Surface 尺寸。
- 当前适配器尚未查询 `UIView.safeAreaInsets`，安全区由 `--insets top,right,bottom,left`
  由宿主给出（默认 0）。这是与真机集成的明确缺口，不冒充已实现。

## 打包与签名

`ios/build-app.sh` 产出 `JasperUI.app`：替换 `Info.plist.in` 的平台字段、复制二进制与
`PkgInfo`、然后签名。

- Simulator 目标 `aarch64-apple-ios-sim`：ad-hoc 签名，`codesign --verify --deep --strict` 通过。
- Device 目标 `aarch64-apple-ios`：只在存在代码签名身份时签名；否则明确输出
  `device_signing=unavailable`，不产出可安装包。
- 两个切片都通过 `-Wl,-platform_version` 产出 `LC_BUILD_VERSION`（`IOS` / `IOSSIMULATOR`，
  `minos 15.0`），而不是旧的 `LC_VERSION_MIN_IPHONEOS`。

## 验收状态

| 层 | 结果 |
|---|---|
| 交叉编译与链接 | `aarch64-apple-ios`、`aarch64-apple-ios-sim` 均通过，链接 UIKit/Metal/QuartzCore |
| bundle 静态验证 | `plutil`、`UIDeviceFamily [1,2]`、平台字段、Mach-O 平台、签名与哈希 |
| 可移植规则 | `cargo test --manifest-path ios/Cargo.toml` 7/7 |
| 适配器演练 | `--features host-on-macos` 把同一宿主模块编译进 macOS 构建，跑 iPhone 与 iPad 视口（含安全区内缩和点击激活） |
| 运行时 | **未验收**：验证机没有安装 Simulator 运行时，也没有设备代码签名身份 |

运行时验收需要其中之一：安装 Simulator 运行时后执行
`xcrun simctl install/launch`，或用开发签名身份在真机安装。证据文件
`evidence/ios-reference-host/report.json` 记录静态检查输出、可移植规则结果与适配器演练数据，
并明确标注运行时未验收。

## 未完成

- 模拟器/真机运行启动与画面验收。
- `UIView.safeAreaInsets` 查询与软键盘遮挡处理。
- 系统无障碍（钉住的 AccessKit 无 iOS 后端）、活 IME/软键盘组合、Pencil 压力与倾斜。
- Metal 设备丢失、热/功耗设备矩阵、App Store 打包与图标。
