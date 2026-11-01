// The engine and model of the conversation in front, for prefilled issue reports.
let current: { engine?: string; model?: string } = {};

export function setIssueReportContext(value: { engine?: string | null; model?: string | null }) {
  current = { engine: value.engine ?? undefined, model: value.model ?? undefined };
}

export function getIssueReportContext() {
  return current;
}

/** Hide every modal for a moment so the screenshot shows the app, not this dialog. */
export async function withModalsHidden<T>(run: () => Promise<T>): Promise<T> {
  const root = document.documentElement;
  root.classList.add("taucode-capturing");
  try {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    await new Promise(resolve => setTimeout(resolve, 80));
    return await run();
  } finally {
    root.classList.remove("taucode-capturing");
  }
}
