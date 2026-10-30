import { describe, expect, it } from "vitest";
import { expandTaskCommand } from "./commands";

describe("task commands", () => {
  it.each(["plan", "goal"])("leaves /%s ready to fill in", command => {
    expect(expandTaskCommand(`/${command}`)).toMatchObject({ needsInput: true });
  });
  it("preserves multiline objectives and accepts uppercase commands", () => {
    expect(expandTaskCommand(" /GOAL Build a timer\nKeep it accessible ")).toEqual({
      text: "Goal: Build a timer\nKeep it accessible\n\nWork toward this goal. Verify the result and report what is complete or blocked.",
      needsInput: false,
    });
  });
  it.each(["/planner", "/plan/file", "Read /goal", "```\n/plan\n```", "/sidechat"])("keeps %s literal", text => {
    expect(expandTaskCommand(text)).toBeNull();
  });
});
