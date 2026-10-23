import test from "node:test";
import assert from "node:assert/strict";
import { connectorDecision, type Connector } from "../extensions/lib/connectors.ts";

test("connector allow/deny and approval rules apply across access modes", () => {
  const server: Connector = { id: "test-server", enabled: true, approval: "ask", toolAllowlist: ["read", "write"], toolDenylist: ["write"] };
  const decide = (tool: string, mode = "full-access", interactive = true) => connectorDecision([server], `mcp__test_server__${tool}`, mode, interactive);
  assert.equal(decide("read"), "ask");
  assert.equal(decide("write"), "block");
  assert.equal(decide("unknown"), "block");
  assert.equal(decide("read", "read-only"), "block");
  assert.equal(decide("read", "ask", false), "block");
  server.approval = "auto";
  assert.equal(decide("read", "ask"), "allow");
  server.enabled = false;
  assert.equal(decide("read"), "block");
  assert.equal(connectorDecision([], "mcp__unknown__read", "full-access", true), "block");
});
