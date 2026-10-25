// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, ProviderEntry } from "@/types";
import { SettingsProvidersSection, SettingsAgentPolicySection } from "./SettingsRegistrySections";
import { defaultAgentPolicy } from "../../registry";
import { providerSetApiKey, providerHasApiKey, providerTestAndFetch } from "@services/tauri";
vi.mock("@services/tauri", () => ({
  providerSetApiKey: vi.fn(), providerHasApiKey: vi.fn(), providerDeleteApiKey: vi.fn(), providerTestAndFetch: vi.fn(),
}));
const provider: ProviderEntry = { id: "local", name: "Local", kind: "ollama", auth: { type: "none" }, enabled: true, models: [{ id: "small", enabled: true }] };
function Providers({ initial = [] }: { initial?: ProviderEntry[] }) {
  const [settings, setSettings] = useState({ providers: initial } as AppSettings);
  return <SettingsProvidersSection appSettings={settings} onUpdateAppSettings={async s => { setSettings(s); }} />;
}
describe("provider registry settings", () => {
  afterEach(cleanup);
  beforeEach(() => { vi.resetAllMocks(); vi.mocked(providerHasApiKey).mockResolvedValue(false); });
  it("saves Anthropic compatibility and enables the Claude default engine", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { rerender } = render(<SettingsProvidersSection appSettings={{ providers: [provider] } as AppSettings} onUpdateAppSettings={save} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit Local" }));
    fireEvent.click(screen.getByLabelText("Anthropic-compatible (/v1/messages)"));
    fireEvent.click(screen.getByText("Save provider"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ providers: [expect.objectContaining({ anthropicCompatible: true })] })));
    rerender(<SettingsAgentPolicySection appSettings={{ providers: [{ ...provider, anthropicCompatible: true }] } as AppSettings} onUpdateAppSettings={save} />);
    expect((screen.getByRole("option", { name: "Claude Code (ACP)" }) as HTMLOptionElement).disabled).toBe(false);
    fireEvent.change(screen.getByLabelText("Default engine"), { target: { value: "claude" } });
    fireEvent.click(screen.getByText("Save policy"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ agentPolicy: expect.objectContaining({ defaultEngine: "claude" }) })));
  });
  it("defaults auth commands to 15 seconds and saves a provider override", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const commandProvider: ProviderEntry = { ...provider, auth: { type: "command", command: "/bin/key", args: [] } };
    render(<SettingsProvidersSection appSettings={{ providers: [commandProvider] } as AppSettings} onUpdateAppSettings={save} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit Local" }));
    expect((screen.getByLabelText("Auth timeout (ms)") as HTMLInputElement).value).toBe("15000");
    fireEvent.change(screen.getByLabelText("Auth timeout (ms)"), { target: { value: "30000" } });
    fireEvent.click(screen.getByText("Save provider"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ providers: [expect.objectContaining({ auth: { type: "command", command: "/bin/key", args: [], timeoutMs: 30000 } })] })));
  });
  it("uses presets, writes a masked key, and adds manual models", async () => {
    render(<Providers />);
    fireEvent.click(screen.getByText("Add provider"));
    fireEvent.change(screen.getByLabelText("Provider ID"), { target: { value: "custom" } });
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Custom" } });
    fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "openai-compatible" } });
    expect((screen.getByLabelText("API style") as HTMLSelectElement).value).toBe("chat-completions");
    fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "http://localhost:9000/v1" } });
    const key = screen.getByLabelText("API key");
    expect(key.getAttribute("type")).toBe("password");
    fireEvent.change(key, { target: { value: "super-secret" } });
    fireEvent.change(screen.getByLabelText("Model ID"), { target: { value: "manual/model" } });
    fireEvent.click(screen.getByText("Add model ID manually"));
    expect(screen.getByLabelText("manual/model").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByText("Save provider"));
    await waitFor(() => expect(providerSetApiKey).toHaveBeenCalledWith("custom", "super-secret"));
    expect(screen.queryByDisplayValue("super-secret")).toBeNull();
  });
  it("renders fetch latency, model toggles and actionable failures", async () => {
    vi.mocked(providerTestAndFetch).mockResolvedValueOnce({ ok: true, latencyMs: 42, models: [{ id: "fetched", enabled: true }] }).mockResolvedValueOnce({ ok: false, latencyMs: 2, models: [], error: "API key rejected" });
    render(<Providers initial={[provider]} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit Local" }));
    fireEvent.click(screen.getByText("Test & fetch models"));
    await waitFor(() => expect(screen.getByText("42 ms")).toBeTruthy());
    expect(screen.getByLabelText("fetched").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByLabelText("fetched"));
    expect(screen.getByLabelText("fetched").getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(screen.getByText("Test & fetch models"));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("API key rejected"));
  });
  it("validates policy bounds and role allow-lists", () => {
    const policy = structuredClone(defaultAgentPolicy);
    policy.subagents.allowedModels = [{ providerId: "local", modelId: "small" }];
    render(<SettingsAgentPolicySection appSettings={{ providers: [{ ...provider, models: [...provider.models, { id: "large", enabled: true }] }], agentPolicy: policy } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    expect(screen.getAllByLabelText("Name")).toHaveLength(3);
    fireEvent.change(screen.getByLabelText("Max concurrent"), { target: { value: "17" } });
    expect(screen.getByRole("alert").textContent).toContain("concurrent 1–16");
    expect((screen.getByText("Save policy") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Max concurrent"), { target: { value: "4" } });
    expect((screen.getByText("Save policy") as HTMLButtonElement).disabled).toBe(false);
    fireEvent.change(screen.getAllByLabelText("Role model")[0], { target: { value: "local/large" } });
    expect(screen.getByRole("alert").textContent).toContain("must be in allowed models");
    expect((screen.getByText("Save policy") as HTMLButtonElement).disabled).toBe(true);
  });
  it("shows engine enforcement and the merged workspace policy", () => {
    const workspace = { id: "ws", name: "Project", path: "/project", connected: true, settings: { sidebarCollapsed: false, agentPolicy: { subagents: { maxConcurrent: 2 } } } };
    render(<SettingsAgentPolicySection workspace={workspace} appSettings={{ agentPolicy: defaultAgentPolicy } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    expect(screen.getAllByText("Max per turn: pi").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Allowed models: pi").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Max concurrent: pi, codex").length).toBeGreaterThan(0);
    expect(screen.getByText("Effective policy for Project (app + workspace)")).toBeTruthy();
    expect((screen.getAllByLabelText("Max concurrent")[1] as HTMLInputElement).value).toBe("2");
    expect((screen.getAllByLabelText("Max per turn")[1] as HTMLInputElement).value).toBe("8");
  });
});

it("renders a provider card with a shared toggle, test status, icon actions and delete confirmation", async () => {
  render(<Providers initial={[provider]} />);
  const toggle = screen.getByRole("button", { name: "Enable Local" });
  expect(toggle.getAttribute("aria-pressed")).toBe("true");
  expect(toggle.closest(".provider-card")).toBeTruthy();
  expect(document.querySelector('input[type="checkbox"]')).toBeNull();
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.getAttribute("aria-pressed")).toBe("false"));
  vi.mocked(providerTestAndFetch).mockResolvedValue({ ok: true, latencyMs: 15, models: [{ id: "fetched", enabled: true }] });
  fireEvent.click(screen.getByRole("button", { name: "Test & fetch Local" }));
  await waitFor(() => expect(screen.getByText(/1 models · tested/)).toBeTruthy());
  fireEvent.click(screen.getByRole("button", { name: "Delete Local" }));
  expect(screen.getByRole("alertdialog", { name: "Delete Local?" })).toBeTruthy();
  expect(screen.getByRole("article", { name: "Local provider" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Delete Local" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete provider" }));
  await waitFor(() => expect(screen.queryByRole("article", { name: "Local provider" })).toBeNull());
});
