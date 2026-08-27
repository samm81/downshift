import AppKit
import ApplicationServices
import CoreGraphics
import Darwin
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("[smoke-input] error: \(message)\n".utf8))
    exit(1)
}

func axAttribute(_ element: AXUIElement, _ attribute: CFString) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute, &value) == .success else {
        return nil
    }
    return value
}

func axString(_ element: AXUIElement, _ attribute: CFString) -> String? {
    axAttribute(element, attribute) as? String
}

func axChildren(_ element: AXUIElement) -> [AXUIElement] {
    guard let value = axAttribute(element, kAXChildrenAttribute) else {
        return []
    }
    if let children = value as? [AXUIElement] {
        return children
    }
    if let children = value as? NSArray {
        return children.compactMap { $0 as? AXUIElement }
    }
    return []
}

func statusItem(in element: AXUIElement, depth: Int = 0) -> AXUIElement? {
    guard depth <= 10 else {
        return nil
    }

    let role = axString(element, kAXRoleAttribute)
    let searchableText = [
        axString(element, kAXTitleAttribute),
        axString(element, kAXDescriptionAttribute),
        axString(element, kAXValueAttribute),
    ]
    .compactMap { $0?.lowercased() }
    if role == "AXMenuBarItem" && searchableText.contains(where: { $0.contains("downshift") }) {
        return element
    }

    for child in axChildren(element) {
        if let match = statusItem(in: child, depth: depth + 1) {
            return match
        }
    }
    return nil
}

func systemStatusItem() -> AXUIElement? {
    let processNames = Set(["SystemUIServer", "ControlCenter"])
    let applications = NSWorkspace.shared.runningApplications.filter { application in
        guard let name = application.localizedName else {
            return false
        }
        return processNames.contains(name)
    }

    for application in applications {
        let applicationElement = AXUIElementCreateApplication(application.processIdentifier)
        if let match = statusItem(in: applicationElement) {
            return match
        }
    }
    return nil
}

func axGeometry(_ element: AXUIElement) -> (CGPoint, CGSize)? {
    guard let positionValue = axAttribute(element, kAXPositionAttribute),
          let sizeValue = axAttribute(element, kAXSizeAttribute),
          CFGetTypeID(positionValue) == AXValueGetTypeID(),
          CFGetTypeID(sizeValue) == AXValueGetTypeID()
    else {
        return nil
    }

    let positionAXValue = positionValue as! AXValue
    let sizeAXValue = sizeValue as! AXValue
    guard AXValueGetType(positionAXValue) == .cgPoint,
          AXValueGetType(sizeAXValue) == .cgSize
    else {
        return nil
    }

    var position = CGPoint.zero
    var size = CGSize.zero
    guard AXValueGetValue(positionAXValue, .cgPoint, &position),
          AXValueGetValue(sizeAXValue, .cgSize, &size)
    else {
        return nil
    }
    return (position, size)
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
        guard let ownerProcessID = (info[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value,
              ownerProcessID == processID,
              (info[kCGWindowLayer as String] as? NSNumber)?.intValue == 0,
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

let arguments = Array(CommandLine.arguments.dropFirst())
let command = requireArgument(arguments, 0, "command")

switch command {
case "window-bounds":
    let processID = Int32(requireArgument(arguments, 1, "process id")) ?? fail("invalid process id")
    guard let bounds = windowBounds(for: processID) else {
        fail("could not find a visible app window for process \(processID)")
    }
    print("\(Int(bounds.origin.x.rounded())) \(Int(bounds.origin.y.rounded())) \(Int(bounds.width.rounded())) \(Int(bounds.height.rounded()))")

case "right-click":
    let x = Double(requireArgument(arguments, 1, "x coordinate")) ?? fail("invalid x coordinate")
    let y = Double(requireArgument(arguments, 2, "y coordinate")) ?? fail("invalid y coordinate")
    postMouse(.rightMouseDown, x: x, y: y, button: .right)
    postMouse(.rightMouseUp, x: x, y: y, button: .right)

case "left-click":
    let x = Double(requireArgument(arguments, 1, "x coordinate")) ?? fail("invalid x coordinate")
    let y = Double(requireArgument(arguments, 2, "y coordinate")) ?? fail("invalid y coordinate")
    postMouse(.leftMouseDown, x: x, y: y, button: .left)
    postMouse(.leftMouseUp, x: x, y: y, button: .left)

case "move":
    let x = Double(requireArgument(arguments, 1, "x coordinate")) ?? fail("invalid x coordinate")
    let y = Double(requireArgument(arguments, 2, "y coordinate")) ?? fail("invalid y coordinate")
    postMouse(.mouseMoved, x: x, y: y, button: .left)

case "key":
    postKey(keyCode(named: requireArgument(arguments, 1, "key name")))

case "tray-rect":
    guard let item = systemStatusItem(), let (position, size) = axGeometry(item) else {
        fail("could not locate the Downshift status item; Accessibility access may be unavailable")
    }
    print("\(Int(position.x.rounded())) \(Int(position.y.rounded())) \(Int(size.width.rounded())) \(Int(size.height.rounded()))")

case "tray-click":
    guard let item = systemStatusItem() else {
        fail("could not locate the Downshift status item; Accessibility access may be unavailable")
    }
    guard AXUIElementPerformAction(item, kAXPressAction) == .success else {
        fail("could not press the Downshift status item")
    }

default:
    fail("unsupported command '\(command)'")
}
