import { describe, expect, it } from "vitest";
import { composerPlaceholder } from "./ComposerInput";
describe("engine placeholder", () => {
  it.each([["pi", "taucode"], ["codex", "Codex"], ["claude", "Claude Code"], [undefined, "taucode"], ["unknown", "taucode"]])("uses %s display name", (engine, name) => {
    expect(composerPlaceholder(engine)).toBe(`Ask ${name} to do something…`);
  });
});
