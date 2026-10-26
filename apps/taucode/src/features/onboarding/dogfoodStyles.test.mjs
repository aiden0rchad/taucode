// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { afterEach, expect, it } from "vitest";
import { builtins, resolveTheme } from "@/features/themes/builtins";
const css = (file) => readFileSync(`src/${file}`, "utf8");
afterEach(() => { document.head.querySelectorAll("style[data-test]").forEach(node => node.remove()); document.body.innerHTML = ""; });
it("keeps takeover chrome after the macOS traffic lights and preserves modal placement", () => {
  const source = css("features/onboarding/onboarding.css");
  const takeover = source.match(/\.setup-takeover \.setup-topbar\{([^}]+)\}/)[1];
  const [inset, extra] = takeover.match(/calc\(var\(--titlebar-inset-left,(\d+)px\) \+ (\d+)px\)/).slice(1).map(Number);
  expect(inset + extra).toBeGreaterThanOrEqual(78);
  expect(takeover).toContain("height:var(--main-topbar-height,44px)");
  expect(source).toContain("height:76px;padding:18px 36px");
});
it("computes comfortable chat row padding and regular model text", () => {
  const style = document.createElement("style"); style.dataset.test = "true";
  // Load the shared density rules after Chat styles to catch specificity regressions.
  style.textContent = css("features/chat/chat.css") + css("styles/visual-refresh.css"); document.head.append(style);
  document.body.innerHTML = '<div class="sidebar chat-sidebar"><div class="chat-list"><div class="mode-row"><button class="thread-row has-secondary-line active"><span class="thread-content"><span class="thread-name">Title</span><span class="thread-details">Model</span></span><span class="thread-time">1m</span></button></div></div></div>';
  const row = getComputedStyle(document.querySelector(".thread-row"));
  expect([row.paddingTop, row.paddingRight, row.paddingBottom, row.paddingLeft]).toEqual(["6px", "10px", "6px", "10px"]);
  expect(row.height).toBe("44px");
  expect(getComputedStyle(document.querySelector(".thread-details")).fontWeight).toBe("400");
});
it("uses appearance palettes and retains accent overrides", () => {
  for (const [id, values] of [["glass-dark", ["#0b0e1c", "#1e2a78", "#3b1d6e"]], ["glass-light", ["#f7f7fb", "#dfe3ff", "#ece0ff"]]]) {
    const tokens = resolveTheme(builtins.find(theme => theme.id === id)).tokens;
    expect([tokens["--setup-base"], tokens["--setup-blob-1"], tokens["--setup-blob-2"]]).toEqual(values);
  }
  for (const id of ["cyberpunk-neon", "matrix"]) expect(resolveTheme(builtins.find(theme => theme.id === id)).tokens["--setup-blob-1"]).toBe("var(--text-accent)");
});
