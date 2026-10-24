// @vitest-environment jsdom
import { cleanup, render, screen, within, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ComposerMetaBar } from "./ComposerMetaBar";
import { engineRuntime } from "@services/tauri";
vi.mock("@services/tauri", () => ({ engineRuntime: vi.fn().mockResolvedValue([]) }));
afterEach(cleanup);
it("groups registry models, disables incompatible engines and shows the engine badge", () => {
  const props = {
    disabled: false, collaborationModes: [], selectedCollaborationModeId: null, onSelectCollaborationMode: vi.fn(),
    models: [
      { id: "a/model", model: "a/model", displayName: "A", providerName: "Anthropic", engines: ["pi"] as ("pi" | "codex")[] },
      { id: "b/model", model: "b/model", displayName: "B", providerName: "OpenAI", engines: ["pi", "codex"] as ("pi" | "codex")[] },
    ], selectedModelId: "b/model", onSelectModel: vi.fn(), reasoningOptions: [], selectedEffort: null, onSelectEffort: vi.fn(),
    selectedServiceTier: null, reasoningSupported: false, accessMode: "read-only" as const, onSelectAccessMode: vi.fn(),
  };
  const { rerender } = render(<ComposerMetaBar {...props} engine="codex" />);
  const option = within(screen.getByRole("group", { name: "Anthropic" })).getByRole("option") as HTMLOptionElement;
  expect(option.disabled).toBe(true);
  expect(option.title).toContain("Unavailable for codex");
  expect(screen.getByLabelText("Thread engine").textContent).toBe("codex");
  rerender(<ComposerMetaBar {...props} engine="pi" />);
  expect((screen.getByRole("option", { name: "A" }) as HTMLOptionElement).disabled).toBe(false);
  expect(screen.getByLabelText("Thread engine").textContent).toBe("pi");
});
it("shows Claude availability and limits an existing Claude thread to its provider models", async () => {
  const props = {
    disabled: false, collaborationModes: [], selectedCollaborationModeId: null, onSelectCollaborationMode: vi.fn(),
    models: [{ id: "a/model", model: "a/model", displayName: "A", providerId: "a", engines: ["pi", "claude"] as import("@/types").EngineKind[] }],
    selectedModelId: "a/model", onSelectModel: vi.fn(), reasoningOptions: [], selectedEffort: null, onSelectEffort: vi.fn(), selectedServiceTier: null,
    reasoningSupported: false, accessMode: "current" as const, onSelectAccessMode: vi.fn(),
  };
  const { rerender } = render(<ComposerMetaBar {...props} engine="claude" onSelectEngine={vi.fn()} />);
  expect((screen.getByRole("option", { name: "Claude Code (ACP)" }) as HTMLOptionElement).disabled).toBe(false);
  expect((screen.getByRole("option", { name: "A" }) as HTMLOptionElement).disabled).toBe(false);
  rerender(<ComposerMetaBar {...props} models={[]} engine="claude" onSelectEngine={vi.fn()} />);
  expect((screen.getByRole("option", { name: /Claude Code \(ACP\)/ }) as HTMLOptionElement).disabled).toBe(true);
  expect(screen.getByRole("option", { name: /Claude Code \(ACP\)/ }).textContent).toContain("Anthropic-compatible");
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "claude-acp", state: "installing", message: "Installing runtime" }]);
  rerender(<ComposerMetaBar {...props} engine="pi" onSelectEngine={vi.fn()} />);
  await waitFor(() => expect(screen.getByRole("option", { name: /Claude Code \(ACP\)/ }).textContent).toContain("Installing runtime"));
});
