import { createContext, useContext, type ReactNode } from "react";
import type { EngineKind, WorkspaceInfo } from "@/types";

export const SidebarProjects = createContext<WorkspaceInfo[]>([]);
export const SidebarEngine = createContext<EngineKind>("pi");
export function ProjectEngine({ engine, children }: { engine?: EngineKind; children: ReactNode }) {
  const inherited = useContext(SidebarEngine);
  return <SidebarEngine.Provider value={engine ?? inherited}>{children}</SidebarEngine.Provider>;
}

// Build once in linear time; nested wrappers let CSS draw continuous branches.
export function SidebarTree<T extends { depth: number }>({ rows, rowKey, selected, children, baseDepth = 1 }: {
  rows: T[]; rowKey: (row: T) => string; selected: (row: T) => boolean;
  children: (row: T) => ReactNode; baseDepth?: number;
}) {
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
    <div key={rowKey(node.row)} className={`sidebar-tree-node${depth ? " tree-branch" : ""}${index === nodes.length - 1 ? " tree-last-child" : ""}${node.path ? " tree-path" : ""}`} data-depth={depth}>
      {children(node.row)}
      {node.children.length > 0 && <div className="sidebar-tree-children">{render(node.children, depth + 1)}</div>}
    </div>);
  return <div className={`sidebar-tree${baseDepth ? " sidebar-tree-children" : ""}`}>{render(roots, baseDepth)}</div>;
}
