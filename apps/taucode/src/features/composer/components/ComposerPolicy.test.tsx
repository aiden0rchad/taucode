// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  expect(screen.queryByRole("button", { name: "Configure Chain of Command" })).toBeNull();
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.getAttribute("aria-pressed")).toBe("true"));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ subagents: expect.objectContaining({ enabled: true, mode: "chain-of-command", maxDepth: 2 }) }));
  fireEvent.click(screen.getByRole("button", { name: "Configure Chain of Command" }));
  const editor = screen.getByRole("dialog", { name: "Configure Chain of Command" });
  expect(screen.queryByRole("dialog", { name: "Model and reasoning" })).toBeNull();
  expect(within(editor).getByText("Project · Demo · Applies to new turns")).toBeTruthy();
  expect(within(editor).queryByLabelText("Agent mode")).toBeNull();
  expect(within(editor).queryByLabelText("Default engine")).toBeNull();
  fireEvent.change(await within(editor).findByLabelText("Enlisted model"), { target: { value: "local/small" } });
  fireEvent.click(within(editor).getByRole("button", { name: "Save changes" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(document.activeElement).toBe(chip);
  fireEvent.click(chip);
  fireEvent.click(screen.getByRole("button", { name: "Chain of Command" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Configure Chain of Command" })).toBeNull());
  expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ subagents: expect.objectContaining({ enabled: false, roles: expect.arrayContaining([expect.objectContaining({ name: "enlisted", model: { providerId: "local", modelId: "small" } })]) }) }));
});
it("leaves the toggle off and reports failed saves", async () => {
  render(<Picker save={async () => { throw new Error("Disk unavailable"); }} />);
  fireEvent.click(screen.getByRole("button", { name: "Commander · Low" }));
  fireEvent.click(screen.getByRole("button", { name: "Chain of Command" }));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Disk unavailable");
  expect(screen.getByRole("button", { name: "Chain of Command" }).getAttribute("aria-pressed")).toBe("false");
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
