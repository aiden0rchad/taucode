import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const css = readFileSync("src/styles/messages.css", "utf8");
const rule = selector => css.match(new RegExp(`${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`))?.[1] ?? "";

it("sizes message tables to their content instead of a fixed wide layout", () => {
  const table = rule(".markdown .markdown-table");
  expect(table).toContain("width: 100%;");
  expect(table).not.toContain("table-layout: fixed");
  expect(table).not.toMatch(/calc\(|rem\)/);
  // No column is forced to a width or to a single line: any table shape must fit or scroll.
  expect(css).not.toMatch(/\.markdown-table (?:th|td):(?:first-child|last-child|nth-child)/);
  const cells = rule(".markdown .markdown-table th,\n.markdown .markdown-table td");
  expect(cells).toContain("word-break: normal;");
  expect(cells).toContain("white-space: normal;");
  expect(rule(".markdown .markdown-table-wrap")).toContain("overflow-x: auto;");
});
