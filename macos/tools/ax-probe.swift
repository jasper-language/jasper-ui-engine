// Walk and drive one process's macOS accessibility tree through the real AX API.
// 通过真实 AX API 遍历并驱动一个进程的 macOS 无障碍树。
//
// Usage: swift ax-probe.swift PID [--press "LABEL"]
// Prints one line per node: role, title, description, actions, position and size.
// 每行一个节点：角色、标题、描述、动作、位置与尺寸。

import AppKit
import ApplicationServices

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    let error = AXUIElementCopyAttributeValue(element, name as CFString, &value)
    return error == .success ? value : nil
}

func text(_ element: AXUIElement, _ name: String) -> String {
    guard let value = attribute(element, name) else { return "" }
    if let string = value as? String { return string }
    return "\(value)"
}

func point(_ element: AXUIElement, _ name: String) -> CGPoint? {
    guard let value = attribute(element, name) else { return nil }
    var result = CGPoint.zero
    guard AXValueGetValue(value as! AXValue, .cgPoint, &result) else { return nil }
    return result
}

func size(_ element: AXUIElement, _ name: String) -> CGSize? {
    guard let value = attribute(element, name) else { return nil }
    var result = CGSize.zero
    guard AXValueGetValue(value as! AXValue, .cgSize, &result) else { return nil }
    return result
}

func actions(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return [] }
    return names as? [String] ?? []
}

var pressed = false
var walked = 0
var buttonFound: (AXUIElement, String)? = nil

func walk(_ element: AXUIElement, _ depth: Int, _ target: String?) {
    walked += 1
    let role = text(element, kAXRoleAttribute)
    let title = text(element, kAXTitleAttribute)
    let description = text(element, kAXDescriptionAttribute)
    let value = text(element, kAXValueAttribute)
    let position = point(element, kAXPositionAttribute)
    let box = size(element, kAXSizeAttribute)
    let names = actions(element)
    let location = position.map { "\(Int($0.x)),\(Int($0.y))" } ?? "-"
    let extent = box.map { "\(Int($0.width))x\(Int($0.height))" } ?? "-"
    print(
        "ax depth=\(depth) role=\(role) title=\"\(title)\" description=\"\(description)\" "
            + "value=\"\(value)\" position=\(location) size=\(extent) actions=[\(names.joined(separator: ","))]"
    )
    // Substring match so a localized label such as "激活 Activate" is still addressable.
    // 子串匹配，使 “激活 Activate” 这类本地化标签仍可定位。
    if let target, title.contains(target), names.contains(kAXPressAction as String), !pressed {
        buttonFound = (element, target)
    }
    if let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] {
        for child in children {
            walk(child, depth + 1, target)
        }
    }
}

let arguments = CommandLine.arguments
guard arguments.count >= 2, let pid = Int32(arguments[1]) else {
    print("usage: ax-probe PID [--press LABEL]")
    exit(2)
}
var target: String? = nil
if let index = arguments.firstIndex(of: "--press"), index + 1 < arguments.count {
    target = arguments[index + 1]
}
print("ax_trusted=\(AXIsProcessTrusted()) pid=\(pid)")
walk(AXUIElementCreateApplication(pid), 0, target)
print("ax_nodes=\(walked)")
if let (element, label) = buttonFound {
    let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
    pressed = error == .success
    print("ax_press label=\"\(label)\" result=\(pressed) error=\(error.rawValue)")
} else if target != nil {
    print("ax_press label=\"\(target!)\" result=false reason=not_found")
}
exit(0)
