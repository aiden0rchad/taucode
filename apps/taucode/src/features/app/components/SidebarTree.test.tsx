// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SidebarTree } from "./SidebarTree";
import { sidebarTreeGeometry } from './sidebarTreeGeometry';

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
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

it("uses one overlay for 500 rows", () => {
  const rows = Array.from({ length: 500 }, (_, id) => ({ id, depth: id % 5 }));
  const { container } = render(<SidebarTree rows={rows} rowKey={row => String(row.id)} selected={() => false}>{row => <button>{row.id}</button>}</SidebarTree>);
  expect(container.querySelectorAll("button")).toHaveLength(500);
  expect(container.querySelectorAll('.sidebar-tree-guides')).toHaveLength(1);
});

it.each([220,420].flatMap(width => [28,44].map(height => ({width,height}))))('aligns guides at width $width and row height $height', ({width,height}) => {
  const root = {x:8,y:0,width:14,height:14};
  const points = [
    {id:'parent',parent:null,glyph:{x:32,y:28+(height-14)/2,width:14,height:14},active:true},
    {id:'selected',parent:'parent',glyph:{x:59,y:30+height+(height-8)/2,width:8,height:8},active:true},
    {id:'other',parent:null,glyph:{x:35,y:32+2*height+(height-8)/2,width:8,height:8},active:false},
  ];
  const {branches,trunks} = sidebarTreeGeometry(points,root);
  expect(branches[0].d).toMatch(/^M 15 17 V /); // Centered, 3px below the folder.
  expect(branches[0].d).toContain('A 8 8');
  for (const [i,branch] of branches.entries()) {
    expect(branch.d.endsWith(`H ${points[i].glyph.x-4}`)).toBe(true);
    expect(points[i].glyph.x).toBeLessThan(width);
  }
  expect(branches[1].d).toContain(`M 39 ${points[0].glyph.y+17}`);
  expect(branches.filter(b => b.active).map(b => b.id)).toEqual(['parent','selected']);
  expect(trunks[0]).toBe(`M 15 17 V ${points[2].glyph.y+4-8}`);
});

it('renders only ancestor active paths and a ring marker for the selected node', () => {
  vi.spyOn(Element.prototype,'getBoundingClientRect').mockImplementation(function(this: Element) {
    const node = this.closest<HTMLElement>('[data-tree-id]');
    const depth = Number(node?.getAttribute('data-depth') ?? 0);
    return {left:depth*24,top:depth*30,width:14,height:14} as DOMRect;
  });
  const rows = [{id:'parent',depth:0},{id:'selected',depth:1},{id:'sibling',depth:1},{id:'other',depth:0}];
  const {container} = render(<div className="project-group-card"><div className="workspace-row"><svg /></div><SidebarTree rows={rows} rowKey={row => row.id} selected={row => row.id === 'selected'}>{row => <button className="thread-row"><span className="thread-kind"><span className="thread-status" /></span>{row.id}</button>}</SidebarTree></div>);
  expect([...container.querySelectorAll('[data-active-path]')].map(node => node.getAttribute('data-active-path'))).toEqual(['parent','selected']);
  expect(container.querySelector('.tree-selected')?.getAttribute('data-tree-id')).toBe('selected');
});
