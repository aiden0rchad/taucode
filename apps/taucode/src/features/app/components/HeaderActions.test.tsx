// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { HeaderActions } from "./HeaderActions";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("renders nothing without actions", () => {
  const { container } = render(<HeaderActions>{false}{null}</HeaderActions>);
  expect(container.innerHTML).toBe("");
});
it("exposes only overflowing actions, and removes the trigger when the header grows", () => {
  let width = 800;
  let resize = () => {};
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => new DOMRect(0, 0, width, 32));
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: () => void) { resize = callback; }
    observe() {}
    disconnect() {}
  });
  const click = vi.fn();
  const { container } = render(<header className="main-header"><HeaderActions><button onClick={click}>Open editor</button><button>Copy thread</button></HeaderActions></header>);
  expect(screen.queryByRole("button", { name: "Header actions" })).toBeNull();
  act(() => { width = 260; resize(); });
  expect(container.querySelector(".has-overflow")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Header actions" }));
  expect(screen.getByRole("dialog", { name: "Header actions" }).textContent).toContain("Open editor");
  fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
  expect(click).toHaveBeenCalledOnce();
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Header actions" }));
  act(() => { width = 800; resize(); });
  expect(screen.queryByRole("button", { name: "Header actions" })).toBeNull();
  expect(screen.queryByRole("dialog")).toBeNull();
});
