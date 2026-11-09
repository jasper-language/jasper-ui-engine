//! Standalone binary entry for the iOS/iPadOS reference host.
//! iOS/iPadOS 参考宿主的独立二进制入口。
//!
//! Device installs use `ios/app/JasperUI.xcodeproj`, which links the static library and calls
//! `jasper_ios_main`. This binary keeps `cargo build`/`cargo run` usable for CI and the
//! `host-on-macos` adapter exercise.
//! 真机安装走 `ios/app/JasperUI.xcodeproj`（链接静态库并调用 `jasper_ios_main`）；
//! 本二进制保留给 CI 与 `host-on-macos` 适配器演练使用。
fn main() {
    jasper_ui_ios::run();
}
