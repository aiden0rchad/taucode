import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { JSDOM } from "jsdom";
import source from "../../../src-tauri/src/browser/native/page_hooks.js?raw";
import evaluateSource from "../../../src-tauri/src/browser/native/evaluate.js?raw";
let page, hooks;
const key = "test-page-hooks";
let originals;
beforeEach(() => {
  page = new JSDOM("", { url: "https://example.test/", runScripts: "outside-only" }).window;
  page.TextEncoder = TextEncoder;
  originals = ["log", "info", "warn", "error", "debug"].map((type) => {
    const original = vi.fn();
    Object.defineProperty(page.console, type, { value: original, configurable: true, writable: true });
    return original;
  });
  page.eval(`(${source})(${JSON.stringify(key)})`);
  hooks = page[key];
});
afterEach(() => page.close());
it("installs once per main document and leaves subframes untouched", () => {
  page.eval(`(${source})(${JSON.stringify(key)})`);
  page.console.log("once");
  expect(hooks.messages).toHaveLength(1);
  const frame = page.document.createElement("iframe");
  page.document.body.append(frame);
  frame.contentWindow.eval(`(${source})(${JSON.stringify(key)})`);
  expect(frame.contentWindow[key]).toBeUndefined();
});
it("captures console methods while calling originals and hides immutable state", () => {
  page.console.log("hello", { count: 2 });
  page.console.info("info");
  page.console.warn("warning");
  page.console.error("error");
  page.console.debug("debug");
  expect(hooks.messages.map((entry) => entry.type)).toEqual(["log", "info", "warn", "error", "debug"]);
  expect(hooks.messages[0]).toEqual({ type: "log", text: 'hello {"count":2}', timestamp: expect.any(Number) });
  for (const original of originals)
    expect(original).toHaveBeenCalledOnce();
  expect(Object.keys(page)).not.toContain(key);
  expect(Object.getOwnPropertyDescriptor(page, key)).toMatchObject({ enumerable: false, writable: false, configurable: false });
  expect(Object.isFrozen(hooks.messages)).toBe(true);
  expect(page.__tauBrowser).toBeUndefined();
});
it("bounds console entries and survives circular or hostile values", () => {
  for (let i = 0; i < 505; i++)
    page.console.log(String(i));
  expect(hooks.messages).toHaveLength(500);
  expect(hooks.messages[0].text).toBe("5");
  page.console.log("x".repeat(3000));
  expect(hooks.messages[hooks.messages.length - 1]?.text).toHaveLength(2000);
  const circular = {};
  circular.self = circular;
  expect(() => page.console.log(circular)).not.toThrow();
  expect(() => page.console.log({ toJSON() { throw Error(); }, toString() { throw Error(); } })).not.toThrow();
  expect(hooks.messages[hooks.messages.length - 1]?.text).toBe("[unprintable]");
});
it("records uncaught errors with their source and unhandled rejections", () => {
  page.dispatchEvent(new page.ErrorEvent("error", { message: "broken", filename: "app.js", lineno: 42 }));
  const rejection = new page.Event("unhandledrejection");
  Object.defineProperty(rejection, "reason", { value: new Error("rejected") });
  page.dispatchEvent(rejection);
  expect(hooks.messages.map((entry) => [entry.type, entry.text])).toEqual([
    ["error", "broken (app.js:42)"], ["error", "Unhandled promise rejection: rejected"],
  ]);
});
it("filters levels cumulatively and supports all and onlyErrors", () => {
  page.console.log("log");
  page.console.info("info");
  page.console.warn("warn");
  page.console.error("error");
  page.console.debug("debug");
  const types = (params = {}) => hooks.request({ action: "console", ...params }).map((entry) => entry.type);
  expect(types()).toEqual(["log", "info", "warn", "error"]);
  expect(types({ level: "error" })).toEqual(["error"]);
  expect(types({ level: "warning" })).toEqual(["warn", "error"]);
  expect(types({ level: "info" })).toEqual(types());
  expect(types({ level: "debug" })).toEqual(["log", "info", "warn", "error", "debug"]);
  expect(types({ all: true })).toEqual(types());
  expect(types({ all: true, level: "error" })).toEqual(["error"]);
  expect(types({ all: true, onlyErrors: true })).toEqual(["error"]);
  expect(() => types({ level: "bogus" })).toThrow("Invalid console level");
});
it("dismisses unarmed dialogs and reports a one-time snapshot notice", () => {
  expect(page.confirm("Clear all?\nReally?")).toBe(false);
  expect(hooks.dialogs[0]).toMatchObject({ type: "confirm", message: "Clear all?\nReally?", defaultValue: "", answered: false, timestamp: expect.any(Number) });
  const note = hooks.request({ action: "snapshot" });
  expect(note).toContain("dismissed automatically");
  expect(note).not.toContain("\n");
  expect(hooks.request({ action: "snapshot" })).toBe("");
  expect(page.prompt("Name", "default")).toBeNull();
  expect(page.alert("Notice")).toBeUndefined();
  expect(hooks.dialogs.map((entry) => entry.type)).toEqual(["confirm", "prompt", "alert"]);
});
it("consumes armed answers once for confirm, prompt and alert", () => {
  page.confirm("Clear all?");
  expect(hooks.request({ action: "dialog", accept: true })).toContain('confirm("Clear all?") was dismissed automatically. The next dialog will be accepted; repeat the action that opens it');
  expect(page.confirm("Clear all?")).toBe(true);
  expect(hooks.dialogs[hooks.dialogs.length - 1]).toMatchObject({ answered: true, accepted: true });
  expect(hooks.request({ action: "snapshot" })).toBe("");
  expect(page.confirm("Again?")).toBe(false);
  hooks.request({ action: "dialog", accept: true, promptText: "answer" });
  expect(page.prompt("Name", "default")).toBe("answer");
  expect(page.prompt("Name", "default")).toBeNull();
  hooks.request({ action: "dialog", accept: true });
  expect(page.prompt("Name", "default")).toBe("default");
  hooks.request({ action: "dialog", accept: true, promptText: "" });
  expect(page.prompt("Name", "default")).toBe("");
  hooks.request({ action: "dialog", accept: false });
  expect(page.prompt("Name", "default")).toBeNull();
  hooks.request({ action: "dialog", accept: true });
  expect(page.alert("Notice")).toBeUndefined();
  expect(page.confirm("After alert")).toBe(false);
  expect(hooks.request({ action: "dialog", accept: false })).toContain("next dialog will be dismissed");
  expect(page.confirm("No")).toBe(false);
  expect(hooks.dialogs[hooks.dialogs.length - 1]).toMatchObject({ answered: true, accepted: false });
});
it("caps dialog history, messages, defaults and supplied answers", () => {
  for (let i = 0; i < 55; i++)
    page.alert(String(i));
  expect(hooks.dialogs).toHaveLength(50);
  expect(hooks.dialogs[0].message).toBe("5");
  hooks.request({ action: "dialog", accept: true, promptText: "a".repeat(3000) });
  expect(page.prompt("m".repeat(3000), "d".repeat(3000))).toHaveLength(2000);
  expect(hooks.dialogs[hooks.dialogs.length - 1]?.message).toHaveLength(2000);
  expect(hooks.dialogs[hooks.dialogs.length - 1]?.defaultValue).toHaveLength(2000);
  expect(() => hooks.request({ action: "dialog" })).toThrow("accept boolean");
  expect(() => hooks.request({ action: "dialog", accept: true, promptText: {} })).toThrow("promptText");
});
it("uses the page evaluation origin guard and 200 KB JSON result cap", async () => {
  const evaluate = page.eval(`(${evaluateSource})`);
  const call = (url = page.location.href) => evaluate(() => () => hooks.request({ action: "console", all: true }), url, null).then(JSON.parse);
  page.console.error("test");
  expect(await call()).toMatchObject({ result: [{ type: "error", text: "test" }] });
  expect(await call("https://different.test/")).toMatchObject({ error: expect.stringContaining("Page navigated") });
  for (let i = 0; i < 500; i++)
    page.console.log("x".repeat(2000));
  expect(await call()).toMatchObject({ error: "Evaluation result exceeds 200 KB" });
});
it("keeps page dialogs when the Tauri dialog plugin script runs afterwards", () => {
  // Real plugin init script (tauri-plugin-dialog 2.6.0); its async IPC dialogs fail in browser tabs.
  const plugin = "!function(){\"use strict\";async function n(n,i={},o){return window.__TAURI_INTERNALS__.invoke(n,i,o)}\"function\"==typeof SuppressedError&&SuppressedError,window.alert=function(i){n(\"plugin:dialog|message\",{message:i.toString()})},window.confirm=async function(i){return await n(\"plugin:dialog|confirm\",{message:i.toString()})}}();";
  page.__TAURI_INTERNALS__ = { invoke: async () => { throw new Error("dialog.confirm not allowed"); } };
  // WebKit runs each init script separately; the plugin's strict assignment fails alone.
  expect(() => page.eval(plugin)).toThrow(/read only/);
  expect(page.confirm("Clear all?")).toBe(false);
  hooks.request({ action: "dialog", accept: true });
  expect(page.confirm("Clear all?")).toBe(true);
  expect(page.alert("hi")).toBeUndefined();
});

it("hides Tauri plugin IPC denials but keeps real page rejections", () => {
  const fire = (reason) => {
    const event = new page.Event("unhandledrejection", { cancelable: true });
    Object.defineProperty(event, "reason", { value: reason });
    page.dispatchEvent(event);
    return event.defaultPrevented;
  };
  expect(fire("notification.is_permission_granted not allowed")).toBe(true);
  expect(fire(new Error("dialog.confirm not allowed on window main"))).toBe(true);
  expect(fire(new Error("Failed to fetch"))).toBe(false);
  expect(hooks.messages.map((entry) => entry.text)).toEqual(["Unhandled promise rejection: Failed to fetch"]);
});
