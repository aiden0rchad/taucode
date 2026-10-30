// Offline OpenAI transport fixture: exercises the real omp client and RPC stream.
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
function crashAfterCheckpoint(marker) {
  const directory = `${process.argv[process.argv.indexOf('--session') + 1]}.streams`;
  let checks = 0;
  const timer = setInterval(() => {
    let saved = false;
    try { saved = readdirSync(directory).some(file => file.endsWith('.json') && readFileSync(join(directory, file), 'utf8').includes(marker)); } catch { /* Writer has not created the directory yet. */ }
    if (saved || ++checks === 500) { clearInterval(timer); process.kill(process.pid, 'SIGKILL'); }
  }, 10);
}
globalThis.fetch = async (_url, options) => {
  const request = JSON.parse(options.body);
  const messages = JSON.stringify(request.messages);
  const stop = messages.includes('GUARD_STOP_FIXTURE');
  if (stop || (messages.includes('GUARD_NATIVE_FIXTURE') && !messages.includes('Say hello after supervisor'))) {
    const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
    const command = `${quote(process.execPath)} -e ${quote("console.log('GUARD_NATIVE_READY '+process.pid); setTimeout(()=>{},10000)")}`;
    if (!stop) crashAfterCheckpoint('GUARD_NATIVE_READY ');
    const chunk = { id: 'guard', object: 'chat.completion.chunk', created: 1, model: 'stub', choices: [{ index: 0, delta: { role: 'assistant', tool_calls: [{ index: 0, id: stop ? 'stop-call' : 'guard-call', type: 'function', function: { name: 'bash', arguments: JSON.stringify({ command, timeout: 20 }) } }] }, finish_reason: 'tool_calls' }] };
    return new Response(`data: ${JSON.stringify(chunk)}\n\ndata: [DONE]\n\n`, { headers: { 'content-type': 'text/event-stream' } });
  }
  if (messages.includes('STREAM_CRASH_FIXTURE') && !messages.includes('Say hello again')) {
    return new Response(new ReadableStream({ start(controller) {
      controller.enqueue(new TextEncoder().encode('data: {"id":"crash","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{"role":"assistant","content":"ROOT_PARTIAL"},"finish_reason":null}]}\n\n'));
      crashAfterCheckpoint('ROOT_PARTIAL');
    } }), { headers: { 'content-type': 'text/event-stream' } });
  }
  return new Response([
  'data: {"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello from omp"},"finish_reason":null}]}\n\n',
  'data: {"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}\n\n',
  'data: [DONE]\n\n',
].join(''), { headers: { 'content-type': 'text/event-stream' } });
};
