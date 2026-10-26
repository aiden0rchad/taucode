import type { BrowserSettings } from "@/types";

export const defaultBrowserSettings: BrowserSettings = { engine: "native", privateBrowsing: false, binary: null, allowedOrigins: [], blockedOrigins: [], approval: "ask", searchUrl: "https://www.google.com/search?q={query}" };
export type BrowserState = {
  tabs: { id: string; title: string; url: string; favicon?: string | null; loading: boolean; ready?: boolean; snapshot?: string; progress?: number; canGoBack?: boolean; canGoForward?: boolean; error?: string }[];
  active: string; frame?: string | null; frameTarget?: string; width?: number; height?: number; dpr?: number;
  backend?: "native" | "cdp"; takeover?: number; externalRequest?: string; newWindow?: string; downloadRequest?: string;
  agent?: boolean; cursor?: string; error?: string;
};
export function normalizeBrowserUrl(input: string, searchUrl: string): string {
  const value = input.trim();
  if (!value) throw new Error("Enter a URL or search term");
  if (value === "about:blank") return value;
  let url: string;
  if (/^https?:\/\//i.test(value)) url = value;
  else if (/^[a-z][a-z\d+.-]*:/i.test(value) && !/^localhost:\d+/i.test(value) && !/^[\w.-]+:\d+(?:\/|$)/.test(value)) throw new Error("Use an http or https URL");
  else if (!/\s/.test(value) && (/^[\w.-]+\.[\w.-]+(?::\d+)?(?:[/?#]|$)/.test(value) || /^localhost(?::\d+)?(?:\/|$)/.test(value) || /^\[[\da-f:]+\]/i.test(value))) url = `https://${value}`;
  else url = searchUrl.split("{query}").join(encodeURIComponent(value));
  const parsed = new URL(url);
  if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("Use an http or https URL");
  return parsed.href;
}
export function modifiers(e: { altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
  return Number(e.altKey) | (Number(e.ctrlKey) << 1) | (Number(e.metaKey) << 2) | (Number(e.shiftKey) << 3);
}
export function drawBrowserFrame(canvas: HTMLCanvasElement, image: CanvasImageSource, width: number, height: number, dpr: number) {
  canvas.width = Math.round(width * dpr); canvas.height = Math.round(height * dpr);
  canvas.getContext("2d")?.drawImage(image, 0, 0, canvas.width, canvas.height);
}
