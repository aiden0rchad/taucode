// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import postcss from "postcss";
import { createElement } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import App from "@/App";
import { installUiPreview } from "@/dev/uiPreview";

const sources = ["styles/themes.dark.css", "styles/sidebar.css", "styles/main.css", "styles/buttons.css", "styles/messages.css", "features/chat/chat.css", "features/work/work.css"];
const rules = sources.flatMap(path => {
  const result = [];
  postcss.parse(readFileSync(`src/${path}`, "utf8")).walkRules(rule => result.push(rule));
  return result;
});
const declarations = selector => Object.fromEntries(rules.filter(rule => rule.selector === selector && rule.parent.type !== "atrule").flatMap(rule => rule.nodes.filter(node => node.type === "decl").map(node => [node.prop, node.value])));

afterEach(() => { cleanup(); clearMocks(); vi.unstubAllEnvs(); });

it("keeps header actions natural-width and moves overflow to a narrow-screen popover", () => {
  expect(declarations(".main-header-actions")).toMatchObject({ "flex-shrink": "0", overflow: "visible" });
  expect(declarations(".workspace-header")).toMatchObject({ "min-width": "0", flex: "1" });
  expect(declarations(".workspace-title")).toMatchObject({ overflow: "hidden", "text-overflow": "ellipsis" });
  for (const rule of rules.filter(rule => rule.selector.includes("main-header-actions"))) {
    for (const decl of rule.nodes.filter(node => node.type === "decl")) {
      if (/^overflow/.test(decl.prop)) expect(decl.value).not.toMatch(/auto|scroll/);
      if (rule.parent.type !== "atrule") expect(decl.prop).not.toBe("max-width");
    }
  }
  const popover = rules.find(rule => rule.selector === ".main-header-actions-content.is-open");
  expect(popover.parent.params).toBe("(max-width: 700px)");
});

it("contains nested sidebar rows without horizontal scrolling", () => {
  expect(declarations(".chat-list")).toMatchObject({ "min-width": "0", "overflow-x": "hidden", "overflow-y": "auto" });
  expect(declarations(".chat-sidebar")["overflow-x"]).toBe("hidden");
  expect(declarations(".chat-list .thread-row.is-nested").width).toBe("calc(100% - 2px)");
  expect(declarations(".thread-name")).toMatchObject({ "min-width": "0", "text-overflow": "ellipsis", overflow: "hidden" });
});

it.each(["chat", "work"])("gives every %s text element an authored type scale instead of the browser default", async mode => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=${mode}&chat=chat-1&task=task-running`);
  installUiPreview();
  render(createElement(App));
  await screen.findByRole("button", { name: "Open settings" });
  await screen.findByText(mode === "chat" ? "A starting point" : "Review the notes, identify the key findings, and write a concise report in outputs/.");
  expect(declarations(".chat-layout")["font-size"]).toBe("var(--ui-font-size)");
  expect(declarations(":root")["--ui-font-size"]).toBe("13px");
  expect(declarations(".markdown")["font-size"]).toBe("var(--ui-font-size)");
  const fontRules = rules.filter(rule => rule.parent.type !== "atrule" && rule.nodes.some(node => node.prop === "font-size"));
  const elements = [...document.querySelectorAll(".chat-layout *")].filter(element => [...element.childNodes].some(node => node.nodeType === 3 && node.textContent.trim()));
  expect(elements.length).toBeGreaterThan(20);
  for (const element of elements) {
    let ancestor = element;
    let sized = false;
    while (ancestor?.closest(".chat-layout")) {
      const matching = fontRules.filter(rule => ancestor.matches(rule.selector));
      if (matching.length) {
        const sizes = matching.flatMap(rule => rule.nodes.filter(node => node.prop === "font-size").map(node => node.value));
        expect(sizes, element.outerHTML).not.toContain("initial");
        sized = true;
        break;
      }
      ancestor = ancestor.parentElement;
    }
    expect(sized, element.outerHTML).toBe(true);
  }
});
