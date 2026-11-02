import ApplicationServices
import AppKit
// usage: notessearch info | notessearch set <text> [confirm] | notessearch menu <title substring>
// Sets Notes' toolbar search field by AX without activating Notes, then reports whether the
// note list changed and whether the frontmost app stayed the same.
let args = CommandLine.arguments
print("trusted", AXIsProcessTrusted())
guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.Notes").first else { print("no notes"); exit(1) }
let root = AXUIElementCreateApplication(app.processIdentifier)
AXUIElementSetMessagingTimeout(root, 1)
func attr(_ e: AXUIElement, _ n: String) -> CFTypeRef? { var v: CFTypeRef?; return AXUIElementCopyAttributeValue(e, n as CFString, &v) == .success ? v : nil }
func settable(_ e: AXUIElement, _ n: String) -> Bool { var s: DarwinBoolean = false; return AXUIElementIsAttributeSettable(e, n as CFString, &s) == .success && s.boolValue }
func walk(_ e: AXUIElement, _ depth: Int, _ visit: (AXUIElement) -> Bool) -> AXUIElement? {
  if depth > 14 { return nil }
  if visit(e) { return e }
  for c in (attr(e, "AXChildren") as? [AXUIElement]) ?? [] { if let f = walk(c, depth + 1, visit) { return f } }
  return nil
}
func rows() -> Int {
  var count = 0
  _ = walk(root, 0) { e in
    let role = attr(e, "AXRole") as? String
    if role == "AXTable" || role == "AXOutline" || (role == "AXList" && count == 0) { count = max(count, ((attr(e, "AXRows") ?? attr(e, "AXChildren")) as? [AXUIElement])?.count ?? 0) }
    return false
  }
  return count
}
guard let field = walk(root, 0, { (attr($0, "AXSubrole") as? String) == "AXSearchField" }) else { print("no search field"); exit(2) }
var names: CFArray?
AXUIElementCopyActionNames(field, &names)
print("role", attr(field, "AXRole") ?? "-", "value", attr(field, "AXValue") ?? "-", "focused", attr(field, "AXFocused") ?? "-",
      "settableValue", settable(field, "AXValue"), "actions", (names as? [String]) ?? [])
print("settableFocused", settable(field, "AXFocused"), "settableSelectedText", settable(field, "AXSelectedText"))
if args.count >= 3 && args[1] == "menu" {
  // Background AXPress on a menu item (the route taucode uses for hotkeys); then read focus.
  let bar = attr(root, "AXMenuBar") as! AXUIElement
  var found: [AXUIElement] = []
  func items(_ e: AXUIElement, _ path: String, _ depth: Int) {
    if depth > 6 { return }
    for c in (attr(e, "AXChildren") as? [AXUIElement]) ?? [] {
      let title = (attr(c, "AXTitle") as? String) ?? ""
      let here = title.isEmpty ? path : path + " > " + title
      if (attr(c, "AXRole") as? String) == "AXMenuItem", title.localizedCaseInsensitiveContains(args[2]) { print("item", here, "enabled", attr(c, "AXEnabled") ?? "-"); found.append(c) }
      items(c, here, depth + 1)
    }
  }
  items(bar, "", 0)
  let front = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "-"
  if let item = found.first { print("press", AXUIElementPerformAction(item, "AXPress" as CFString).rawValue) }
  usleep(800_000)
  print("field focused", attr(field, "AXFocused") ?? "-", "app focused element is field", attr(root, "AXFocusedUIElement").map { CFEqual($0, field) } ?? false)
  print("front before", front, "after", NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "-")
  exit(0)
}
if args.count < 3 || args[1] != "set" { print("rows", rows()); exit(0) }
let front = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "-"
let before = rows()
let status = AXUIElementSetAttributeValue(field, "AXValue" as CFString, args[2] as CFString)
print("set status", status.rawValue)
if args.count > 3 { print("confirm status", AXUIElementPerformAction(field, "AXConfirm" as CFString).rawValue) }
usleep(1_200_000)
print("value now", attr(field, "AXValue") ?? "-", "rows before", before, "after", rows())
print("front before", front, "after", NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "-")
