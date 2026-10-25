// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { SidebarTree } from "./SidebarTree";

afterEach(cleanup);
it("marks arbitrary depth, last siblings and the selected path to the root", () => {
  const rows = [{ id: "main", depth: 0 }, { id: "side", depth: 1 }, { id: "agent", depth: 2 }, { id: "sibling", depth: 1 }, { id: "other", depth: 0 }];
  const { container } = render(<SidebarTree rows={rows} rowKey={row => row.id} selected={row => row.id === "agent"}>{row => <button>{row.id}</button>}</SidebarTree>);
  const nodes = Array.from(container.querySelectorAll(".sidebar-tree-node"));
  // Project-level roots must draw the same trunk/elbows as nested children.
  expect(nodes[0].classList.contains("tree-branch")).toBe(true);
  expect(nodes[4].classList.contains("tree-branch")).toBe(true);
  expect(nodes[0].classList.contains("tree-last-child")).toBe(false);
  expect(nodes.map(node => node.getAttribute("data-depth"))).toEqual(["1", "2", "3", "2", "1"]);
  expect(nodes.filter(node => node.classList.contains("tree-last-child")).map(node => node.querySelector("button")?.textContent)).toEqual(["agent", "sibling", "other"]);
  expect(nodes.filter(node => node.classList.contains("tree-path")).map(node => node.querySelector("button")?.textContent)).toEqual(["main", "side", "agent"]);
});

it("renders 500 rows without measuring layout", () => {
  const rows = Array.from({ length: 500 }, (_, id) => ({ id, depth: id % 5 }));
  const { container } = render(<SidebarTree rows={rows} rowKey={row => String(row.id)} selected={() => false}>{row => <button>{row.id}</button>}</SidebarTree>);
  expect(container.querySelectorAll("button")).toHaveLength(500);
});
