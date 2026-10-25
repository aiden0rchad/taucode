import { execFileSync } from "node:child_process";
import { isAbsolute } from "node:path";
import { expect, it } from "vitest";
import { agentSnippets } from "./DiagnosticsSettings";

it("round-trips absolute paths through shell and TOML basic strings", () => {
  const bin = "/Users/test/Tau 'code'/bin";
  const dir = '/Users/test/Library/Application Support/with "quotes" and \\slashes/diagnostics';
  const snippets = agentSnippets(bin, dir);
  expect(snippets.claude + snippets.codex).not.toContain("~");
  expect(isAbsolute(bin) && isAbsolute(dir)).toBe(true);
  const args = execFileSync("/bin/sh", ["-c", 'set -- ' + snippets.claude + '; printf "%s\\n" "$@"'], { encoding: "utf8" }).trimEnd().split("\n");
  expect(args.slice(-3)).toEqual([bin, "--dir", dir]);
  const lines = snippets.codex.split("\n");
  expect(JSON.parse(lines[1].slice("command = ".length))).toBe(bin);
  expect(JSON.parse(lines[2].slice("args = ".length))).toEqual(["--dir", dir]);
  expect(() => agentSnippets("~/bin", dir)).toThrow("absolute paths");
  expect(() => agentSnippets(bin, "relative/diagnostics")).toThrow("absolute paths");
});
