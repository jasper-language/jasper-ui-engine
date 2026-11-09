// Drive one real macOS pointer click at screen coordinates into a target process.
// 在屏幕坐标处向目标进程投放一次真实的 macOS 指针点击。
//
// The helper refuses to click unless the target application is frontmost, so a failed
// activation cannot land the click on an unrelated window.
// 目标应用不是最前台时，助手拒绝点击；这样激活失败也不会把点击投到无关窗口。
//
// Usage: swift os-click.swift PID X Y
import AppKit

let arguments = CommandLine.arguments
guard arguments.count == 4,
    let pid = Int32(arguments[1]),
    let x = Double(arguments[2]),
    let y = Double(arguments[3])
else {
    print("usage: os-click PID X Y")
    exit(2)
}

let trusted = AXIsProcessTrusted()
guard let application = NSRunningApplication(processIdentifier: pid_t(pid)) else {
    print("os_click=skipped reason=no_such_process pid=\(pid) ax_trusted=\(trusted)")
    exit(3)
}

let activated = application.activate(options: [.activateAllWindows])
Thread.sleep(forTimeInterval: 0.4)
let frontmost = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
print(
    "os_click ax_trusted=\(trusted) activate_requested=\(activated) "
        + "frontmost=\(frontmost) target=\(pid)"
)
guard frontmost == pid_t(pid) else {
    print("os_click=skipped reason=target_not_frontmost")
    exit(4)
}

let point = CGPoint(x: x, y: y)
guard let down = CGEvent(
    mouseEventSource: nil, mouseType: .leftMouseDown, mouseCursorPosition: point, mouseButton: .left
),
    let up = CGEvent(
        mouseEventSource: nil, mouseType: .leftMouseUp, mouseCursorPosition: point, mouseButton: .left
    )
else {
    print("os_click=skipped reason=event_creation_failed")
    exit(5)
}
down.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.08)
up.post(tap: .cghidEventTap)
print("os_click=posted point=\(Int(x)),\(Int(y))")
exit(0)
