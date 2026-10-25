const paths = new Map<string, string>();

// Figma-style corner smoothing: tangent cubic transitions around a circular arc.
export function squirclePath(width: number, height: number, radius = 20, smoothing = 0.6, inset = 0) {
  if (![width, height, radius, smoothing, inset].every(Number.isFinite)) return "";
  const w = Math.max(0, width - 2 * inset), h = Math.max(0, height - 2 * inset);
  const r = Math.max(0, Math.min(radius - inset, w / 2, h / 2));
  const s = r ? Math.max(0, Math.min(smoothing, 1, Math.min(w, h) / (2 * r) - 1)) : 0;
  const key = [w, h, r, s, inset].join(",");
  const cached = paths.get(key);
  if (cached) return cached;
  const p = (1 + s) * r, angle = s * Math.PI / 4;
  const d = r * (1 - Math.cos(angle));
  const c = angle ? d / Math.tan(angle) : 0;
  const b = (p - r + r * Math.sin(angle) - c) / 3, a = 2 * b;
  const n = (value: number) => String(Math.round(value * 1000) / 1000);
  const point = (corner: number, x: number, y: number) => {
    const [px, py] = [[w - p + x, y], [w - y, h - p + x], [p - x, h - y], [y, p - x]][corner];
    return `${n(px + inset)} ${n(py + inset)}`;
  };
  let path = `M ${point(0, 0, 0)}`;
  for (let corner = 0; corner < 4; corner++) {
    if (corner) path += ` L ${point(corner, 0, 0)}`;
    path += ` C ${point(corner, a, 0)} ${point(corner, a + b, 0)} ${point(corner, a + b + c, d)}`;
    if (r) path += ` A ${n(r)} ${n(r)} 0 0 1 ${point(corner, p - d, p - a - b - c)}`;
    path += ` C ${point(corner, p, p - a - b)} ${point(corner, p, p - a)} ${point(corner, p, p)}`;
  }
  path += " Z";
  if (paths.size >= 256) paths.delete(paths.keys().next().value!);
  paths.set(key, path);
  return path;
}
