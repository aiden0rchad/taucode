// Offline OpenAI transport fixture: exercises the real omp client and RPC stream.
globalThis.fetch = async () => new Response([
  'data: {"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello from omp"},"finish_reason":null}]}\n\n',
  'data: {"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}\n\n',
  'data: [DONE]\n\n',
].join(''), { headers: { 'content-type': 'text/event-stream' } });
