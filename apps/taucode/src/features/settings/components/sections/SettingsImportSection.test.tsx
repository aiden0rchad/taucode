// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { importRequest, pickImportExport } from "@services/tauri";
import { SettingsImportSection } from "./SettingsImportSection";
vi.mock("@services/tauri", () => ({ importRequest: vi.fn(), pickImportExport: vi.fn() }));
const entries = [
  { key: "codex:1", id: "1", source: "codex", title: "Active clock", project: "Clock project", cwd: "/clock", kind: "thread", archived: false },
  { key: "codex:2", id: "2", source: "codex", title: "Archived clock", project: "Clock project", cwd: "/clock", kind: "thread", archived: true },
];
const log = { id: "import-1", created: [{ key: "codex:1", title: "Active clock", kind: "thread" }], skipped: [], warnings: [], cancelled: false, undone: false };
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("detects a project tree, defaults to non-archived, reports progress and supports confirmed undo", async () => {
  let finish: (value: unknown) => void = () => {};
  let imported = false;
  vi.mocked(importRequest).mockImplementation(async (action, params) => {
    if (action === "logs") return imported ? [log] : [];
    if (action === "status") return { running: false, completed: 0, total: 0 };
    if (action === "detect") return { entries, warnings: [] };
    if (action === "run") { imported = true; return new Promise(resolve => { finish = resolve; }); }
    if (action === "undo") return params?.confirmChanged ? { ...log, undone: true } : { needsConfirmation: ["Active clock"] };
    return {};
  });
  render(<SettingsImportSection />);
  await screen.findByText("No imports yet.");
  fireEvent.click(screen.getByRole("button", { name: "Detect local sources" }));
  expect(await screen.findByText("Active clock")).toBeTruthy();
  expect(screen.queryByText("Archived clock")).toBeNull();
  expect(screen.getByRole("button", { name: "Active clock" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Include archived" }));
  expect(screen.getByText("Archived clock")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Active clock" }));
  fireEvent.click(screen.getByRole("button", { name: "Import selected (1)" }));
  await waitFor(() => expect(importRequest).toHaveBeenCalledWith("run", { keys: ["codex:2"] }));
  expect(screen.getByRole("progressbar", { name: "Import progress" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel import" }));
  expect(importRequest).toHaveBeenCalledWith("cancel");
  finish(log);
  expect(await screen.findByText("Import complete")).toBeTruthy();
  fireEvent.click(screen.getByText(/import-1 ·/));
  fireEvent.click(screen.getByRole("button", { name: "Undo import" }));
  fireEvent.click(await screen.findByRole("button", { name: "Remove changed imports" }));
  expect(await screen.findByText("Import undone")).toBeTruthy();
});
it("uses the export picker and renders read-only history with continuation", async () => {
  vi.mocked(pickImportExport).mockResolvedValue("/synthetic/conversations.json");
  vi.mocked(importRequest).mockImplementation(async action => {
    if (action === "logs") return [log];
    if (action === "status") return { running: false, completed: 0, total: 0 };
    if (action === "export") return { entries: [], warnings: ["Synthetic export"] };
    if (action === "read") return { title: "Clock history", source: "chatgpt", kind: "chat", messages: [{ role: "assistant", text: "Clock report" }], raw: [{ type: "unknown" }], attachments: ["outputs/report.txt"] };
    return {};
  });
  render(<SettingsImportSection />);
  fireEvent.click(await screen.findByRole("button", { name: "Import chats from export file…" }));
  expect(await screen.findByText("Synthetic export")).toBeTruthy();
  expect(importRequest).toHaveBeenCalledWith("export", { path: "/synthetic/conversations.json" });
  fireEvent.click(screen.getByText(/import-1 ·/));
  fireEvent.click(screen.getByRole("button", { name: "Active clock" }));
  expect(await screen.findByText("Clock report")).toBeTruthy();
  expect(screen.getByText("Audit entry 1")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Continue in pi" }));
  expect(await screen.findByText(/Continuation created/)).toBeTruthy();
});

it("directs imported Cowork continuation to Work mode", async () => {
  vi.mocked(importRequest).mockImplementation(async action => {
    if (action === "logs") return [{ ...log, created: [{ key: "cowork:1", title: "Cowork report", kind: "work" }] }];
    if (action === "status") return { running: false, completed: 0, total: 0 };
    if (action === "read") return { title: "Cowork report", source: "cowork", kind: "work", messages: [], raw: [], attachments: [] };
    return {};
  });
  render(<SettingsImportSection />);
  fireEvent.click(await screen.findByText(/import-1 ·/));
  fireEvent.click(screen.getByRole("button", { name: "Cowork report" }));
  expect(await screen.findByText(/Open Work mode to continue as a new task/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Continue in pi" })).toBeNull();
});
