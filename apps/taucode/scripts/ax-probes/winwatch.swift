import AppKit
// winwatch <seconds>: print Preview's on-screen window ids and x position whenever they change.
let pid = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.Preview").first!.processIdentifier
let front = { NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?" }
var last = ""
let end = Date().addingTimeInterval(Double(CommandLine.arguments[1])!)
while Date() < end {
  let list = (CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]]) ?? []
  let wins = list.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
    .map { w -> String in let b = w[kCGWindowBounds as String] as? [String: Any]; return "\(w[kCGWindowNumber as String]!)@x\(b?["X"] ?? "?") onscreen=\(w[kCGWindowIsOnscreen as String] as? Bool ?? false)" }
    .sorted().joined(separator: ",")
  let line = "\(wins) front=\(front())"
  if line != last { print(String(format: "%.1f", Date().timeIntervalSince1970.truncatingRemainder(dividingBy: 1000)), line); fflush(stdout); last = line }
  usleep(100_000)
}
