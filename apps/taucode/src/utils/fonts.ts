export const DEFAULT_UI_FONT_FAMILY =
  '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

export const DEFAULT_CODE_FONT_FAMILY =
  'ui-monospace, "SF Mono", Menlo, "Cascadia Mono", "Segoe UI Mono", Monaco, Consolas, "Liberation Mono", "Courier New", monospace';

export const CODE_FONT_SIZE_DEFAULT = 11;
export const CODE_FONT_SIZE_MIN = 9;
export const CODE_FONT_SIZE_MAX = 16;

export function normalizeFontFamily(
  value: string | null | undefined,
  fallback: string,
) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : fallback;
}

export function clampCodeFontSize(value: number) {
  if (!Number.isFinite(value)) {
    return CODE_FONT_SIZE_DEFAULT;
  }
  return Math.min(CODE_FONT_SIZE_MAX, Math.max(CODE_FONT_SIZE_MIN, value));
}
