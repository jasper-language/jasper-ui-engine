// Entry point for the Xcode app target.
// Xcode app target 的入口。
//
// `UIApplicationMain` runs inside winit's event loop, so the app hands control to the Rust host
// and never returns until the host exits.
// `UIApplicationMain` 在 winit 事件循环内部运行；这里把控制权交给 Rust 宿主，宿主退出前不返回。

#import <UIKit/UIKit.h>

extern void jasper_ios_main(void);

int main(int argc, char *argv[]) {
    @autoreleasepool {
        jasper_ios_main();
    }
    return 0;
}
