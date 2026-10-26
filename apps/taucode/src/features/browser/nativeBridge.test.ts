// @vitest-environment jsdom
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import source from "../../../src-tauri/src/browser/native/bridge.js?raw";
import evaluateSource from "../../../src-tauri/src/browser/native/evaluate.js?raw";
const bridge = (params: Record<string, unknown>) => JSON.parse((window as unknown as { __tauBrowser: (p: unknown) => string }).__tauBrowser(params));
beforeEach(() => {
  delete (window as unknown as { __tauBrowser?: unknown }).__tauBrowser;
  document.body.innerHTML = '<main><h1>Example</h1><label for="name">Name</label><input id="name"/><button>Save</button></main>';
  vi.spyOn(Element.prototype, "getClientRects").mockReturnValue([{}] as unknown as DOMRectList);
  vi.stubGlobal("PointerEvent", MouseEvent);
  Element.prototype.scrollIntoView = vi.fn();
  window.eval(source);
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("produces stable refs, roles, names and bounds without DOM attributes", () => {
  const first = bridge({ op: "snapshot" }).result.nodes;
  expect(first.map((n: { role: string }) => n.role)).toEqual(["main", "heading", "textbox", "button"]);
  expect(first[2].name).toBe("Name");
  expect(first[2].bounds).toEqual({ x: 0, y: 0, w: 0, h: 0 });
  expect(bridge({ op: "snapshot" }).result.nodes[2].ref).toBe(first[2].ref);
  expect(document.querySelector('[data-taucode-ref]')).toBeNull();
});
it("clicks handlers, types using value setters and emits input/change", () => {
  const nodes = bridge({ op: "snapshot" }).result.nodes;
  const click = vi.fn(), input = vi.fn(), change = vi.fn();
  document.querySelector("button")!.addEventListener("click", click);
  const field = document.querySelector("input")!;
  field.addEventListener("input", input); field.addEventListener("change", change);
  expect(bridge({ op: "click", ref: nodes[3].ref }).error).toBeUndefined();
  expect(click).toHaveBeenCalledOnce();
  expect(bridge({ op: "type", ref: nodes[2].ref, text: "hello" }).error).toBeUndefined();
  expect(field.value).toBe("hello"); expect(input).toHaveBeenCalledOnce(); expect(change).toHaveBeenCalledOnce();
  field.remove(); expect(bridge({ op: "type", ref: nodes[2].ref, text: "bad" }).error).toContain("Stale");
});
it("caps nodes and encoded bytes and excludes password values", () => {
  document.body.innerHTML = '<input type="password" value="secret"/>' + '<button>Go</button>'.repeat(1200);
  let result = bridge({ op: "snapshot" }).result;
  expect(result.nodes).toHaveLength(1000); expect(result.truncated).toBe(true); expect(result.nodes[0].value).toBeUndefined();
  document.body.innerHTML = `<button>${"界".repeat(1000)}</button>`.repeat(200);
  result = bridge({ op: "snapshot" }).result;
  expect(new TextEncoder().encode(JSON.stringify(result)).length).toBeLessThan(200000);
  expect(result.truncated).toBe(true);
});
it("selects options, edits with common keys, and rejects unknown options", () => {
  document.body.innerHTML = '<input value="abc"/><select><option value="one">One</option><option value="two">Two</option></select>';
  const nodes = bridge({ op: "snapshot" }).result.nodes;
  const input = document.querySelector("input")!; input.focus(); input.setSelectionRange(3, 3);
  bridge({ op: "press", key: "Backspace" }); expect(input.value).toBe("ab");
  bridge({ op: "press", key: "ControlOrMeta+a" }); bridge({ op: "press", key: "x" }); expect(input.value).toBe("x");
  bridge({ op: "select", ref: nodes[1].ref, values: ["two"] }); expect(document.querySelector("select")!.value).toBe("two");
  expect(bridge({ op: "select", ref: nodes[1].ref, values: ["missing"] }).error).toContain("Unknown option");
});
it("resolves unique CSS selectors and rejects ambiguous selectors and stale refs", () => {
  const click = vi.fn(); document.querySelector("button")!.addEventListener("click", click);
  expect(bridge({ op: "click", ref: "button" }).error).toBeUndefined();
  expect(click).toHaveBeenCalledOnce();
  document.body.insertAdjacentHTML("beforeend", "<button>Other</button>");
  expect(bridge({ op: "click", ref: "button" }).error).toContain("matched 2");
  expect(bridge({ op: "click", ref: "e999" }).error).toContain("Stale");
  expect(bridge({ op: "type", ref: "#name", text: "Fern" }).error).toBeUndefined();
  expect(document.querySelector<HTMLInputElement>("#name")!.value).toBe("Fern");
});
it("fills the advertised textbox, checkbox, radio, combobox and slider fields", () => {
  document.body.innerHTML = '<input id="user"/><input id="pass" type="password"/><input id="check" type="checkbox"/><input id="radio" type="radio"/><select id="choice"><option value="sun">Sunny</option><option value="shade">Shady</option></select><input id="range" type="range"/>';
  const input = vi.fn(), change = vi.fn();
  document.body.addEventListener("input", input); document.body.addEventListener("change", change);
  const fields = [
    { target: "#user", name: "Username", type: "textbox", value: "demo" },
    { target: "#pass", name: "Password", type: "textbox", value: "demo123" },
    { target: "#check", name: "Remember", type: "checkbox", value: "true" },
    { target: "#radio", name: "Choice", type: "radio", value: "true" },
    { target: "#choice", name: "Light", type: "combobox", value: "Shady" },
    { target: "#range", name: "Amount", type: "slider", value: "25" },
  ];
  expect(bridge({ op: "fill", fields }).error).toBeUndefined();
  expect([...document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("input,select")].map(el => el.value)).toEqual(["demo", "demo123", "on", "on", "shade", "25"]);
  expect(document.querySelector<HTMLInputElement>("#check")!.checked).toBe(true);
  expect(document.querySelector<HTMLInputElement>("#radio")!.checked).toBe(true);
  expect(input).toHaveBeenCalledTimes(6); expect(change).toHaveBeenCalledTimes(6);
  const nodes = bridge({ op: "snapshot" }).result.nodes;
  expect(nodes[1].value).toBeUndefined();
  expect(bridge({ op: "fill", fields: [{ ...fields[0], target: nodes[0].ref, value: "updated" }] }).error).toBeUndefined();
  expect(document.querySelector<HTMLInputElement>("#user")!.value).toBe("updated");
});
it("validates every field before filling and rejects disabled/read-only/non-text inputs", () => {
  document.body.innerHTML = '<input id="first" value="unchanged"/><fieldset disabled><input id="disabled"/></fieldset><input id="readonly" readonly/><input id="file" type="file"/><button id="button">Save</button>';
  const first = { target: "#first", type: "textbox", value: "changed" };
  for (const target of ["#disabled", "#readonly", "#file", "#button"]) {
    expect(bridge({ op: "fill", fields: [first, { ...first, target }] }).error).toBeDefined();
    expect(document.querySelector<HTMLInputElement>("#first")!.value).toBe("unchanged");
    expect(bridge({ op: "type", ref: target, text: "bad" }).error).toBeDefined();
  }
  expect(bridge({ op: "fill", fields: [{ ...first, type: "unknown" }] }).error).toContain("Unsupported field");
});

const pageEvaluate = async (fn: unknown, url = location.href, marker: string | null = null) => JSON.parse(await window.eval(`(${evaluateSource})`)(() => fn, url, marker));
it("awaits page evaluation and returns JSON, undefined and errors safely", async () => {
  expect(await pageEvaluate(async () => ({ title: document.title, count: 3 }))).toEqual({ result: { title: document.title, count: 3 } });
  expect(await pageEvaluate(() => undefined)).toEqual({ result: null });
  expect(await pageEvaluate(async () => { throw new Error("rejected"); })).toEqual({ error: "rejected" });
  expect((await pageEvaluate(42)).error).toContain("Expected a JavaScript function");
  expect((await pageEvaluate(() => document.body)).error).toContain("DOM nodes");
  const cyclic: { self?: unknown } = {}; cyclic.self = cyclic;
  expect((await pageEvaluate(() => cyclic)).error).toBeDefined();
  expect((await pageEvaluate(() => BigInt(1))).error).toBeDefined();
  expect((await pageEvaluate(() => "界".repeat(70000))).error).toContain("200 KB");
});
it("carries a private snapshot target into evaluation without leaving DOM markers", async () => {
  const ref = bridge({ op: "snapshot" }).result.nodes[2].ref;
  const marker = "data-taucode-evaluate-12345678-1234-1234-1234-123456789abc";
  expect(bridge({ op: "mark", ref, marker }).error).toBeUndefined();
  expect(await pageEvaluate((el: HTMLInputElement) => { expect(el.hasAttribute(marker)).toBe(false); return el.id; }, location.href, marker)).toEqual({ result: "name" });
  expect(document.querySelector(`[${marker}]`)).toBeNull();
  bridge({ op: "mark", ref, marker });
  expect((await pageEvaluate(() => null, "https://different.example", marker)).error).toContain("Page navigated");
  bridge({ op: "unmark", marker }); expect(document.querySelector(`[${marker}]`)).toBeNull();
  expect(bridge({ op: "mark", ref, marker: "bad]" }).error).toContain("Invalid evaluation marker");
});

it("submits a filled login form and refuses evaluation after a URL change", async () => {
  document.body.innerHTML = '<form><input id="user"/><input id="pass" type="password"/><button>Log in</button></form>';
  const submit = vi.fn((e: Event) => e.preventDefault()); document.querySelector("form")!.addEventListener("submit", submit);
  expect(bridge({ op: "type", ref: "#user", text: "demo" }).error).toBeUndefined();
  expect(bridge({ op: "type", ref: "#pass", text: "demo123", submit: true }).error).toBeUndefined();
  expect(submit).toHaveBeenCalledOnce();
  const createFunction = vi.fn(() => () => "unexpected");
  const result = JSON.parse(await window.eval(`(${evaluateSource})`)(createFunction, "https://elsewhere.example", null));
  expect(result.error).toContain("Page navigated"); expect(createFunction).not.toHaveBeenCalled();
});

it("refuses selectors and form writes if the page changed after origin validation", () => {
  const fields = [{ target: "#name", type: "textbox", value: "wrong page" }];
  expect(bridge({ op: "fill", fields, expectedUrl: "https://previous.example" }).error).toContain("Page navigated before action");
  expect(bridge({ op: "type", ref: "#name", text: "wrong page", expectedUrl: "https://previous.example" }).error).toContain("Page navigated before action");
  expect(document.querySelector<HTMLInputElement>("#name")!.value).toBe("");
  expect(bridge({ op: "fill", fields, expectedUrl: location.href }).error).toBeUndefined();
  expect(document.querySelector<HTMLInputElement>("#name")!.value).toBe("wrong page");
});
