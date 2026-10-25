// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useState } from "react";
import { SettingsButton, SettingsFieldRow, SettingsGroup, SettingsSelect, SettingsSegmented, SettingsTextField, SettingsToggleSwitch } from "./SettingsPrimitives";

afterEach(cleanup);

it("shows the complete selected label while preserving native selection and its accessible label", () => {
  function Example() {
    const [value, setValue] = useState("background");
    return <SettingsFieldRow title="Input mode"><SettingsSelect value={value} onChange={event => setValue(event.target.value)}><option value="background">Background (recommended): you can keep using your Mac</option><option value="foreground">Foreground: takes over the mouse</option></SettingsSelect></SettingsFieldRow>;
  }
  render(<Example />);
  const select = screen.getByRole("combobox", { name: "Input mode" });
  expect(screen.getByText("Background (recommended): you can keep using your Mac", { selector: ".settings-select-value" })).toBeTruthy();
  fireEvent.change(select, { target: { value: "foreground" } });
  expect(screen.getByText("Foreground: takes over the mouse", { selector: ".settings-select-value" })).toBeTruthy();
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
