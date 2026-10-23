import test from "node:test";
import assert from "node:assert/strict";
import { connectorDecision, type Connector } from "../extensions/lib/connectors.ts";

test("browser read tools never ask; all mutations and unknown tools use approval", () => {
  const browser: Connector = {id:"taucode-browser",enabled:true,approval:"ask"};
  const decide = (name: string, mode="ask", interactive=true) => connectorDecision([browser],`mcp__taucode_browser__browser_${name}`,mode,interactive);
  for (const name of ["snapshot","take_screenshot","console_messages","network_requests","tabs","navigate","navigate_back","wait_for"]) {
    assert.equal(decide(name),"allow"); assert.equal(decide(name,"read-only",false),"allow");
  }
  for (const name of ["click","type","fill_form","select_option","drag","drop","file_upload","evaluate","run_code_unsafe","press_key","handle_dialog","set_cookie","set_storage","future_mutation"]) {
    assert.equal(decide(name),"ask"); assert.equal(decide(name,"full-access"),"ask"); assert.equal(decide(name,"read-only"),"block"); assert.equal(decide(name,"ask",false),"block");
  }
  browser.approval="auto"; assert.equal(decide("click"),"allow");
  browser.toolDenylist=["browser_snapshot"]; assert.equal(decide("snapshot"),"block");
});

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

test("computer actions use host policy while connector restrictions still apply", () => {
  const computer: Connector = { id: "taucode-computer", enabled: true, approval: "ask" };
  const decide = (tool: string, mode = "full-access") => connectorDecision([computer], `mcp__taucode_computer__${tool}`, mode, false);
  assert.equal(decide("computer_run_task"), "allow");
  assert.equal(decide("computer_run_task", "read-only"), "block");
  assert.equal(decide("computer_screenshot", "read-only"), "allow");
  computer.toolDenylist = ["computer_screenshot"];
  assert.equal(decide("computer_screenshot"), "block");
  computer.enabled = false;
  assert.equal(decide("computer_run_task"), "block");
});
