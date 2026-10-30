// Document-start page-world hooks. This state has no access to the private bridge.
(key) => {
  if (window !== window.top || Object.prototype.hasOwnProperty.call(globalThis, key)) return;
  const freeze = Object.freeze, now = Date.now, stringify = JSON.stringify;
  const slice = Function.call.bind(String.prototype.slice);
  const text = (value) => {
    try {
      return slice(typeof value === "string" ? value : stringify(value) ?? String(value), 0, 2000);
    } catch {
      try { return slice(String(value), 0, 2000); } catch { return "[unprintable]"; }
    }
  };
  let messages = freeze([]), dialogs = freeze([]), answer = null, unseen = false;
  const record = (type, value) => {
    messages = freeze([...messages.slice(-499), freeze({ type, text: text(value), timestamp: now() })]);
  };
  for (const type of ["log", "info", "warn", "error", "debug"]) {
    const original = console[type].bind(console);
    console[type] = (...args) => {
      try {
        let value = "";
        for (const arg of args) {
          value = slice(value + (value ? " " : "") + text(arg), 0, 2000);
          if (value.length >= 2000) break;
        }
        record(type, value);
      } catch { /* Instrumentation must never throw into the page. */ }
      try { original(...args); } catch { /* Keep page code running. */ }
    };
  }
  addEventListener("error", (event) => {
    try { record("error", `${text(event.message)} (${text(event.filename)}:${text(event.lineno)})`); } catch { /* Ignore malformed events. */ }
  });
  // Tauri injects app plugin scripts into every webview; their IPC is denied in browser tabs
  // ("notification.is_permission_granted not allowed"). That is taucode noise, not a page error.
  const tauriDenial = /^[\w-]+\.[\w-]+ not allowed\b/;
  addEventListener("unhandledrejection", (event) => {
    try {
      const reason = text(event.reason?.message ?? event.reason);
      if (tauriDenial.test(reason)) { event.preventDefault(); return; }
      record("error", `Unhandled promise rejection: ${reason}`);
    } catch { /* Ignore malformed events. */ }
  });
  // Read-only: the Tauri dialog plugin's init script assigns async IPC versions of these into
  // every webview; in a browser tab that IPC is rejected, so its strict assignment must fail.
  for (const type of ["alert", "confirm", "prompt"]) {
    const handler = (message = "", defaultValue = "") => {
      const armed = answer;
      answer = null;
      let fallback = "";
      try {
        fallback = text(defaultValue);
        dialogs = freeze([...dialogs.slice(-49), freeze({ type, message: text(message), defaultValue: fallback,
          answered: armed !== null, timestamp: now(), accepted: armed?.accept === true })]);
        unseen = armed === null;
      } catch { /* Dialogs still return their default answer if recording fails. */ }
      if (type === "confirm") return armed?.accept === true;
      if (type === "prompt") return armed?.accept ? armed.promptText ?? fallback : null;
    };
    Object.defineProperty(globalThis, type, { value: handler, writable: false, configurable: true, enumerable: true });
  }
  const request = (params) => {
    if (params.action === "console") {
      const levels = { error: 0, warning: 1, info: 2, debug: 3 };
      // `all` has the same history here: this buffer starts at document load.
      const level = params.onlyErrors === true ? "error" : params.level ?? "info";
      if (!Object.prototype.hasOwnProperty.call(levels, level)) throw new Error("Invalid console level");
      const ranks = { error: 0, warn: 1, log: 2, info: 2, debug: 3 };
      return messages.filter((entry) => ranks[entry.type] <= levels[level]);
    }
    const latest = dialogs[dialogs.length - 1];
    const description = latest ? `${latest.type}(${stringify(latest.message)}) was ${latest.accepted ? "accepted" : "dismissed"}${latest.answered ? " using an armed answer" : " automatically"}.` : "No dialogs recorded.";
    if (params.action === "dialog") {
      if (typeof params.accept !== "boolean") throw new Error("Missing dialog accept boolean");
      if (params.promptText != null && typeof params.promptText !== "string") throw new Error("Invalid dialog promptText");
      answer = freeze({ accept: params.accept, promptText: params.promptText == null ? null : text(params.promptText) });
      return `${description} The next dialog will be ${answer.accept ? "accepted" : "dismissed"}; repeat the action that opens it`;
    }
    if (params.action === "snapshot") {
      const note = unseen ? `Dialog: ${description} Use browser_handle_dialog to arm an answer, then repeat the action.` : "";
      unseen = false;
      return note;
    }
    throw new Error("Unknown page hook action");
  };
  Object.defineProperty(globalThis, key, { value: freeze({ request, get messages() { return messages; }, get dialogs() { return dialogs; } }) });
}
