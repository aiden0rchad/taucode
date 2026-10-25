import { createContext, useContext, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { EngineKind, WorkspaceInfo } from "@/types";
import { sidebarTreeGeometry, type TreePoint } from './sidebarTreeGeometry';

export const SidebarProjects = createContext<WorkspaceInfo[]>([]);
export const SidebarEngine = createContext<EngineKind>("pi");
export function ProjectEngine({ engine, children }: { engine?: EngineKind; children: ReactNode }) {
  const inherited = useContext(SidebarEngine);
  return <SidebarEngine.Provider value={engine ?? inherited}>{children}</SidebarEngine.Provider>;
}

// Nested wrappers keep a fixed glyph column at each depth; one SVG draws the guides.
export function SidebarTree<T extends { depth: number }>({ rows, rowKey, selected, children, baseDepth = 1 }: {
  rows: T[]; rowKey: (row: T) => string; selected: (row: T) => boolean;
  children: (row: T) => ReactNode; baseDepth?: number;
}) {
  const element = useRef<HTMLDivElement>(null);
  const [geometry, setGeometry] = useState<ReturnType<typeof sidebarTreeGeometry>>({ branches: [], trunks: [] });
  const [hovered, setHovered] = useState<string | null>(null);
  type Node = { row: T; children: Node[]; path: boolean };
  const roots: Node[] = [];
  const stack: Node[] = [];
  for (const row of rows) {
    while (stack.length && stack[stack.length - 1].row.depth >= row.depth) stack.pop();
    const node: Node = { row, children: [], path: selected(row) };
    (stack[stack.length - 1]?.children ?? roots).push(node);
    stack.push(node);
  }
  const mark = (nodes: Node[]): boolean => {
    let path = false;
    for (const node of nodes) { node.path = mark(node.children) || node.path; path ||= node.path; }
    return path;
  };
  mark(roots);
  const render = (nodes: Node[], depth: number): ReactNode => nodes.map((node, index) =>
    <div key={rowKey(node.row)} data-tree-id={rowKey(node.row)} className={`sidebar-tree-node${depth ? " tree-branch" : ""}${index === nodes.length - 1 ? " tree-last-child" : ""}${node.path ? " tree-path" : ""}${selected(node.row) ? ' tree-selected' : ''}`} data-depth={depth}>
      {children(node.row)}
      {node.children.length > 0 && <div className="sidebar-tree-children">{render(node.children, depth + 1)}</div>}
    </div>);
  const layoutKey = rows.map(row => `${rowKey(row)}:${row.depth}:${selected(row)}`).join('|');
  useLayoutEffect(() => {
    const tree = element.current;
    if (!tree) return;
    let frame = 0;
    const glyph = (node: Element) => node.querySelector<HTMLElement>('.thread-kind > svg, .thread-kind > .thread-status, .thread-row > .thread-status');
    const header = baseDepth ? tree.closest('.project-group-card, .workspace-card, .worktree-card')?.querySelector<SVGElement>('.workspace-row > svg, .work-space-toggle > svg') : null;
    const measure = () => {
      frame = 0;
      const origin = tree.getBoundingClientRect();
      const rect = (node: Element) => { const r = node.getBoundingClientRect(); return { x:r.left-origin.left, y:r.top-origin.top, width:r.width, height:r.height }; };
      const points: TreePoint[] = [];
      for (const node of tree.querySelectorAll<HTMLElement>('.sidebar-tree-node')) {
        const icon = glyph(node);
        if (!icon) continue;
        points.push({ id:node.dataset.treeId!, parent:node.parentElement?.closest<HTMLElement>('.sidebar-tree-node')?.dataset.treeId ?? null, glyph:rect(icon), active:node.classList.contains('tree-path') });
      }
      const next = sidebarTreeGeometry(points, header ? rect(header) : undefined);
      setGeometry(previous => JSON.stringify(previous) === JSON.stringify(next) ? previous : next);
    };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(measure); };
    measure();
    const observer = new ResizeObserver(schedule);
    observer.observe(tree);
    for (const node of tree.querySelectorAll('.thread-kind, .thread-status')) observer.observe(node);
    if (header) observer.observe(header);
    // Glyph swaps and density changes can move a guide without changing tree width.
    const mutations = new MutationObserver(records => {
      if (records.some(record => !(record.target as Element).closest('.sidebar-tree-guides'))) schedule();
    });
    mutations.observe(tree, { childList:true, subtree:true });
    const density = new MutationObserver(schedule);
    density.observe(document.documentElement, { attributes:true, attributeFilter:['data-sidebar-density'] });
    return () => { observer.disconnect(); mutations.disconnect(); density.disconnect(); cancelAnimationFrame(frame); };
  }, [layoutKey, baseDepth]);
  const hoverPath = new Set<string>();
  let current = hovered;
  while (current && !hoverPath.has(current)) { hoverPath.add(current); current = geometry.branches.find(branch => branch.id === current)?.parent ?? null; }
  return <div ref={element} className={`sidebar-tree${baseDepth ? " sidebar-tree-children" : ""}`} onPointerOver={event => setHovered((event.target as Element).closest<HTMLElement>('.sidebar-tree-node')?.dataset.treeId ?? null)} onPointerLeave={() => setHovered(null)}>
    {render(roots, baseDepth)}
    <svg className="sidebar-tree-guides" aria-hidden="true" width="100%" height="100%" fill="none" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
      <path className="tree-guide-normal" d={[...geometry.trunks,...geometry.branches.map(branch => branch.elbow)].join(' ')} />
      {geometry.branches.filter(branch => hoverPath.has(branch.id) && !branch.active).map(branch => <path key={'hover-'+branch.id} className="tree-guide-hover" d={branch.d} />)}
      {geometry.branches.filter(branch => branch.active).map(branch => <path key={branch.id} data-active-path={branch.id} className="tree-guide-active" d={branch.d} />)}
    </svg>
  </div>;
}
