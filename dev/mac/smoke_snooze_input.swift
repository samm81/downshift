import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("[smoke-input] error: \(message)\n".utf8))
    exit(1)
}

func axAttribute(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else {
        return nil
    }
    return value
}

func axString(_ element: AXUIElement, _ attribute: String) -> String? {
    axAttribute(element, attribute) as? String
}

func axChildren(_ element: AXUIElement) -> [AXUIElement] {
    guard let value = axAttribute(element, kAXChildrenAttribute) else {
        return []
    }
    return value as? [AXUIElement] ?? []
}

func menuItem(in element: AXUIElement, matching targetTitle: String, depth: Int = 0) -> AXUIElement? {
    guard depth <= 12 else {
        return nil
    }

    let role = axString(element, kAXRoleAttribute)
    let title = axString(element, kAXTitleAttribute)?.lowercased()
    if role == "AXMenuItem" && title == targetTitle.lowercased() {
        return element
    }

    for child in axChildren(element) {
        if let match = menuItem(in: child, matching: targetTitle, depth: depth + 1) {
            return match
        }
    }
    return nil
}

func findMenuItem(named title: String, processID: Int32?) -> AXUIElement? {
    if let processID {
        return menuItem(
            in: AXUIElementCreateApplication(processID),
            matching: title
        )
    }

    let applications = NSWorkspace.shared.runningApplications.filter { application in
        application.activationPolicy != .prohibited
    }
    for application in applications {
        let applicationElement = AXUIElementCreateApplication(application.processIdentifier)
        if let item = menuItem(in: applicationElement, matching: title) {
            return item
        }
    }
    return nil
}

func waitForMenuItem(
    named title: String,
    processID: Int32?,
    timeout: TimeInterval = 5.0
) -> AXUIElement? {
    let deadline = Date().addingTimeInterval(timeout)
    repeat {
        if let item = findMenuItem(named: title, processID: processID) {
            return item
        }
        Thread.sleep(forTimeInterval: 0.05)
    } while Date() < deadline
    return nil
}

func pressAllowButton(in element: AXUIElement, depth: Int = 0) -> Bool {
    guard depth <= 12 else {
        return false
    }

    let role = axString(element, kAXRoleAttribute)
    let title = axString(element, kAXTitleAttribute)?.lowercased()
    if role == "AXButton" && title == "allow" {
        return AXUIElementPerformAction(element, kAXPressAction as CFString) == .success
    }

    for child in axChildren(element) {
        if pressAllowButton(in: child, depth: depth + 1) {
            return true
        }
    }
    return false
}

func pressVisibleAllowButton() -> Bool {
    let applications = NSWorkspace.shared.runningApplications.filter { application in
        application.localizedName != nil && application.activationPolicy != .prohibited
    }
    for application in applications {
        let applicationElement = AXUIElementCreateApplication(application.processIdentifier)
        if pressAllowButton(in: applicationElement) {
            return true
        }
    }
    return false
}

func number(_ value: Any?) -> Double? {
    (value as? NSNumber)?.doubleValue
}

func windowBounds(for processID: Int32) -> CGRect? {
    let windows = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] ?? []

    var best: CGRect?
    for info in windows {
        guard let owner = info[kCGWindowOwnerName as String] as? String,
              ["Downshift", "downshift"].contains(owner),
              let ownerProcessID = (info[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value,
              ownerProcessID == processID,
              let bounds = info[kCGWindowBounds as String] as? [String: Any],
              let x = number(bounds["X"]),
              let y = number(bounds["Y"]),
              let width = number(bounds["Width"]),
              let height = number(bounds["Height"]),
              width >= 50,
              height >= 50
        else {
            continue
        }

        let candidate = CGRect(x: x, y: y, width: width, height: height)
        if best == nil || candidate.width * candidate.height > best!.width * best!.height {
            best = candidate
        }
    }
    return best
}

func postMouse(_ type: CGEventType, x: Double, y: Double, button: CGMouseButton) {
    let point = CGPoint(x: x, y: y)
    guard let event = CGEvent(
        mouseEventSource: nil,
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: button
    ) else {
        fail("could not create mouse event")
    }
    event.post(tap: .cghidEventTap)
}

func postKey(_ keyCode: CGKeyCode) {
    guard let keyDown = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: true),
          let keyUp = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: false)
    else {
        fail("could not create keyboard event")
    }
    keyDown.post(tap: .cghidEventTap)
    keyUp.post(tap: .cghidEventTap)
}

func keyCode(named name: String) -> CGKeyCode {
    switch name.lowercased() {
    case "home": return 115
    case "down": return 125
    case "right": return 124
    case "return", "enter": return 36
    case "escape", "esc": return 53
    default: fail("unsupported key '\(name)'")
    }
}

func requireArgument(_ arguments: [String], _ index: Int, _ description: String) -> String {
    guard arguments.indices.contains(index) else {
        fail("missing \(description)")
    }
    return arguments[index]
}

func requireInt32(_ arguments: [String], _ index: Int, _ description: String) -> Int32 {
    guard let value = Int32(requireArgument(arguments, index, description)) else {
        fail("invalid \(description)")
    }
    return value
}

func requireDouble(_ arguments: [String], _ index: Int, _ description: String) -> Double {
    guard let value = Double(requireArgument(arguments, index, description)) else {
        fail("invalid \(description)")
    }
    return value
}

let arguments = Array(CommandLine.arguments.dropFirst())
let command = requireArgument(arguments, 0, "command")

switch command {
case "window-bounds":
    let processID = requireInt32(arguments, 1, "process id")
    guard let bounds = windowBounds(for: processID) else {
        fail("could not find a visible app window for process \(processID)")
    }
    print("\(Int(bounds.origin.x.rounded())) \(Int(bounds.origin.y.rounded())) \(Int(bounds.width.rounded())) \(Int(bounds.height.rounded()))")

case "right-click":
    let x = requireDouble(arguments, 1, "x coordinate")
    let y = requireDouble(arguments, 2, "y coordinate")
    postMouse(.rightMouseDown, x: x, y: y, button: .right)
    Thread.sleep(forTimeInterval: 0.1)
    postMouse(.rightMouseUp, x: x, y: y, button: .right)

case "key":
    postKey(keyCode(named: requireArgument(arguments, 1, "key name")))

case "menu-visible":
    let title = requireArgument(arguments, 1, "menu item title")
    let processID = arguments.indices.contains(2) ? Int32(arguments[2]) : nil
    let timeout = arguments.indices.contains(3)
        ? requireDouble(arguments, 3, "menu wait timeout")
        : 5.0
    guard waitForMenuItem(
        named: title,
        processID: processID,
        timeout: timeout
    ) != nil else {
        fail("timed out waiting for native menu item '\(title)'")
    }
    print("visible")

case "menu-click":
    let title = requireArgument(arguments, 1, "menu item title")
    let processID = arguments.indices.contains(2) ? Int32(arguments[2]) : nil
    let item = waitForMenuItem(named: title, processID: processID)
    guard let item else {
        fail("timed out waiting for native menu item '\(title)'")
    }
    guard AXUIElementPerformAction(item, kAXPressAction as CFString) == .success else {
        fail("could not press menu item '\(title)'")
    }

case "allow-screen-capture":
    guard pressVisibleAllowButton() else {
        fail("could not find a visible screen-capture Allow button")
    }

default:
    fail("unsupported command '\(command)'")
}
