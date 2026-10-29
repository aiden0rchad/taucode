// @vitest-environment jsdom
import { cleanup, createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import type { AgentPolicy, AppSettings, EngineKind } from "@/types";
import { FeatureSettings } from "@settings/features";
import { defaultAgentPolicy, setAgentMode } from "@settings/registry";
import { ComposerPolicyProvider } from "./ComposerPolicy";
import { ReasoningControl } from "./ReasoningControl";

afterEach(cleanup);
const initial = setAgentMode(defaultAgentPolicy, "single");
function Picker({ save = async () => {}, engine = "pi", reasoning = true, enabled = true }: { save?: (policy: AgentPolicy) => Promise<void>; engine?: EngineKind; reasoning?: boolean; enabled?: boolean }) {
  const [policy, setPolicy] = useState(initial);
  return <FeatureSettings.Provider value={{ agentPolicy: policy, features: { subagents: enabled }, providers: [{ id: "local", name: "Local", enabled: true, kind: "ollama", auth: { type: "none" }, models: [{ id: "small", enabled: true }] }] } as AppSettings}>
    <ComposerPolicyProvider scope="Project · Demo" policy={policy} onChange={async next => { await save(next); setPolicy(next); }}>
      <ReasoningControl engine={engine} modelName="Commander" capability={{ supported: reasoning, levels: reasoning ? ["low", "high"] : [], default: "low", source: "api", control: "effort", detectedAt: 1 }} onChange={() => {}} />
    </ComposerPolicyProvider>
  </FeatureSettings.Provider>;
}
it("toggles real policy, exposes configuration only when enabled, and preserves role models", async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<Picker save={save} />);
  const chip = screen.getByRole("button", { name: "Commander · Low" });
  fireEvent.click(chip);
  const toggle = screen.getByRole("button", { name: "Chain of Command" });
  expect(toggle.getAttribute("aria-pressed")).toBe("false");
  expect(toggle.querySelectorAll("svg.lucide-star")).toHaveLength(5);
  expect(toggle.querySelector(".command-insignia")?.getAttribute("width")).toBe("20");
  expect(toggle.classList.contains("is-activating")).toBe(false);
  expect(screen.queryByRole("button", { name: "Configure Chain of Command" })).toBeNull();
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.getAttribute("aria-pressed")).toBe("true"));
  expect(toggle.classList.contains("is-activating")).toBe(true);
  expect([...toggle.querySelectorAll<SVGElement>(".command-star")].map(star => star.style.getPropertyValue("--star-index"))).toEqual(["0", "1", "2", "3", "4"]);
  const animationEnd = createEvent.animationEnd(toggle);
  Object.defineProperty(animationEnd, "animationName", { value: "command-aura-in" });
  fireEvent(toggle, animationEnd);
  expect(toggle.classList.contains("is-activating")).toBe(false);
  expect(toggle.getAttribute("aria-pressed")).toBe("true");
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ subagents: expect.objectContaining({ enabled: true, mode: "chain-of-command", maxDepth: 2 }) }));
  fireEvent.click(screen.getByRole("button", { name: "Configure Chain of Command" }));
  const editor = screen.getByRole("dialog", { name: "Configure Chain of Command" });
  expect(screen.queryByRole("dialog", { name: "Model and reasoning" })).toBeNull();
  expect(within(editor).getByText("Project · Demo")).toBeTruthy();
  expect(within(editor).getByText("Applies to new turns")).toBeTruthy();
  expect(within(editor).getByRole("button", { name: "Save changes" })).toHaveProperty("disabled", true);
  expect(within(editor).queryByLabelText("Agent mode")).toBeNull();
  expect(within(editor).queryByLabelText("Default engine")).toBeNull();
  fireEvent.change(await within(editor).findByLabelText("Enlisted model"), { target: { value: "local/small" } });
  fireEvent.click(within(editor).getByRole("button", { name: "Save changes" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(document.activeElement).toBe(chip);
  fireEvent.click(chip);
  expect(screen.getByRole("button", { name: "Chain of Command" }).classList.contains("is-activating")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Chain of Command" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Configure Chain of Command" })).toBeNull());
  expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ subagents: expect.objectContaining({ enabled: false, roles: expect.arrayContaining([expect.objectContaining({ name: "enlisted", model: { providerId: "local", modelId: "small" } })]) }) }));
});
it("keeps role drafts across keyboard navigation, validates limits, and discards cancelled edits", async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<Picker save={save} />);
  const open = async () => {
    fireEvent.click(screen.getByRole("button", { name: "Commander · Low" }));
    const toggle = screen.getByRole("button", { name: "Chain of Command" });
    if (toggle.getAttribute("aria-pressed") !== "true") {
      fireEvent.click(toggle);
      await waitFor(() => expect(toggle.getAttribute("aria-pressed")).toBe("true"));
    }
    fireEvent.click(screen.getByRole("button", { name: "Configure Chain of Command" }));
    await screen.findByLabelText("Colonel model");
    const body = screen.getByRole("tabpanel", { name: "Team models" });
    body.scrollTop = 200;
    fireEvent.keyDown(screen.getByRole("tab", { name: "Team models" }), { key: "End" });
    expect(body.scrollTop).toBe(0);
  };
  await open();
  fireEvent.change(screen.getByLabelText("Instructions"), { target: { value: "Delegate bounded work." } });
  fireEvent.keyDown(screen.getByRole("tab", { name: "Colonel" }), { key: "ArrowRight" });
  expect(screen.getByRole("tab", { name: "Major" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.change(screen.getByLabelText("Instructions"), { target: { value: "Report verification evidence." } });
  fireEvent.keyDown(screen.getByRole("tab", { name: "Major" }), { key: "ArrowLeft" });
  expect(screen.getByLabelText("Instructions")).toHaveProperty("value", "Delegate bounded work.");
  fireEvent.click(screen.getByRole("tab", { name: "Limits" }));
  fireEvent.change(screen.getByLabelText("Max concurrent"), { target: { value: "17" } });
  expect(screen.getByRole("alert").textContent).toContain("concurrent 1–16");
  expect(screen.getByRole("button", { name: "Save changes" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByLabelText("Max concurrent"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ subagents: expect.objectContaining({ maxConcurrent: 3, roles: expect.arrayContaining([
    expect.objectContaining({ name: "colonel", instructions: "Delegate bounded work." }),
    expect.objectContaining({ name: "major", instructions: "Report verification evidence." }),
  ]) }) }));
  await open();
  fireEvent.change(screen.getByLabelText("Instructions"), { target: { value: "Discard this edit." } });
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await open();
  expect(screen.getByLabelText("Instructions")).toHaveProperty("value", "Delegate bounded work.");
});
it("leaves the toggle off and reports failed saves", async () => {
  render(<Picker save={async () => { throw new Error("Disk unavailable"); }} />);
  fireEvent.click(screen.getByRole("button", { name: "Commander · Low" }));
  fireEvent.click(screen.getByRole("button", { name: "Chain of Command" }));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Disk unavailable");
  expect(screen.getByRole("button", { name: "Chain of Command" }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.getByRole("button", { name: "Chain of Command" }).classList.contains("is-activating")).toBe(false);
});
it("offers the controls for models without reasoning and keeps unsupported engines disabled", () => {
  const view = render(<Picker reasoning={false} engine="codex" />);
  fireEvent.click(screen.getByRole("button", { name: "Commander" }));
  expect(screen.getByRole("button", { name: "Chain of Command" })).toHaveProperty("disabled", true);
  view.rerender(<Picker reasoning={false} engine="omp" />);
  expect(screen.getByRole("button", { name: "Chain of Command" })).toHaveProperty("disabled", false);
  view.rerender(<Picker reasoning={false} enabled={false} />);
  expect(screen.queryByRole("button", { name: "Chain of Command" })).toBeNull();
});
