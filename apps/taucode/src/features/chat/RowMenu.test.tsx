// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { nativeOverlayIntersects } from "@/features/design-system/nativeOverlays";
import { RowMenu } from "./RowMenu";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it.each([false, true])("dismisses a portaled menu on Tab (shift=%s) and releases its native overlay", shiftKey => {
  const bounds = { x: 10, y: 10, width: 100, height: 100 };
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(bounds as DOMRect);
  render(<RowMenu label="Browser pane" portal actions={[{ label: "Move left", onClick: vi.fn() }]}><span /></RowMenu>);
  const trigger = screen.getByRole("button", { name: "Actions for Browser pane" });
  fireEvent.click(trigger);
  const item = screen.getByRole("menuitem", { name: "Move left" });
  expect(document.activeElement).toBe(item);
  expect(nativeOverlayIntersects(bounds)).toBe(true);
  expect(fireEvent.keyDown(item, { key: "Tab", shiftKey })).toBe(true);
  expect(screen.queryByRole("menu")).toBeNull();
  expect(document.activeElement).toBe(trigger);
  expect(nativeOverlayIntersects(bounds)).toBe(false);
});
