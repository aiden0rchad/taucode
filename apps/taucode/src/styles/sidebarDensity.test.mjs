// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { cleanup, render as renderComponent } from "@testing-library/react";
import { NewConversationRow } from "@/features/chat/NewConversationRow";
const css = ["styles/ds-tokens.css", "styles/buttons.css", "styles/sidebar.css", "styles/messages.css", "features/chat/chat.css", "features/work/work.css", "features/sidechats/sidechats.css", "styles/visual-refresh.css", "styles/toolbars.css", "features/design-system/components/icon-button.css"].map(file => readFileSync(`src/${file}`, "utf8")).join("\n");

function render(html) {
  document.head.innerHTML = "";
  const style = document.createElement("style");
  style.textContent = css;
  document.head.append(style);
  document.body.innerHTML = html;
  return { container: document.body };
}
afterEach(() => { cleanup(); document.head.innerHTML = ""; document.body.innerHTML = ""; delete document.documentElement.dataset.sidebarDensity; });
it("computes compact and comfortable height tokens for the same row", () => {
  const { container } = render('<aside class="sidebar"><button class="thread-row has-secondary-line">Title</button></aside>');
  const row = container.querySelector("button");
  expect(getComputedStyle(document.documentElement).getPropertyValue("--sidebar-row-height")).toBe("28px");
  expect(getComputedStyle(row).height).toBe("var(--sidebar-row-height)");
  expect(getComputedStyle(row).minHeight).toBe("0");
  document.documentElement.dataset.sidebarDensity = "comfortable";
  expect(getComputedStyle(document.documentElement).getPropertyValue("--sidebar-row-height")).toBe("44px");
  expect(getComputedStyle(row).height).toBe("var(--sidebar-row-height)");
});

it.each(["New thread", "New chat", "New task"])("centers both controls in the 28px %s row", label => {
  render('<aside class="sidebar"></aside>');
  const { container } = renderComponent(createElement(NewConversationRow, { label, onNew() {}, onSearch() {} }), { container: document.querySelector("aside") });
  const row = getComputedStyle(container.querySelector(".sidebar-new-row"));
  const button = getComputedStyle(container.querySelector(".mode-new"));
  expect(row.display).toBe("flex");
  expect(row.alignItems).toBe("center");
  expect(row.height).toBe("28px");
  expect(button.alignSelf).toBe("center");
  expect(button.height).toBe(row.height);
  expect(button.minHeight).toBe(row.height);
  expect(container.querySelector("[data-icon-button]").dataset.iconButton).toBe("md");
  expect(getComputedStyle(document.documentElement).getPropertyValue("--icon-button-28")).toBe(row.height);
});

it.each(["compact", "comfortable"])("keeps Work rows at most 32px and cards inset at %s density", density => {
  const { container } = render(`
    <div class="app"><aside class="sidebar"><div class="sidebar-body"><section class="project-group-card">Project</section></div></aside></div>
    <div class="chat-layout work-layout"><aside class="sidebar chat-sidebar"><div class="chat-list sidebar-body">
      <section class="work-space project-group-card"><div class="sidebar-tree"><div class="mode-row">
        <button class="thread-row work-task-row is-nested"><span class="thread-status"></span><span class="thread-name">Task</span><span class="thread-time">1m</span></button>
        <button class="ds-icon-button mode-row-menu-trigger" data-icon-button="sm" aria-label="Actions"></button>
      </div></div></section>
    </div></aside></div>
    <div class="chat-layout"><aside class="sidebar chat-sidebar"><div class="chat-list sidebar-body"><button class="thread-row has-secondary-line">Chat title and model</button></div></aside></div>`);
  document.documentElement.dataset.sidebarDensity = density;
  const sidebar = container.querySelector(".work-layout .sidebar");
  const row = sidebar.querySelector(".work-task-row");
  const codeBody = getComputedStyle(container.querySelector(".app .sidebar-body"));
  for (const width of [220, 280, 420]) {
    sidebar.style.width = `${width}px`;
    // jsdom does not lay out boxes or resolve inherited CSS variables. Check the
    // computed sizing rule plus its authored token; WebKit checks the actual rects.
    const token = getComputedStyle(sidebar).getPropertyValue("--sidebar-row-height") || getComputedStyle(document.documentElement).getPropertyValue("--sidebar-row-height");
    const style = getComputedStyle(row);
    const height = parseFloat(style.height.replace("var(--sidebar-row-height)", token));
    expect(height).toBe(density === "compact" ? 28 : 32);
    expect(height).toBeLessThanOrEqual(32);
    expect(style.minHeight).toBe("0");
    expect(style.boxSizing).toBe("border-box");
    expect(style.paddingTop).toBe("0px");
    expect(style.paddingBottom).toBe("0px");
    const action = getComputedStyle(sidebar.querySelector(".mode-row-menu-trigger"));
    expect(action.position).toBe("absolute");
    expect(action.getPropertyValue("--icon-button-size")).toBe("var(--icon-button-24)");
    expect(parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--icon-button-24"))).toBeLessThanOrEqual(height);
    const body = getComputedStyle(sidebar.querySelector(".sidebar-body"));
    for (const property of ["marginInline", "paddingInline", "marginLeft", "marginRight", "paddingLeft", "paddingRight"]) expect(body[property], property).toBe(codeBody[property]);
    expect(getComputedStyle(sidebar).padding).toBe("var(--sidebar-top-padding, 36px) var(--sidebar-padding) 20px");
    expect(parseFloat(getComputedStyle(sidebar).getPropertyValue("--sidebar-padding"))).toBe(20);
    expect(getComputedStyle(sidebar.querySelector(".work-space")).marginLeft).toBe("0px");
  }
  expect(getComputedStyle(container.querySelector(".chat-layout:not(.work-layout) .thread-row")).height).toBe("44px");
});

it("keeps the assistant content inset unchanged on hover and floats actions", () => {
  const { container } = render('<article class="message assistant"><div class="message-bubble"><p>Answer</p><pre>code</pre><div class="message-hover-actions"><button>Copy</button></div></div></article>');
  const bubble = container.querySelector(".message-bubble");
  const offset = () => {
    const style = getComputedStyle(bubble);
    return [parseFloat(style.paddingLeft) + parseFloat(style.marginLeft), parseFloat(style.paddingTop) + parseFloat(style.marginTop)];
  };
  expect(getComputedStyle(bubble).padding).toBe("10px 12px");
  const before = offset();
  bubble.classList.add("is-hovered");
  expect(offset()).toEqual(before);
  expect(before).toEqual([0, 0]);
  const actions = getComputedStyle(container.querySelector(".message-hover-actions"));
  expect(actions.position).toBe("absolute");
  expect(actions.right).toBe("8px");
  expect(actions.bottom).toBe("-14px");
});

it("lets user bubbles hug their content", () => {
  const { container } = render('<article class="message user"><div class="message-bubble">One line</div></article>');
  const style = getComputedStyle(container.querySelector(".message-bubble"));
  expect(style.padding).toBe("12px 14px");
  expect(style.minHeight).toBe("0");
  expect(style.height).toBe("auto");
});
