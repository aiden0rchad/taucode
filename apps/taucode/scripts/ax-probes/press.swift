import ApplicationServices
import AppKit
// usage: press <timeoutSeconds>  — AXPress Preview's Open panel Cancel button
let check = CommandLine.arguments[1] == "check"
let timeout = check ? 1 : Float(CommandLine.arguments[1])!
print("trusted", AXIsProcessTrusted())
guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.Preview").first else { print("no preview"); exit(1) }
let root = AXUIElementCreateApplication(app.processIdentifier)
AXUIElementSetMessagingTimeout(root, 1)
func attr(_ e: AXUIElement, _ n: String) -> CFTypeRef? { var v: CFTypeRef?; return AXUIElementCopyAttributeValue(e, n as CFString, &v) == .success ? v : nil }
func find(_ e: AXUIElement, _ depth: Int) -> AXUIElement? {
  if depth > 12 { return nil }
  if (attr(e, "AXRole") as? String) == "AXButton", (attr(e, "AXIdentifier") as? String) == "CancelButton" || (attr(e, "AXTitle") as? String) == "Cancel" { return e }
  for c in (attr(e, "AXChildren") as? [AXUIElement]) ?? [] { if let f = find(c, depth + 1) { return f } }
  return nil
}
guard let button = find(root, 0) else { print("no cancel button"); exit(2) }
if check { print("cancel button present"); exit(0) }
AXUIElementSetMessagingTimeout(button, timeout)
if CommandLine.arguments.count > 2 { let d = Double(CommandLine.arguments[2])!; print("waiting", d); usleep(UInt32(d * 1_000_000)) }
let t = Date()
let r = AXUIElementPerformAction(button, "AXPress" as CFString)
print("timeout", timeout, "result", r.rawValue, "ms", Int(Date().timeIntervalSince(t) * 1000))
// After the press, poll the same handle's AXRole and print each status.
AXUIElementSetMessagingTimeout(button, 0.05)
for i in 0..<20 {
  usleep(30_000)
  var v: CFTypeRef?
  let s = AXUIElementCopyAttributeValue(button, "AXRole" as CFString, &v)
  print("poll", i, "status", s.rawValue)
}
