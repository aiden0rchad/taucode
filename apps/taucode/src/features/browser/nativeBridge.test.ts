// @vitest-environment jsdom
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import source from "../../../src-tauri/src/browser/native/bridge.js?raw";
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
