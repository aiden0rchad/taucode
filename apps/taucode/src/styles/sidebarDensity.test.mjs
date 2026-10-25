// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { readFileSync } from "node:fs";
const css = ["styles/sidebar.css", "styles/messages.css", "features/chat/chat.css", "features/sidechats/sidechats.css", "styles/visual-refresh.css"].map(file => readFileSync(`src/${file}`, "utf8")).join("\n");

function render(html) {
  document.head.innerHTML = "";
  const style = document.createElement("style");
  style.textContent = css;
  document.head.append(style);
  document.body.innerHTML = html;
  return { container: document.body };
}
afterEach(() => { document.head.innerHTML = ""; document.body.innerHTML = ""; delete document.documentElement.dataset.sidebarDensity; });
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
