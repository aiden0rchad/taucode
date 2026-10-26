// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ComposerShell, ComposerSendButton, ComposerAccessControl } from "./ComposerShell";

afterEach(cleanup);
it("explains disabled send and switches to stop without losing the callback", () => {
  const onSend = vi.fn(), onStop = vi.fn();
  const { rerender } = render(<ComposerSendButton disabled disabledReason="Write a message" onSend={onSend} onStop={onStop} />);
  const send = screen.getByRole("button", { name: "Send" });
  expect(send).toHaveProperty("disabled", true);
  expect(send.title).toBe("Write a message");
  rerender(<ComposerSendButton running disabled disabledReason="Write a message" onSend={onSend} onStop={onStop} />);
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  expect(onStop).toHaveBeenCalledOnce();
  expect(onSend).not.toHaveBeenCalled();
  rerender(<ComposerSendButton disabledReason="Write a message" onSend={onSend} />);
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(onSend).toHaveBeenCalledOnce();
});

it("opens add controls, closes on selection and Escape, and grows the editor", () => {
  const attach = vi.fn();
  render(<ComposerShell menu={<button onClick={attach}>Attach files</button>} actions={null}><textarea aria-label="Message" /></ComposerShell>);
  const toggle = screen.getByRole("button", { name: "Add attachments and tools" });
  expect(screen.queryByText("Attach files")).toBeNull();
  fireEvent.click(toggle);
  const dialog = screen.getByRole("dialog", { name: "Attachments and tools" });
  expect(dialog.parentElement).toBe(document.body);
  expect(toggle.closest(".composer-shell")?.contains(dialog)).toBe(false);
  expect(dialog.style.position).toBe("fixed");
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Attach files" }));
  fireEvent.mouseDown(screen.getByRole("button", { name: "Attach files" }));
  expect(screen.getByRole("dialog")).toBe(dialog);
  fireEvent.click(screen.getByRole("button", { name: "Attach files" }));
  expect(attach).toHaveBeenCalledOnce();
  expect(screen.queryByText("Attach files")).toBeNull();
  fireEvent.click(toggle);
  fireEvent.keyDown(document, { key: "Escape" });
  expect(screen.queryByText("Attach files")).toBeNull();
  expect(screen.getByRole("textbox").style.height).toBe("48px");
});

it("keeps the compact model chip's full accessible name and reasoning level", () => {
  render(<ComposerShell menu={<button>Attach</button>} actions={null} reasoning={{ modelName: "gpt-5", capability: { supported: true, levels: ["low", "high"], default: "high", control: "effort", source: "user", detectedAt: 0 }, value: "high", onChange: vi.fn() }}><textarea aria-label="Message" /></ComposerShell>);
  const chip = screen.getByRole("button", { name: "gpt-5 · High" });
  expect(chip.querySelector(".reasoning-chip-model")?.textContent).toBe("gpt-5");
  expect(chip.querySelector(".reasoning-chip-dot")?.getAttribute("aria-hidden")).toBe("true");
  fireEvent.click(chip);
  expect(screen.getByRole("dialog", { name: "Model and reasoning" })).toBeTruthy();
});


it("releases the native access picker after choosing a mode", () => {
  const onChange = vi.fn();
  render(<ComposerAccessControl value="ask" onChange={onChange} />);
  const select = screen.getByRole("combobox", { name: "Access mode" });
  expect(select.closest("label")).toBeNull();
  select.focus();
  fireEvent.change(select, { target: { value: "full-access" } });
  expect(onChange).toHaveBeenCalledWith("full-access");
  expect(document.activeElement).not.toBe(select);
});
