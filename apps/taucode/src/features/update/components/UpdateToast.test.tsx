// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { UpdateToast } from "./UpdateToast";

afterEach(cleanup);

it("renders nothing while idle", () => {
  const { container } = render(<UpdateToast state={{ stage: "idle" }} onDismiss={vi.fn()} />);
  expect(container.childElementCount).toBe(0);
});

it("offers the download and release notes for a newer version", () => {
  const onDismiss = vi.fn();
  render(<UpdateToast state={{ stage: "available", version: "9.0.0", result: { status: "available", currentVersion: __APP_VERSION__, latestVersion: "9.0.0", downloadUrl: "https://github.com/aiden0rchad/taucode/releases/download/v9/taucode.dmg", notes: "## Fixed\n- Crash on start" } }} onDismiss={onDismiss} />);
  expect(screen.getByText(`taucode 9.0.0 is available. You have ${__APP_VERSION__}.`)).toBeTruthy();
  expect(screen.getByText("Crash on start")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Download update" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(onDismiss).toHaveBeenCalled();
});

it.each([
  ["latest", "You're up to date."],
  ["noReleases", "No releases published yet."],
  ["checking", "Checking for updates…"],
] as const)("shows a calm message for %s", (stage, text) => {
  render(<UpdateToast state={{ stage }} onDismiss={vi.fn()} />);
  expect(screen.getByText(text)).toBeTruthy();
});

it("shows the reason when GitHub could not be reached", () => {
  render(<UpdateToast state={{ stage: "unavailable", error: "Couldn't reach GitHub." }} onDismiss={vi.fn()} />);
  expect(screen.getByText("Couldn't reach GitHub.")).toBeTruthy();
});
