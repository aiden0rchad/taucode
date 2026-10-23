export type Connector = { id: string; enabled: boolean; toolAllowlist?: string[] | null; toolDenylist?: string[] | null; approval: "auto" | "ask" };

export function connectorDecision(servers: Connector[], tool: string, mode: string, interactive: boolean): "allow" | "ask" | "block" {
  const server = servers.filter(s => tool.startsWith(`mcp__${s.id.replaceAll("-", "_")}__`)).sort((a, b) => b.id.length - a.id.length)[0];
  if (!server?.enabled) return "block";
  const name = tool.slice(`mcp__${server.id.replaceAll("-", "_")}__`.length);
  const normalize = (s: string) => s.replace(/[^a-zA-Z0-9_]/g, "_");
  if (server.toolDenylist?.some(t => normalize(t) === name || name.startsWith(`${normalize(t)}_`)) || (server.toolAllowlist && !server.toolAllowlist.some(t => normalize(t) === name))) return "block";
  if (server.id === "taucode-browser" && ["browser_snapshot", "browser_take_screenshot", "browser_console_messages", "browser_network_requests", "browser_tabs", "browser_navigate", "browser_navigate_back", "browser_wait_for"].includes(name)) return "allow";
  if (mode === "read-only") return "block";
  return server.approval === "ask" ? (interactive ? "ask" : "block") : "allow";
}
