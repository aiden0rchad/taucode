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
    expect((screen.getByLabelText("manual/model") as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByText("Save provider"));
    await waitFor(() => expect(providerSetApiKey).toHaveBeenCalledWith("custom", "super-secret"));
    expect(screen.queryByDisplayValue("super-secret")).toBeNull();
  });
  it("renders fetch latency, model toggles and actionable failures", async () => {
    vi.mocked(providerTestAndFetch).mockResolvedValueOnce({ ok: true, latencyMs: 42, models: [{ id: "fetched", enabled: true }] }).mockResolvedValueOnce({ ok: false, latencyMs: 2, models: [], error: "API key rejected" });
    render(<Providers initial={[provider]} />);
    fireEvent.click(screen.getByText("Edit Local"));
    fireEvent.click(screen.getByText("Test & fetch models"));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("42 ms"));
    expect((screen.getByLabelText("fetched") as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByLabelText("fetched"));
    expect((screen.getByLabelText("fetched") as HTMLInputElement).checked).toBe(false);
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
});
