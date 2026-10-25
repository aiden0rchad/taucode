// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useState } from "react";
import { SettingsButton, SettingsFieldRow, SettingsGroup, SettingsSelect, SettingsSegmented, SettingsTextField, SettingsToggleSwitch } from "./SettingsPrimitives";

afterEach(cleanup);

it("shows concise labels, full tooltips and rich options with keyboard and pointer selection", () => {
  function Example() {
    const [value, setValue] = useState("background");
    return <SettingsFieldRow title="Input mode"><SettingsSelect value={value} onChange={event => setValue(event.target.value)}><option value="background" data-description="You can keep using your Mac." data-recommended>Background</option><option value="disabled" disabled>Unavailable</option><option value="foreground" data-description="Takes over the mouse.">Foreground</option></SettingsSelect></SettingsFieldRow>;
  }
  render(<Example />);
  const select = screen.getByRole("combobox", { name: "Input mode" }) as HTMLSelectElement;
  expect(select.title).toBe("Background — Recommended — You can keep using your Mac.");
  fireEvent.mouseDown(select);
  const list = screen.getByRole("listbox");
  expect(within(list).getByText("Recommended")).toBeTruthy();
  expect(within(list).getByText("You can keep using your Mac.")).toBeTruthy();
  fireEvent.keyDown(select, { key: "ArrowDown" });
  fireEvent.keyDown(select, { key: "Enter" });
  expect(select.value).toBe("foreground");
  expect(screen.queryByRole("listbox")).toBeNull();
  fireEvent.mouseDown(select);
  fireEvent.click(within(screen.getByRole("listbox")).getByRole("option", { name: /Background/ }));
  expect(select.value).toBe("background");
  fireEvent.keyDown(select, { key: "End" });
  fireEvent.keyDown(select, { key: "Escape" });
  expect(select.value).toBe("background");
  expect(screen.queryByRole("listbox")).toBeNull();
  fireEvent.mouseDown(select);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("listbox")).toBeNull();
});

it("shows the complete selected label while preserving native selection and its accessible label", () => {
  function Example() {
    const [value, setValue] = useState("background");
    return <SettingsFieldRow title="Input mode"><SettingsSelect value={value} onChange={event => setValue(event.target.value)}><option value="background">Background (recommended): you can keep using your Mac</option><option value="foreground">Foreground: takes over the mouse</option></SettingsSelect></SettingsFieldRow>;
  }
  render(<Example />);
  const select = screen.getByRole("combobox", { name: "Input mode" });
  expect(screen.getByText("Background (recommended): you can keep using your Mac", { selector: ".settings-select-label" })).toBeTruthy();
  fireEvent.change(select, { target: { value: "foreground" } });
  expect(screen.getByText("Foreground: takes over the mouse", { selector: ".settings-select-label" })).toBeTruthy();
});

it("keeps nested controls disabled when a group is busy without native fieldsets", () => {
  const click = vi.fn();
  const { container } = render(<SettingsGroup title="Busy" disabled><SettingsTextField aria-label="Name" /><SettingsGroup><SettingsSelect aria-label="Model"><option>Default</option></SettingsSelect><SettingsToggleSwitch aria-label="Enable" pressed onClick={click} /><SettingsButton onClick={click}>Save</SettingsButton></SettingsGroup></SettingsGroup>);
  expect(container.querySelector("fieldset")).toBeNull();
  for (const control of container.querySelectorAll("input, select, button")) expect(control.hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(click).not.toHaveBeenCalled();
});

it("moves tab selection and focus with the keyboard, skipping disabled tabs", () => {
  function Example() {
    const [value, setValue] = useState("defaults");
    return <SettingsSegmented tabs label="Models & Agents" value={value} onChange={setValue} options={[{ value: "defaults", label: "Defaults" }, { value: "roles", label: "Roles", disabled: true }, { value: "engines", label: "Engines" }]} />;
  }
  render(<Example />);
  fireEvent.keyDown(screen.getByRole("tab", { name: "Defaults" }), { key: "ArrowRight" });
  expect(screen.getByRole("tab", { name: "Engines", selected: true })).toBe(document.activeElement);
  fireEvent.keyDown(document.activeElement!, { key: "Home" });
  expect(screen.getByRole("tab", { name: "Defaults", selected: true })).toBe(document.activeElement);
});
