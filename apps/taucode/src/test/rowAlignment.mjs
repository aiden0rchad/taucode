/** Shared by DOM tests and Playwright page.evaluate; keep this function self-contained.
 * jsdom has no layout: unmeasured counts distinguish markup checks from geometry checks.
 */
export function assertRowAlignment(container) {
  const selectors = ".workspace-titlebar, .main-header, .sidebar-header, .sidebar-bottom-actions, .sidebar-new-row, .composer-shell-toolbar, .composer-bar, .composer-meta, .composer-shell-left, .composer-shell-actions, .side-panel-tabbar, .settings-group-title, .setup-footer, .sidebar .mode-row";
  const rows = [...container.querySelectorAll(selectors)];
  if (container.matches?.(selectors)) rows.unshift(container);
  const failures = [];
  const result = { rows: 0, icons: 0, unmeasuredRows: 0, unmeasuredIcons: 0 };
  const describe = element => ({
    element: element.tagName.toLowerCase(), class: element.className,
    label: element.getAttribute("aria-label"), rect: element.getBoundingClientRect().toJSON(),
  });
  const visible = element => {
    for (let parent = element; parent; parent = parent.parentElement) {
      const css = getComputedStyle(parent);
      if (parent.hidden || css.display === "none" || css.visibility === "hidden") return false;
    }
    return true;
  };
  const buttons = new Set();
  for (const row of rows) {
    if (!visible(row)) continue;
    if (!row.getBoundingClientRect().height) result.unmeasuredRows++;
    else {
      result.rows++;
      const children = [...row.children].filter(child => visible(child) && child.getBoundingClientRect().height > 0 && getComputedStyle(child).position !== "fixed" && (getComputedStyle(child).position !== "absolute" || child.matches("[data-icon-button]")));
      const centers = children.map(child => { const rect = child.getBoundingClientRect(); return rect.y + rect.height / 2; });
      if (centers.length > 1 && Math.max(...centers) - Math.min(...centers) > 1) {
        failures.push({ problem: "Vertical centers differ by more than 1px", row: describe(row), children: children.map(describe) });
      }
    }
    for (const button of row.querySelectorAll("button")) buttons.add(button);
  }
  for (const button of container.querySelectorAll("[data-icon-button]")) buttons.add(button);
  for (const button of buttons) {
    if (button.closest(".browser-pane, .browser-toolbar, .browser-tabs") || !visible(button)) continue;
    if (!button.hasAttribute("data-icon-button") && (button.textContent.trim() || !button.querySelector("svg, img"))) continue;
    if (!button.getAttribute("aria-label")) failures.push({ problem: "Icon button needs aria-label", ...describe(button) });
    const rect = button.getBoundingClientRect();
    if (!rect.width && !rect.height) {
      result.unmeasuredIcons++;
      if (!["sm", "md", "lg"].includes(button.dataset.iconButton)) failures.push({ problem: "Unmeasured icon button must use IconButton", ...describe(button) });
    } else {
      result.icons++;
      if (![24, 28, 32].includes(rect.width) || rect.width !== rect.height) failures.push({ problem: "Icon button must be a 24/28/32px square", ...describe(button) });
    }
  }
  if (failures.length) throw new Error(JSON.stringify(failures, null, 2));
  return result;
}
