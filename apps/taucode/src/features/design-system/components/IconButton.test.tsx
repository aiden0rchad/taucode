// @vitest-environment jsdom
import { createRef } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Plus } from "lucide-react";
import { IconButton } from "./IconButton";
import { SettingsGroup, SettingsIconButton } from "./settings/SettingsPrimitives";

afterEach(cleanup);
it("forwards focus, native attributes and clicks without submitting forms", () => {
  const ref = createRef<HTMLButtonElement>(), click = vi.fn();
  render(<form><IconButton ref={ref} aria-label="Add" aria-pressed variant="active" onClick={click}><Plus /></IconButton></form>);
  const button = screen.getByRole("button", { name: "Add", pressed: true });
  expect(button.getAttribute("type")).toBe("button");
  ref.current?.focus();
  expect(document.activeElement).toBe(button);
  fireEvent.click(button);
  expect(click).toHaveBeenCalledOnce();
});
it("inherits disabled settings groups", () => {
  const click = vi.fn();
  render(<SettingsGroup title="Unavailable" disabled><SettingsIconButton aria-label="Remove" onClick={click}><Plus /></SettingsIconButton></SettingsGroup>);
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(click).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Remove" })).toHaveProperty("disabled", true);
});

// Keep the accessible name mandatory at the call site.
// @ts-expect-error Icon buttons must have an aria-label.
const unnamed = <IconButton><Plus /></IconButton>;
void unnamed;
