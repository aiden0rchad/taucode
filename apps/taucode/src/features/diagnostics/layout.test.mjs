// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { diagnosticsRequest } from "@services/tauri";
import { buildDefaultSettings } from "@settings/hooks/useAppSettings";
import { createPreviewHandler } from "@/dev/uiPreview/handler";
import { DiagnosticsSettings } from "./DiagnosticsSettings";
const settingsCss = readFileSync("src/styles/settings.css", "utf8");
const diagnosticsCss = readFileSync("src/features/diagnostics/diagnostics.css", "utf8");
vi.mock("@services/tauri", () => ({ diagnosticsRequest: vi.fn() }));
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function setup(dir) {
  const { handler } = createPreviewHandler();
  diagnosticsRequest.mockImplementation(async (action, params) => {
    const result = await handler("diagnostics_request", { action, params });
    return action === "info" && dir ? { ...result, dir } : result;
  });
  render(createElement(DiagnosticsSettings, { appSettings: buildDefaultSettings(), onUpdateAppSettings: vi.fn() }));
}

// The longer usernames put the space at the end of the first or start of the second span.
it.each(["x", "rolandleyco", "rolandleycoxx"])("preserves journal path whitespace for user %s", async user => {
  const dir = `/Users/${user}/Library/Application Support/dev.taucode.desktop/diagnostics`;
  const style = document.createElement("style");
  style.textContent = settingsCss + diagnosticsCss;
  document.head.append(style);
  try {
    setup(dir);
    const code = await screen.findByTitle(dir);
    const [start, end] = code.children;
    // jsdom has no layout engine: check exact DOM text and the CSS responsible
    // for preserving boundary spaces, including when flex clips the path.
    for (const width of ["1200px", "240px"]) {
      code.parentElement.style.width = width;
      expect(code.textContent).toBe(dir);
      expect(code.textContent).not.toContain("ApplicationSupport");
      expect(code.title).toBe(dir);
      expect(getComputedStyle(code).display).toBe("flex");
      for (const part of [start, end]) {
        expect(getComputedStyle(part).whiteSpace).toBe("pre");
        expect(getComputedStyle(part).overflow).toBe("hidden");
      }
      expect(getComputedStyle(start).textOverflow).toBe("ellipsis");
    }
  } finally { style.remove(); }
});

it.each([false, true])("keeps custom layouts independent of the settings grid (generic CSS last: %s)", async genericLast => {
  expect(diagnosticsCss).toContain(".diagnostics-path");
  const style = document.createElement("style");
  style.textContent = genericLast ? diagnosticsCss + settingsCss : settingsCss + diagnosticsCss;
  document.head.append(style);
  try {
    setup();
    await screen.findByText("Engine failed to start");
    const page = document.querySelector(".diagnostics-page");
    const path = page.querySelector(".diagnostics-path");
    expect(path.classList.contains("settings-row")).toBe(false);
    expect(getComputedStyle(path).display).toBe("flex");
    expect(getComputedStyle(page.querySelector(".diagnostics-stats")).display).toBe("flex");
    for (const button of path.querySelectorAll("button")) {
      expect(getComputedStyle(button).width).toBe("28px");
      expect(getComputedStyle(button).height).toBe("28px");
    }
    expect(screen.getByText("Log an issue or idea").closest(".settings-group-actions")).toBeTruthy();
    const rows = page.querySelectorAll(".diagnostics-issue");
    expect(rows).toHaveLength(6);
    for (const row of rows) {
      expect(row.tagName).toBe("BUTTON");
      expect(row.closest(".settings-row")).toBeNull();
      expect(row.closest("li")?.parentElement?.tagName).toBe("UL");
      expect(getComputedStyle(row).width).toBe("100%");
      expect(getComputedStyle(row).display).toBe("flex");
      const title = getComputedStyle(row.querySelector(".diagnostics-issue-title"));
      expect(title.whiteSpace).toBe("nowrap");
      expect(title.textOverflow).toBe("ellipsis");
    }
    for (const block of page.querySelectorAll(".diagnostics-code")) {
      expect(getComputedStyle(block).position).toBe("relative");
      const copy = getComputedStyle(block.querySelector("button"));
      expect(copy.position).toBe("absolute");
      expect(copy.top).toBe("8px");
      expect(copy.right).toBe("8px");
      expect(block.querySelector("pre")?.classList.contains("thin-scroll")).toBe(true);
    }
    fireEvent.click(rows[0]);
    const detail = await screen.findByRole("region", { name: "Issue details" });
    expect(detail.querySelector(".diagnostics-identifiers.settings-row")).toBeNull();
    expect(getComputedStyle(detail.querySelector(".diagnostics-actions")).display).toBe("flex");
  } finally { style.remove(); }
});
