// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { describeTurnError, TurnErrorCard } from "./TurnErrorCard";

afterEach(cleanup);
it.each([
  ["429 chatgpt_quota_paused", "Account usage limit reached"],
  ["429 Too many requests", "Too many requests"],
  ["401 Unauthorized", "Provider access needs attention"],
  ["context_length_exceeded", "Conversation is too long"],
  ["fetch failed", "Couldn’t reach the provider"],
  ["Unexpected backend error", "Request couldn’t complete"],
])("summarizes %s", (message, title) => expect(describeTurnError(message).title).toBe(title));

it("prevents duplicate retries and handles rejection without losing error details", async () => {
  let reject!: (error: Error) => void;
  const onRetry = vi.fn(() => new Promise<boolean>((_resolve, fail) => { reject = fail; }));
  const { container } = render(<TurnErrorCard message="fetch failed" onRetry={onRetry} onChangeModel={vi.fn()} />);
  const retry = screen.getByRole("button", { name: "Retry" });
  fireEvent.click(retry);
  fireEvent.click(retry);
  expect(onRetry).toHaveBeenCalledOnce();
  expect((screen.getByRole("button", { name: "Change model" }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => reject(new Error("offline")));
  expect(screen.getByRole("status").textContent).toContain("Your message is still available");
  expect((screen.getByRole("button", { name: "Retry" }) as HTMLButtonElement).disabled).toBe(false);
  expect(container.querySelector("pre")?.textContent).toBe("fetch failed");
});
