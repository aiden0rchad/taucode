// @vitest-environment jsdom
import { assertRowAlignment } from "@/test/rowAlignment.mjs";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { WorkspaceHome } from "./WorkspaceHome";
const openFile = vi.fn();
vi.mock("@/features/messages/hooks/useFileLinkOpener", () => ({ useFileLinkOpener: () => ({ openFileLink: openFile }) }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (path: string) => path }));
vi.mock("@services/dragDrop", () => ({ subscribeWindowDragDrop: () => () => {} }));
vi.mock("@/features/composer/hooks/useComposerImages", () => ({ useComposerImages: () => ({ activeImages: [], attachImages: vi.fn(), pickImages: vi.fn(), removeImage: vi.fn(), clearActiveImages: vi.fn() }) }));
afterEach(() => { assertRowAlignment(document.body); cleanup(); });
const props = {
  workspace: { id: "w", name: "Project", path: "/project", connected: true, settings: { sidebarCollapsed: false } },
  runs: [], recentThreadInstances: [], recentThreadsUpdatedAt: null, prompt: "", runMode: "local", engine: "pi",
  models: [{ id: "litellm/opus", displayName: "Claude Opus 5.5", model: "litellm/opus", providerName: "LiteLLM", reasoning: { supported: true, levels: ["low", "medium", "high"], default: "medium", control: "effort", source: "user", detectedAt: 0 } }],
  selectedModelId: "litellm/opus", modelSelections: {}, collaborationModes: [], selectedEffort: "medium",
  skills: [], prompts: [], files: [], threadStatusById: {}, dictationState: "idle", dictationLevel: 0,
  agentMdFilename: "CLAUDE.md", agentMdStatuses: { "AGENTS.md": false, "CLAUDE.md": true }, agentMdDetected: ["GEMINI.md", ".github/copilot-instructions.md"],
  agentMdContent: "Instructions", agentMdExists: true, agentMdDirty: false,
  onAgentMdSelect: vi.fn(), onAgentMdChange: vi.fn(), onAgentMdSave: vi.fn(), onAgentMdRefresh: vi.fn(),
  onPromptChange: vi.fn(), onSelectEffort: vi.fn(), onSelectModel: vi.fn(), onRunModeChange: vi.fn(), onToggleModel: vi.fn(), onModelCountChange: vi.fn(),
} as unknown as ComponentProps<typeof WorkspaceHome>;
it("uses the combined model chip with heat and a reasoning slider", () => {
  const { container, rerender } = render(<WorkspaceHome {...props} />);
  expect(screen.getByPlaceholderText("Ask taucode to do something…")).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "Thinking mode" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Claude Opus 5.5 · Medium" }));
  expect(screen.getByRole("slider")).toBeTruthy();
  expect(container.querySelector(".reasoning-chip")).toBeTruthy();
  rerender(<WorkspaceHome {...props} selectedEffort="high" />);
  expect(document.querySelector(".reasoning-popover.burning .reasoning-flame")).toBeTruthy();
});
it("shows file statuses, edits the selected file, and opens detected files", () => {
  render(<WorkspaceHome {...props} />);
  expect(screen.getByRole("button", { name: "CLAUDE.md · Found" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "AGENTS.md · Not found" }));
  expect(props.onAgentMdSelect).toHaveBeenCalledWith("AGENTS.md");
  expect(screen.getByDisplayValue("Instructions")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "GEMINI.md" }));
  expect(openFile).toHaveBeenCalledWith({ path: "GEMINI.md", line: null, column: null });
});

it("displays and removes shared browser draft attachments across workspace changes", () => {
  const image = "data:image/png;base64,AA==";
  const imageAttachments = { activeImages: [image], attachImages: vi.fn(), pickImages: vi.fn(), removeImage: vi.fn(), clearActiveImages: vi.fn() };
  const { container, rerender } = render(<WorkspaceHome {...props} imageAttachments={imageAttachments} />);
  expect(container.querySelector('.composer-attachment img')?.getAttribute("src")).toBe(image);
  fireEvent.click(screen.getByRole("button", { name: /^Remove / }));
  expect(imageAttachments.removeImage).toHaveBeenCalledWith(image);
  rerender(<WorkspaceHome {...props} workspace={{ ...props.workspace, id: "other", path: "/other" }} imageAttachments={{ ...imageAttachments, activeImages: [] }} />);
  expect(container.querySelector(".composer-attachment")).toBeNull();
  rerender(<WorkspaceHome {...props} imageAttachments={imageAttachments} />);
  expect(container.querySelector('.composer-attachment img')?.getAttribute("src")).toBe(image);
});
