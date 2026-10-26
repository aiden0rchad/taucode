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
  const target = ref => { const el = nodes.get(ref); if (!el?.isConnected) throw new Error(`Stale or unknown ref ${ref}; take another snapshot. ${hint}`); return el; };
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
      } else if (p.op === "wait") {
        result = (!p.ref || nodes.get(p.ref)?.isConnected === true) && (!p.text || document.body?.innerText.includes(p.text)) && (!p.textGone || !document.body?.innerText.includes(p.textGone));
      } else {
        const el = p.ref ? target(p.ref) : document.activeElement || document.body;
        if (el.disabled) throw new Error(`Element is disabled. ${hint}`);
        if (p.op === "click") {
          el.scrollIntoView({ block: "center" }); el.focus();
          for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) el.dispatchEvent(new (type.startsWith("pointer") ? PointerEvent : MouseEvent)(type, { bubbles: true, cancelable: true, button: 0 }));
          el.click();
        } else if (p.op === "type") {
          if (typeof p.text !== "string" || p.text.length > 200000) throw new Error("Invalid text");
          el.focus();
          if (el.isContentEditable) { document.execCommand("selectAll"); document.execCommand("insertText", false, p.text); }
          else {
            if (el.readOnly) throw new Error("Element is read-only");
            const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
            Object.getOwnPropertyDescriptor(proto, "value").set.call(el, p.text);
            el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: p.text }));
            el.dispatchEvent(new Event("change", { bubbles: true }));
          }
          if (p.submit) press(el, "Enter");
        } else if (p.op === "press") press(el, p.key);
        else if (p.op === "select") {
          if (!(el instanceof HTMLSelectElement) || !Array.isArray(p.values)) throw new Error("Expected a select and values");
          for (const value of p.values) if (![...el.options].some(o => o.value === value)) throw new Error("Unknown option");
          for (const option of el.options) option.selected = p.values.includes(option.value);
          el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
        } else if (p.op === "scroll") (p.ref ? el : window).scrollBy(Number(p.x) || 0, Number(p.y) || 0);
        else throw new Error(`Unsupported native operation. ${hint}`);
        result = { performed: true, note: `Synthetic input is not isTrusted. If the site ignored it, ${hint}` };
      }
      return JSON.stringify({ result });
    } catch (e) { return JSON.stringify({ error: `${e.message}. ${hint}` }); }
  };
})();
