// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ComputerHud } from "./ComputerHud";
import { ComputerRunItem } from "./ComputerRunItem";
import { defaultComputerSettings, parseComputerRun } from "./computer";
import { SettingsComputerSection } from "@settings/components/sections/SettingsComputerSection";
import { computerRequest } from "@services/tauri";
import type { AppSettings } from "@/types";
vi.mock("@services/tauri", () => ({ computerRequest: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const step = { step: 1, thumbnail: "aGVsbG8=", action: { kind: "click" }, reflection: "Found button" };
describe("computer use", () => {
  it("renders persisted tool steps and validates unknown output", () => {
    const run = parseComputerRun(JSON.stringify({ computerRun: { steps: [step], summary: "Done" } }));
    expect(run).not.toBeNull(); expect(parseComputerRun('{"computerRun":{"steps":[{}]}}')).toBeNull();
    render(<ComputerRunItem run={run!} />);
    expect(screen.getByText("Step 1: click")).toBeTruthy(); expect(screen.getByAltText("Screen before step 1")).toBeTruthy(); expect(screen.getByText("Done")).toBeTruthy();
  });
  it("wires pause and stop plus one-action approval through the existing UI", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ running: true, steps: [step], approval: { id: "approval", reason: "Secure field", action: { kind: "type" }, app: "com.apple.TextEdit" } });
    render(<ComputerHud />);
    await screen.findByText("Step 2");
    fireEvent.click(screen.getByText("Pause")); expect(computerRequest).toHaveBeenCalledWith({ action: "pause", paused: true });
    fireEvent.click(screen.getByText("Stop", { selector: "button" })); expect(computerRequest).toHaveBeenCalledWith({ action: "stop" });
    fireEvent.click(screen.getByText("Approve (Enter)")); expect(computerRequest).toHaveBeenCalledWith({ action: "approve", id: "approval", accept: true });
    expect(screen.queryByText("Allow for session")).toBeNull();
  });
  it("shows registry models, coordinate presets and permission states", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions: { screen: true, accessibility: false } });
    const settings = { computer: defaultComputerSettings, providers: [{ id: "local", name: "Local", enabled: true, kind: "ollama", models: [{ id: "vision", enabled: true }] }] } as AppSettings;
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={settings} onUpdateAppSettings={save} />);
    await screen.findByText("Screen Recording: Granted"); expect(screen.getByText("Accessibility: Not granted")).toBeTruthy();
    expect(screen.getAllByText("Local / vision")).toHaveLength(2);
    fireEvent.change(screen.getByLabelText("Grounding coordinates"), { target: { value: "1000x1000" } });
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ groundingWidth: 1000, groundingHeight: 1000 }) })));
    fireEvent.click(screen.getByText("Open System Settings — Accessibility")); expect(computerRequest).toHaveBeenCalledWith({ action: "permissions", pane: "accessibility" });
  });
});
