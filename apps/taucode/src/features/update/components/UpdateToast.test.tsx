// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { UpdateToast } from "./UpdateToast";

afterEach(cleanup);

it("renders nothing while idle", () => {
  const { container } = render(<UpdateToast state={{ stage: "idle" }} onUpdate={vi.fn()} onDismiss={vi.fn()} />);
  expect(container.childElementCount).toBe(0);
});

it("offers the download and release notes for a newer version", () => {
  const onUpdate = vi.fn();
  const onDismiss = vi.fn();
  render(<UpdateToast state={{ stage: "available", version: "9.0.0", result: { status: "available", currentVersion: __APP_VERSION__, latestVersion: "9.0.0", notes: "## Fixed\n- Crash on start" } }} onUpdate={onUpdate} onDismiss={onDismiss} />);
  expect(screen.getByText(`taucode 9.0.0 is available. You have ${__APP_VERSION__}.`)).toBeTruthy();
  expect(screen.getByText("Crash on start")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download" }));
  fireEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(onUpdate).toHaveBeenCalled();
  expect(onDismiss).toHaveBeenCalled();
});

it.each([
  ["latest", "You're up to date."],
  ["noReleases", "No releases published yet."],
  ["checking", "Checking for updates…"],
] as const)("shows a calm message for %s", (stage, text) => {
  render(<UpdateToast state={{ stage }} onUpdate={vi.fn()} onDismiss={vi.fn()} />);
  expect(screen.getByText(text)).toBeTruthy();
});

it("shows the reason when GitHub could not be reached", () => {
  render(<UpdateToast state={{ stage: "unavailable", error: "Couldn't reach GitHub." }} onUpdate={vi.fn()} onDismiss={vi.fn()} />);
  expect(screen.getByText("Couldn't reach GitHub.")).toBeTruthy();
});
