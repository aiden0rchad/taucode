// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Plus } from "lucide-react";
import { IconButton } from "@/features/design-system/components/IconButton";
import { assertRowAlignment } from "./rowAlignment.mjs";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const rect = (element: Element, y: number, width: number, height: number) =>
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue(new DOMRect(0, y, width, height));

it.each(["workspace-titlebar", "main-header", "sidebar-header", "sidebar-new-row", "sidebar-bottom-actions", "composer-shell-toolbar", "side-panel-tabbar", "settings-group-title", "setup-footer"])("audits %s centers and reports offending rects", className => {
  const { container } = render(<div className={className}><span>Title</span><IconButton aria-label="Add"><Plus /></IconButton></div>);
  rect(container.firstElementChild!, 0, 200, 32);
  rect(container.querySelector("span")!, 8, 100, 16);
  const button = container.querySelector("button")!;
  const measure = rect(button, 2, 28, 28);
  expect(assertRowAlignment(container)).toMatchObject({ rows: 1, icons: 1, unmeasuredRows: 0 });
  measure.mockReturnValue(new DOMRect(0, 3, 28, 28));
  expect(() => assertRowAlignment(container)).not.toThrow();
  measure.mockReturnValue(new DOMRect(0, 4, 28, 28));
  expect(() => assertRowAlignment(container)).toThrow(/Vertical centers/);
  measure.mockReturnValue(new DOMRect(0, 2, 155, 34));
  expect(() => assertRowAlignment(container)).toThrow(/155/);
});

it.each([24, 28, 32])("accepts %ipx squares but rejects other dimensions", size => {
  const { container } = render(<IconButton aria-label="Add"><Plus /></IconButton>);
  const measure = rect(container.querySelector("button")!, 0, size, size);
  expect(assertRowAlignment(container).icons).toBe(1);
  measure.mockReturnValue(new DOMRect(0, 0, size, size - 1));
  expect(() => assertRowAlignment(container)).toThrow(/square/);
});

it("ignores hidden rows and fixed popovers; distinguishes jsdom's unmeasured geometry", () => {
  const { container } = render(<><div hidden className="main-header"><button><Plus /></button></div><div className="sidebar-header"><IconButton aria-label="Add"><Plus /></IconButton><div style={{ position: "fixed" }}>Menu</div></div></>);
  expect(assertRowAlignment(container)).toEqual({ rows: 0, icons: 0, unmeasuredRows: 1, unmeasuredIcons: 1 });
  rect(container.querySelector(".sidebar-header")!, 0, 200, 32);
  rect(container.querySelector("[style]")!, 100, 200, 200);
  expect(() => assertRowAlignment(container)).not.toThrow();
});
