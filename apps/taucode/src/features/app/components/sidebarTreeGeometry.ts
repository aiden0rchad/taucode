export type TreeGlyph = { x: number; y: number; width: number; height: number };
export type TreePoint = { id: string; parent: string | null; glyph: TreeGlyph; active: boolean };

export function sidebarTreeGeometry(points: TreePoint[], root?: TreeGlyph) {
  const byId = new Map(points.map(point => [point.id, point]));
  const trunks = new Map<string, { x: number; start: number; end: number }>();
  const branches = points.flatMap(point => {
    const parent = point.parent ? byId.get(point.parent)?.glyph : root;
    if (!parent) return [];
    const x = parent.x + parent.width / 2, start = parent.y + parent.height + 3;
    const y = point.glyph.y + point.glyph.height / 2, end = point.glyph.x - 4;
    const radius = Math.max(0, Math.min(8, end - x, y - start));
    const turn = radius ? `A ${radius} ${radius} 0 0 0 ${x+radius} ${y}` : `L ${x} ${y}`;
    const elbow = `M ${x} ${y-radius} ${turn} H ${end}`;
    const key = point.parent ?? '__project__', previous = trunks.get(key);
    trunks.set(key, { x, start, end: Math.max(previous?.end ?? start, y-radius) });
    return [{ id: point.id, parent: point.parent, active: point.active, elbow, d: `M ${x} ${start} V ${y-radius} ${turn} H ${end}` }];
  });
  return { branches, trunks: [...trunks.values()].map(({x,start,end}) => `M ${x} ${start} V ${end}`) };
}
