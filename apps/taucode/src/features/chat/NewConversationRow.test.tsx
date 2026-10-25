// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { NewConversationRow } from "./NewConversationRow";
afterEach(cleanup);
it("uses the configured shortcut and exposes search without triggering new conversation", () => {
  const onNew = vi.fn(), onSearch = vi.fn();
  const { rerender } = render(<NewConversationRow label="New chat" onNew={onNew} onSearch={onSearch} shortcut="ctrl+n" handleShortcut />);
  fireEvent.keyDown(window, { key: "n", ctrlKey: true });
  expect(onNew).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "Search conversations" }));
  expect(onSearch).toHaveBeenCalledOnce();
  rerender(<NewConversationRow label="New chat" onNew={onNew} onSearch={onSearch} shortcut="ctrl+n" handleShortcut disabled />);
  fireEvent.keyDown(window, { key: "n", ctrlKey: true });
  expect(onNew).toHaveBeenCalledOnce();
});
