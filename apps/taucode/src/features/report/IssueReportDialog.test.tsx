// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { issueReportDraft, issueReportScreenshot, openIssueReport } from "@services/tauri";
import { IssueReportDialog, ISSUE_REPORT_INCLUDES } from "./IssueReportDialog";
import { setIssueReportContext } from "./issueReportContext";

vi.mock("@services/tauri", () => ({
  getAppBuildType: vi.fn().mockResolvedValue("debug"),
  issueReportDraft: vi.fn().mockResolvedValue({ title: "", body: "### Environment\n- Engine: pi" }),
  openIssueReport: vi.fn().mockResolvedValue({ truncated: false }),
  issueReportScreenshot: vi.fn(),
}));
afterEach(cleanup);

it("says what is included, lets the user edit, and only opens GitHub on request", async () => {
  setIssueReportContext({ engine: "pi", model: "glm-5.3" });
  render(<IssueReportDialog onClose={vi.fn()} fingerprint="abcd" />);
  for (const item of ISSUE_REPORT_INCLUDES) expect(screen.getByText(item)).toBeTruthy();
  expect(screen.getByText(/Nothing is sent from taucode/)).toBeTruthy();
  const body = await screen.findByDisplayValue(/Engine: pi/);
  expect(vi.mocked(issueReportDraft)).toHaveBeenCalledWith(expect.objectContaining({ engine: "pi", model: "glm-5.3", buildType: "debug", fingerprint: "abcd" }));
  expect(openIssueReport).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Title"), { target: { value: "Crash on start" } });
  fireEvent.change(body, { target: { value: "My own words" } });
  fireEvent.click(screen.getByRole("button", { name: "Open on GitHub" }));
  await waitFor(() => expect(openIssueReport).toHaveBeenCalledWith("Crash on start", "My own words"));
  expect(await screen.findByText(/click Submit to post it/)).toBeTruthy();
});

it("hides dialogs while taking the screenshot", async () => {
  let hidden = false;
  vi.mocked(issueReportScreenshot).mockImplementation(async () => { hidden = document.documentElement.classList.contains("taucode-capturing"); return { copied: true }; });
  render(<IssueReportDialog onClose={vi.fn()} />);
  await screen.findByDisplayValue(/Engine: pi/);
  fireEvent.click(screen.getByRole("button", { name: "Copy screenshot" }));
  expect(await screen.findByText(/Screenshot copied/)).toBeTruthy();
  expect(issueReportScreenshot).toHaveBeenCalledWith("copy");
  expect(hidden).toBe(true);
  expect(document.documentElement.classList.contains("taucode-capturing")).toBe(false);
});
