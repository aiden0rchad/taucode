// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RenameThreadPrompt } from "./RenameThreadPrompt";

afterEach(cleanup);

describe("RenameThreadPrompt", () => {
  it("contains Tab navigation, handles Escape from buttons, and restores the trigger", () => {
    const trigger = document.createElement("button");
    document.body.append(trigger);
    trigger.focus();
    const onCancel = vi.fn();
    const { unmount } = render(<RenameThreadPrompt currentName="Old" name="New" onChange={vi.fn()} onCancel={onCancel} onConfirm={vi.fn()} />);
    const input = screen.getByLabelText("New name");
    const submit = screen.getByRole("button", { name: "Rename" });
    fireEvent.keyDown(input, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(submit);
    fireEvent.keyDown(submit, { key: "Tab" });
    expect(document.activeElement).toBe(input);
    screen.getByRole("button", { name: "Cancel" }).focus();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
  it("blocks duplicate submissions while saving and restores focus after failure", () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    const props = { currentName: "Old name", name: "New name", onChange: vi.fn(), onCancel, onConfirm, title: "Rename task", maxLength: 200 };
    const { baseElement, rerender } = render(<RenameThreadPrompt {...props} busy />);
    const input = screen.getByLabelText("New name");
    expect(input).toHaveProperty("readOnly", true);
    expect(input).toHaveProperty("maxLength", 200);
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.keyDown(input, { key: "Escape" });
    fireEvent.click(baseElement.querySelector(".ds-modal-backdrop")!);
    expect(onCancel).not.toHaveBeenCalled();
    expect(onConfirm).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Renaming…" })).toHaveProperty("disabled", true);
    rerender(<RenameThreadPrompt {...props} error="Could not save" />);
    expect(screen.getByRole("alert").textContent).toBe("Could not save");
    expect(input.getAttribute("aria-describedby")).toBe(screen.getByRole("alert").id);
    expect(input).toBe(document.activeElement);
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(onConfirm).not.toHaveBeenCalled();
    rerender(<RenameThreadPrompt {...props} name="   " />);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onConfirm).not.toHaveBeenCalled();
  });
  it("handles backdrop and keyboard actions", () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    const { baseElement } = render(
      <RenameThreadPrompt
        currentName="Old name"
        name="New name"
        onChange={vi.fn()}
        onCancel={onCancel}
        onConfirm={onConfirm}
      />,
    );

    const input = screen.getByLabelText("New name");
    fireEvent.keyDown(input, { key: "Escape" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledTimes(1);

    const backdrop = baseElement.querySelector(".ds-modal-backdrop");
    expect(backdrop).toBeTruthy();
    if (!backdrop) {
      throw new Error("Expected rename thread backdrop");
    }
    fireEvent.click(backdrop);
    expect(onCancel).toHaveBeenCalledTimes(2);
  });
});
