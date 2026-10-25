// @vitest-environment jsdom
import { createElement } from "react";
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { ModalShell } from "../modal/ModalShell";
const squircleCss = readFileSync("src/features/design-system/components/squircle/squircle.css", "utf8");
const modalCss = readFileSync("src/styles/ds-modal.css", "utf8");
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it("only positions elements with zero specificity", () => {
  for (const [, selector, declarations] of squircleCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (/(?:^|;)\s*(?:position|top|left|inset|transform)\s*:/.test(declarations)) {
      expect(selector.trim()).toMatch(/^:where\([^()]+\)$/);
    }
  }
});

it.each([false, true])("preserves absolute modal positioning with native support %s", native => {
  vi.stubGlobal("CSS", { supports: () => native });
  const style = document.createElement("style");
  // jsdom applies source order instead of selector specificity; the CSS guard above
  // checks specificity independently, and the WebKit script checks the real cascade.
  style.textContent = squircleCss + modalCss;
  document.head.append(style);
  try {
    render(createElement(ModalShell, { squircle: true, ariaLabel: "Centered" }, "Content"));
    const card = document.querySelector(".ds-modal-card.squircle");
    expect(getComputedStyle(card).position).toBe("absolute");
    expect(getComputedStyle(card).transform).toBe("translate(-50%, -50%)");
  } finally { style.remove(); }
});

