// Evaluated only in WKContentWorld("taucode.browser.private"), never the page world.
(() => {
  if (globalThis.__tauBrowser) return;
  const refs = new WeakMap();
  let nodes = new Map(), next = 0, userInput = false;
  for (const event of ["pointerdown", "keydown", "wheel", "touchstart"]) {
    addEventListener(event, e => { if (e.isTrusted) userInput = true; }, { capture: true, passive: true });
  }
  const hint = "Try the Headless Chromium backend (Settings → Browser)";
  const role = el => el.getAttribute("role") || ({ A: "link", BUTTON: "button", SELECT: "combobox", TEXTAREA: "textbox", NAV: "navigation", MAIN: "main", HEADER: "banner", FOOTER: "contentinfo", FORM: "form", H1: "heading", H2: "heading", H3: "heading" }[el.tagName]) || (el.tagName === "INPUT" ? ({ checkbox: "checkbox", radio: "radio", range: "slider", button: "button", submit: "button" }[el.type] || "textbox") : el.isContentEditable ? "textbox" : "generic");
  const name = el => (el.getAttribute("aria-label") || (el.getAttribute("aria-labelledby") || "").split(/\s+/).map(id => document.getElementById(id)?.textContent || "").join(" ").trim() || [...(el.labels || [])].map(l => l.textContent).join(" ") || el.getAttribute("alt") || el.getAttribute("placeholder") || el.textContent || "").trim().slice(0, 1000);
  const target = ref => {
    if (typeof ref !== "string" || !ref.trim()) throw new Error("Missing target");
    let el = nodes.get(ref);
    if (!el && !/^e\d+$/.test(ref)) {
      let matches;
      try { matches = document.querySelectorAll(ref); } catch { throw new Error("Invalid selector; use a snapshot ref or unique CSS selector"); }
      if (matches.length !== 1) throw new Error(`Target selector matched ${matches.length} elements; expected one`);
      el = matches[0];
    }
    if (!el?.isConnected) throw new Error(`Stale or unknown ref ${ref}; take another snapshot. ${hint}`);
    return el;
  };
  const editable = el => {
    if (el.matches(":disabled") || el.closest('[inert],[aria-disabled="true"]')) throw new Error("Element is disabled");
    if (el.readOnly) throw new Error("Element is read-only");
  };
  function textField(el, text) {
    editable(el);
    if (typeof text !== "string" || text.length > 200000) throw new Error("Invalid text");
    if (!el.isContentEditable && !(el instanceof HTMLTextAreaElement) && !(el instanceof HTMLInputElement && !["checkbox", "radio", "file", "button", "submit", "reset", "hidden", "image", "range", "color"].includes(el.type))) throw new Error("Expected an editable text field");
  }
  function type(el, text) {
    textField(el, text); el.focus();
    if (el.isContentEditable) {
      const range = document.createRange(); range.selectNodeContents(el);
      const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range);
      if (!document.execCommand("insertText", false, text)) throw new Error("Text insertion failed");
    } else {
      const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, "value").set.call(el, text);
      el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    }
  }
  function select(el, values) {
    editable(el);
    if (!(el instanceof HTMLSelectElement) || !Array.isArray(values) || values.some(v => typeof v !== "string") || (!el.multiple && values.length !== 1)) throw new Error("Expected a select and valid values");
    for (const value of values) if (![...el.options].some(o => o.value === value && !o.disabled && !o.parentElement.disabled)) throw new Error("Unknown option or option is disabled");
    for (const option of el.options) option.selected = values.includes(option.value);
    el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
  }
  function press(el, key) {
    const parts = key.split("+");
    const last = parts.pop();
    key = ({ ESC: "Escape", RETURN: "Enter", ARROWDOWN: "ArrowDown", ARROWUP: "ArrowUp" })[last.toUpperCase()] || last;
    const opts = { key, bubbles: true, cancelable: true, ctrlKey: parts.includes("Control"), metaKey: (parts.includes("Meta") || parts.includes("ControlOrMeta")), altKey: parts.includes("Alt"), shiftKey: parts.includes("Shift") };
    const accepted = el.dispatchEvent(new KeyboardEvent("keydown", opts));
    if (accepted && key === "Enter" && el.form) el.form.requestSubmit();
    if (accepted && key === " " && (el.tagName === "BUTTON" || ["checkbox", "radio"].includes(el.type))) el.click();
    if (accepted && (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) && !el.readOnly && el.selectionStart !== null) {
      let start = el.selectionStart, end = el.selectionEnd;
      if ((opts.ctrlKey || opts.metaKey) && key.toLowerCase() === "a") el.select();
      else if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) {
        const pos = key === "Home" ? 0 : key === "End" ? el.value.length : key === "ArrowLeft" ? Math.max(0, start - 1) : Math.min(el.value.length, end + 1);
        el.setSelectionRange(opts.shiftKey ? Math.min(start, pos) : pos, opts.shiftKey ? Math.max(end, pos) : pos);
      } else if (["Backspace", "Delete"].includes(key) || (key.length === 1 && !opts.metaKey && !opts.ctrlKey && !opts.altKey)) {
        if (start === end && key === "Backspace") start = Math.max(0, start - 1);
        if (start === end && key === "Delete") end = Math.min(el.value.length, end + 1);
        const text = key.length === 1 ? key : "";
        const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
        Object.getOwnPropertyDescriptor(proto, "value").set.call(el, el.value.slice(0, start) + text + el.value.slice(end));
        el.setSelectionRange(start + text.length, start + text.length);
        el.dispatchEvent(new InputEvent("input", { bubbles: true, data: text, inputType: text ? "insertText" : "deleteContentBackward" }));
      }
    }
    if (accepted && key === "Escape") el.blur();
    if (accepted && key === "Tab") {
      const list = [...document.querySelectorAll('a[href],button,input,textarea,select,[tabindex="0"]')].filter(e => !e.disabled && e.getClientRects().length);
      list[(list.indexOf(el) + (opts.shiftKey ? -1 : 1) + list.length) % list.length]?.focus();
    }
    el.dispatchEvent(new KeyboardEvent("keyup", opts));
  }
  globalThis.__tauBrowser = p => {
    try {
      if (p.expectedUrl && location.href !== p.expectedUrl) throw new Error("Page navigated before action; retry on the current page");
      let result = null;
      if (p.op === "meta") {
        result = { title: document.title, url: location.href, favicon: document.querySelector('link[rel~="icon"]')?.href || null, userInput };
        userInput = false;
      } else if (p.op === "snapshot") {
        const items = []; nodes = new Map(); let bytes = 2, truncated = false;
        for (const el of document.querySelectorAll('a[href],button,input,textarea,select,[role],[contenteditable="true"],nav,main,header,footer,form,h1,h2,h3')) {
          if (el.hidden || el.closest('[aria-hidden="true"],[inert]') || !el.getClientRects().length) continue;
          const rect = el.getBoundingClientRect();
          let ref = refs.get(el); if (!ref) { ref = `e${++next}`; refs.set(el, ref); }
          const item = { ref, role: role(el), name: name(el), value: el.type === "password" ? undefined : typeof el.value === "string" ? el.value.slice(0, 1000) : undefined, bounds: { x: rect.x, y: rect.y, w: rect.width, h: rect.height } };
          // Reserve room for the outer envelope within the 200 KB UTF-8 cap.
          bytes += new TextEncoder().encode(JSON.stringify(item)).length + 1;
          if (items.length >= 1000 || bytes > 199000) { truncated = true; break; }
          items.push(item); nodes.set(ref, el);
        }
        result = { nodes: items, truncated };
      } else if (p.op === "mark" || p.op === "unmark") {
        if (!/^data-taucode-evaluate-[a-f0-9-]{36}$/.test(p.marker)) throw new Error("Invalid evaluation marker");
        if (p.op === "mark") target(p.ref).setAttribute(p.marker, "");
        else document.querySelectorAll(`[${p.marker}]`).forEach(el => el.removeAttribute(p.marker));
      } else if (p.op === "fill") {
        if (!Array.isArray(p.fields) || p.fields.length > 1000) throw new Error("Invalid fields");
        const fields = p.fields.map(field => {
          const el = target(field.target ?? field.ref); editable(el);
          if (typeof field.value !== "string" || field.value.length > 200000) throw new Error("Invalid field value");
          let value = field.value;
          if (field.type === "textbox") textField(el, value);
          else if (field.type === "checkbox" || field.type === "radio") {
            if (!(el instanceof HTMLInputElement) || el.type !== field.type || !["true", "false"].includes(value) || (field.type === "radio" && value !== "true")) throw new Error("Expected a checkbox/radio and boolean value (radio must be true)");
          } else if (field.type === "combobox") {
            if (!(el instanceof HTMLSelectElement) || el.multiple) throw new Error("Expected a single select");
            const options = [...el.options].filter(o => o.label === value || o.value === value);
            if (options.length !== 1 || options[0].disabled || options[0].parentElement.disabled) throw new Error("Unknown, ambiguous or disabled option");
            value = options[0].value;
          } else if (field.type === "slider") {
            if (!(el instanceof HTMLInputElement) || el.type !== "range" || !value.trim() || !Number.isFinite(Number(value))) throw new Error("Expected a slider and numeric value");
          } else throw new Error("Unsupported field type");
          return { el, kind: field.type, value };
        });
        for (const { el, kind, value } of fields) {
          editable(el); if (!el.isConnected) throw new Error("Field was removed; take another snapshot");
          if (kind === "textbox") type(el, value);
          else if (kind === "combobox") select(el, [value]);
          else if (kind === "slider") {
            Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, value);
            el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
          } else if (el.checked !== (value === "true")) el.click();
        }
        result = { performed: true, note: `Synthetic input is not isTrusted. If the site ignored it, ${hint}` };
      } else if (p.op === "wait") {
        result = (!p.ref || nodes.get(p.ref)?.isConnected === true) && (!p.text || document.body?.innerText.includes(p.text)) && (!p.textGone || !document.body?.innerText.includes(p.textGone));
      } else {
        const el = p.ref ? target(p.ref) : document.activeElement || document.body;
        if (el.matches(":disabled") || el.closest('[inert],[aria-disabled="true"]')) throw new Error("Element is disabled");
        if (p.op === "click") {
          el.scrollIntoView({ block: "center" }); el.focus();
          for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) el.dispatchEvent(new (type.startsWith("pointer") ? PointerEvent : MouseEvent)(type, { bubbles: true, cancelable: true, button: 0 }));
          el.click();
        } else if (p.op === "type") {
          type(el, p.text);
          if (p.submit) press(el, "Enter");
        } else if (p.op === "press") press(el, p.key);
        else if (p.op === "select") select(el, p.values);
        else if (p.op === "scroll") (p.ref ? el : window).scrollBy(Number(p.x) || 0, Number(p.y) || 0);
        else throw new Error(`Unsupported native operation. ${hint}`);
        result = { performed: true, note: `Synthetic input is not isTrusted. If the site ignored it, ${hint}` };
      }
      return JSON.stringify({ result });
    } catch (e) { return JSON.stringify({ error: `${e.message}. ${hint}` }); }
  };
})();
