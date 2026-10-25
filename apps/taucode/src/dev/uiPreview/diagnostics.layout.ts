// Opt-in, real-DOM preview test. jsdom cannot measure layout.
export async function checkDiagnosticsLayout() {
  const waitFor = async (selector: string) => {
    for (let attempt = 0; attempt < 200; attempt++) {
      const element = document.querySelector<HTMLElement>(selector);
      if (element) return element;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw new Error("Missing preview element: " + selector);
  };
  const assert = (condition: boolean, message: string) => { if (!condition) throw new Error(message); };
  const inside = (inner: DOMRect, outer: DOMRect) => inner.left >= outer.left - 1 && inner.right <= outer.right + 1;
  try {
    const row = await waitFor(".diagnostics-issue");
    await document.fonts.ready;
    await new Promise(requestAnimationFrame);
    row.click();
    await waitFor(".diagnostics-detail");
    await new Promise(requestAnimationFrame);
    const page = document.querySelector<HTMLElement>(".diagnostics-page")!;
    assert(page.getBoundingClientRect().width > 0, "A real layout engine is required");
    for (const group of page.querySelectorAll(".settings-group")) {
      const bounds = group.getBoundingClientRect();
      for (const element of group.querySelectorAll("*")) {
        const rect = element.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) assert(inside(rect, bounds), "Horizontal overflow: " + element.className);
      }
    }
    const rows = page.querySelectorAll(".diagnostics-issue");
    assert(rows.length === 6, "Expected the populated six-issue fixture");
    for (const item of rows) {
      const group = item.closest(".settings-group-card")!;
      const style = getComputedStyle(group);
      const width = group.getBoundingClientRect().width - parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth) - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      assert(item.getBoundingClientRect().width >= width * .95, "Inbox row does not span its group");
    }
    const blocks = page.querySelectorAll(".diagnostics-code");
    assert(blocks.length === 3, "Build the diagnostics binary before testing all setup cards");
    for (const block of blocks) {
      const rect = block.getBoundingClientRect();
      const copy = block.querySelector("button")!.getBoundingClientRect();
      assert(inside(copy, rect) && copy.top >= rect.top && copy.bottom <= rect.bottom, "Copy button is outside its code block");
    }
    document.documentElement.dataset.diagnosticsLayout = "passed";
    return { rows: rows.length, codeBlocks: blocks.length, width: page.getBoundingClientRect().width };
  } catch (error) {
    document.documentElement.dataset.diagnosticsLayout = "failed: " + String(error);
    throw error;
  }
}
