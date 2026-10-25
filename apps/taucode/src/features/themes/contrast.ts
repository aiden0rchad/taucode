export function rgba(value: string): number[] | null {
  if (value.startsWith('#')) {
    let h = value.slice(1);
    if (h.length < 5) h = [...h].map(c => c+c).join('');
    return [parseInt(h.slice(0,2),16),parseInt(h.slice(2,4),16),parseInt(h.slice(4,6),16),h.length === 8 ? parseInt(h.slice(6),16)/255 : 1];
  }
  if (/^rgba?\(/.test(value)) {
    const p = value.slice(value.indexOf('(')+1,-1).trim().split(/[\s,/]+/).map((v,i) => v.endsWith('%') ? parseFloat(v) * (i === 3 ? .01 : 2.55) : Number(v));
    return p.length >= 3 ? [p[0],p[1],p[2],p[3] ?? 1] : null;
  }
  // Use the browser's color parser for hsl, oklch and color-mix (including gamut mapping).
  if (typeof CanvasRenderingContext2D !== 'undefined') {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const context = canvas.getContext('2d');
    if (context && CSS.supports('color',value)) {
      context.fillStyle = value;
      context.fillRect(0,0,1,1);
      const [r,g,b,a] = context.getImageData(0,0,1,1).data;
      return [r,g,b,a/255];
    }
  }
  return null;
}
export function composite(color: number[], background: number[]) {
  return color.slice(0,3).map((v,i) => v * color[3] + background[i] * (1-color[3])).concat(1);
}
export function contrast(text: number[], surface: number[]) {
  const luminance = (rgb: number[]) => rgb.slice(0,3).reduce((sum,v,i) => {
    const c = v / 255;
    return sum + (c <= .04045 ? c / 12.92 : ((c+.055)/1.055)**2.4) * [.2126,.7152,.0722][i];
  },0);
  const a = luminance(composite(text,surface)), b = luminance(surface);
  return (Math.max(a,b)+.05)/(Math.min(a,b)+.05);
}
export function contrastPairs(tokens: Record<string,string>) {
  const pairs: { text: string; surface: string; label: string; ratio: number | null }[] = [];
  const resolve = (key: string, seen: string[] = []): string => seen.includes(key) ? '' : (tokens[key] ?? '').replace(/var\((--[a-z0-9-]+)\)/g, (_, ref: string) => resolve(ref,[...seen,key]));
  for (const surface of ['app','sidebar','card','composer']) {
    for (const text of ['primary','muted']) {
      const a = rgba(resolve('--text-'+text)), b = rgba(resolve('--surface-'+surface));
      const ratio = a && b ? Math.min(...[0,255].map(v => contrast(a,composite(b,[v,v,v,1])))) : NaN;
      pairs.push({ text:'--text-'+text, surface:'--surface-'+surface, label:(text === 'primary' ? 'Body' : 'Muted')+' / '+surface, ratio:Number.isFinite(ratio) ? ratio : null });
    }
  }
  return pairs;
}
export function contrastWarnings(tokens: Record<string,string>) {
  return contrastPairs(tokens).filter(p => p.ratio !== null && p.ratio < 4.5).map(p => p.text+' on '+p.surface+' is below 4.5:1');
}
