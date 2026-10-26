// Called in WKContentWorld.pageWorld; the private automation bridge stays isolated.
async (createFunction, expectedUrl, marker) => {
  // Capture serialization before invoking page code that may replace globals.
  const stringify = JSON.stringify, encode = TextEncoder.prototype.encode, encoder = new TextEncoder();
  let element;
  try {
    if (location.href !== expectedUrl) throw new Error("Page navigated before evaluation; retry on the current page");
    if (marker) {
      element = document.querySelector(`[${marker}]`);
      if (!element) throw new Error("Evaluation target disappeared; take another snapshot");
      element.removeAttribute(marker);
    }
    const fn = createFunction();
    if (typeof fn !== "function") throw new Error("Expected a JavaScript function");
    const result = await fn(element);
    const serialized = stringify({ result: result === undefined ? null : result }, (_key, value) => {
      if (value instanceof Node) throw new Error("Evaluation cannot return DOM nodes; return JSON data");
      return value;
    });
    if (encode.call(encoder, serialized).length > 200000) throw new Error("Evaluation result exceeds 200 KB");
    return serialized;
  } catch (error) {
    return stringify({ error: String(error?.message || error).slice(0, 1000) });
  } finally {
    element?.removeAttribute(marker);
  }
}
