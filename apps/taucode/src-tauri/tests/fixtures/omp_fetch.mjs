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
  const request = JSON.parse(options?.body ?? await _url.text());
  const messages = JSON.stringify(request.messages);
  if (messages.includes('LIMIT_TIMER_')) {
    const child = request.messages.some(message => message.role === 'user' && JSON.stringify(message.content).includes('Task: LIMIT_TIMER_CHILD'));
    const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
    const command = `${quote(process.execPath)} -e ${quote("require('node:fs').writeFileSync('timer.pid',String(process.pid)); console.log('LIMIT_TIMER_READY'); setTimeout(()=>{},90000)")}`;
    const tool = child ? 'bash' : 'subagent';
    const args = child ? { command, timeout: 100 } : { agent: 'enlisted', task: 'LIMIT_TIMER_CHILD' };
    const chunk = { id: 'deadline', object: 'chat.completion.chunk', created: 1, model: 'stub', choices: [{ index: 0, delta: { role: 'assistant', tool_calls: [{ index: 0, id: 'deadline-call', type: 'function', function: { name: tool, arguments: JSON.stringify(args) } }] }, finish_reason: 'tool_calls' }], usage: { prompt_tokens: 2, completion_tokens: 3, total_tokens: 5 } };
    return new Response(`data: ${JSON.stringify(chunk)}\n\ndata: [DONE]\n\n`, { headers: { 'content-type': 'text/event-stream' } });
  }
  if (/QA_(PASS|MISSING|FAIL)_FIXTURE/.test(messages)) {
    const calls = request.messages.filter(message => message.role === 'tool').length;
    const major = process.env.TAUCODE_SUBAGENT_ROLE === 'major' || request.messages.some(message => message.role === 'user' && JSON.stringify(message.content).includes('Task: QA_'));
    const marker = messages.match(/QA_(PASS|MISSING|FAIL)_FIXTURE/)[0];
    let tool, args;
    if (!calls) {
      tool = major || marker === 'QA_MISSING_FIXTURE' ? 'read' : 'subagent';
      args = tool === 'read' ? { path: 'answer.txt' } : { agent: 'major', task: marker, acceptanceCriteria: ['answer.txt contains 42'] };
    } else if (major && calls === 1) {
      if (!messages.includes('Verification evidence: E1')) throw new Error('QA evidence hook did not run');
      tool = 'report_verification';
      args = { checks: [{ criterion: 'answer.txt contains 42', passed: marker === 'QA_PASS_FIXTURE', evidence: ['E1'], note: 'Read the artifact using the read tool' }], artifacts: ['answer.txt'] };
    }
    const delta = tool ? { role: 'assistant', tool_calls: [{ index: 0, id: `qa-${calls}`, type: 'function', function: { name: tool, arguments: JSON.stringify(args) } }] } : { role: 'assistant', content: 'Done' };
    const chunk = { id: 'qa', object: 'chat.completion.chunk', created: 1, model: 'stub', choices: [{ index: 0, delta, finish_reason: tool ? 'tool_calls' : 'stop' }], usage: { prompt_tokens: 2, completion_tokens: 3, total_tokens: 5 } };
    return new Response(`data: ${JSON.stringify(chunk)}\n\ndata: [DONE]\n\n`, { headers: { 'content-type': 'text/event-stream' } });
  }
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

// Pi supplies its own fetch implementation; serve the same fixture over loopback.
if (process.argv.includes('--serve')) {
  const { createServer } = await import('node:http');
  let requests = 0;
  const server = createServer(async (request, response) => {
    if (request.url === '/count') { response.end(String(requests)); return; }
    requests++;
    try {
      const chunks = [];
      for await (const chunk of request) chunks.push(chunk);
      const result = await globalThis.fetch('', { body: Buffer.concat(chunks).toString() });
      response.writeHead(result.status, Object.fromEntries(result.headers));
      response.end(await result.text());
    } catch (error) { response.writeHead(500); response.end(String(error)); }
  });
  server.listen(0, '127.0.0.1', () => process.stdout.write(`${server.address().port}\n`));
}
