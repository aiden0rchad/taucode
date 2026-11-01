import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("shows approval prompts without an entry animation that can stall in a background window", () => {
  const css = readFileSync("src/styles/approval-toasts.css", "utf8");
  expect(css).toMatch(/\.approval-toasts \.approval-toast\s*\{[^}]*animation: none;/);
});
