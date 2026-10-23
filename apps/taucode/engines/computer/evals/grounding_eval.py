"""Opt-in live grounding eval. Only a synthetic headless page is captured."""
import asyncio
import base64
import contextlib
import io
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.parse import urlsplit, urlunsplit

# Already locked transitively by google-genai; no additional installation.
from websockets.asyncio.client import connect

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from taucode_cua.actions import StructuredACI
from taucode_cua.requests import predict_budget, redact
from taucode_cua.server import engine_params


HTML = '''<!doctype html><meta charset="utf-8"><title>Grounding eval</title>
<style>
*{box-sizing:border-box}body{margin:0;background:#eef2f7;color:#172338;font:22px Arial}
header{height:110px;background:#172338;color:white;padding:36px}h1{margin:0;font-size:30px}
nav{position:absolute;top:110px;bottom:0;width:280px;background:white;padding:32px}
button,input{font:22px Arial;border:1px solid #8090a8;border-radius:10px;padding:18px}
button{background:#245cce;color:white}input{background:white;color:#172338}
#search{position:absolute;left:1570px;top:26px;width:320px;height:58px}
#projects{display:block;margin-top:60px;width:216px;background:#e3ecff;color:#172338}
main{position:absolute;left:330px;top:160px;width:1510px;background:white;padding:36px;border-radius:18px;height:750px}
#new-project{position:absolute;right:36px;top:30px}
article{margin-top:50px;border:1px solid #ccd3df;border-radius:12px;padding:30px;height:230px}
#email{width:470px;margin-top:30px}footer{position:absolute;bottom:40px;right:40px}
</style><header><h1>Acme workspace</h1><input id="search" placeholder="Search projects"></header>
<nav>Workspace<button id="projects">Projects</button><p>Team</p><p>Reports</p></nav>
<main><h2>Projects overview</h2><button id="new-project">New project</button>
<article><h3>Website refresh</h3>Design review scheduled for Friday.</article>
<label for="email">Invite a teammate</label><br><input id="email" placeholder="Email address">
<footer><button id="save">Save changes</button></footer></main>'''
ELEMENTS = {'search': 'the Search projects field in the top right',
            'projects': 'the Projects button in the left sidebar',
            'new-project': 'the New project button',
            'email': 'the Email address input field',
            'save': 'the Save changes button'}


async def render_ui():
    chrome = next((p for p in [shutil.which('google-chrome'), shutil.which('chromium'),
                               '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome']
                   if p and Path(p).is_file()), None)
    if not chrome:
        raise RuntimeError('Install Chrome or Chromium to run this eval')
    with tempfile.TemporaryDirectory(prefix='taucode-grounding-eval-') as profile:
        page = Path(profile) / 'synthetic.html'
        page.write_text(HTML)
        process = subprocess.Popen([chrome, '--headless=new', '--no-first-run', '--no-default-browser-check',
                                    '--disable-background-networking', '--remote-debugging-port=0',
                                    f'--user-data-dir={profile}', page.as_uri()], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            port_file = Path(profile) / 'DevToolsActivePort'
            deadline = time.monotonic() + 20
            while not port_file.exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Headless Chrome did not expose CDP within 20 seconds')
                await asyncio.sleep(0.1)
            port, path = port_file.read_text().splitlines()[:2]
            endpoint = urlunsplit(('ws', f'127.0.0.1:{int(port)}', path, '', ''))
            # origin=None omits the Origin header; no --remote-allow-origins bypass.
            async with connect(endpoint, origin=None, proxy=None, max_size=32 * 1024 * 1024, open_timeout=10) as socket:
                counter = 0
                async def cdp(method, params=None, session=None):
                    nonlocal counter
                    counter += 1
                    request = {'id': counter, 'method': method, 'params': params or {}}
                    if session:
                        request['sessionId'] = session
                    await socket.send(json.dumps(request))
                    async with asyncio.timeout(20):
                        while True:
                            response = json.loads(await socket.recv())
                            if response.get('id') == counter:
                                if 'error' in response:
                                    raise RuntimeError(response['error']['message'])
                                return response['result']
                targets = await cdp('Target.getTargets')
                target = next(t['targetId'] for t in targets['targetInfos'] if t['type'] == 'page')
                session = (await cdp('Target.attachToTarget', {'targetId': target, 'flatten': True}))['sessionId']
                await cdp('Emulation.setDeviceMetricsOverride', {'width': 1920, 'height': 1080, 'deviceScaleFactor': 2, 'mobile': False}, session)
                await cdp('Page.enable', session=session)
                frame = (await cdp('Page.getFrameTree', session=session))['frameTree']['frame']['id']
                await cdp('Page.setDocumentContent', {'frameId': frame, 'html': HTML}, session)
                # Static, repo-owned JavaScript; never derived from a model response.
                expression = '''document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => {
                    resolve(Object.fromEntries(Array.from(document.querySelectorAll('[id]'), e => {
                        const r=e.getBoundingClientRect(); return [e.id, [r.x,r.y,r.width,r.height]];
                    })));
                })))'''
                boxes = (await cdp('Runtime.evaluate', {'expression': expression, 'awaitPromise': True, 'returnByValue': True}, session))['result']['value']
                screenshot = await cdp('Page.captureScreenshot', {'format': 'png', 'captureBeyondViewport': False}, session)
                return base64.b64decode(screenshot['data']), boxes
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    base_url = os.environ.get('TAUCODE_EVAL_BASE_URL', '')
    models = [m.strip() for m in os.environ.get('TAUCODE_EVAL_MODELS', '').split(',') if m.strip()]
    endpoint = urlsplit(base_url)
    if endpoint.scheme not in ('http', 'https') or not endpoint.hostname or endpoint.username or endpoint.password or not models:
        raise ValueError('Set TAUCODE_EVAL_BASE_URL (without credentials) and TAUCODE_EVAL_MODELS')
    key = os.environ.get('TAUCODE_EVAL_KEY')
    if not key and os.environ.get('TAUCODE_EVAL_KEY_COMMAND'):
        try:
            # Operator-provided argv only. No shell or model-generated commands.
            key = subprocess.run(shlex.split(os.environ['TAUCODE_EVAL_KEY_COMMAND']), check=True,
                                 capture_output=True, text=True, timeout=30).stdout.strip()
        except (OSError, subprocess.SubprocessError):
            raise ValueError('TAUCODE_EVAL_KEY_COMMAND failed') from None
    if not key:
        raise ValueError('Set TAUCODE_EVAL_KEY or TAUCODE_EVAL_KEY_COMMAND (a local provider may use a placeholder)')
    os.environ['TAUCODE_CUA_GROUNDING_KEY'] = key
    screenshot, boxes = asyncio.run(render_ui())
    for model in models:
        params = engine_params({'kind': 'openai-compatible', 'base_url': base_url, 'model': model, 'mode': 'pixels'}, 'grounding')
        aci = StructuredACI(None, 'darwin', {}, params)
        aci.assign_screenshot({'screenshot': screenshot})
        hits = 0
        for element, description in ELEMENTS.items():
            try:
                with predict_budget(), contextlib.redirect_stdout(io.StringIO()):
                    point = aci.point(description)
                # Grounding image pixels -> CSS/display points, as the Rust host does.
                x = point[0] * 1920 / params['grounding_width']
                y = point[1] * 1080 / params['grounding_height']
                left, top, width, height = boxes[element]
                hit = left <= x < left + width and top <= y < top + height
                hits += hit
                print(f'  {element}: {"HIT" if hit else "MISS"} ({x:.1f}, {y:.1f})')
            except Exception as error:
                print(f'  {element}: ERROR {redact(error)}')
        print(f'{model}: {hits}/{len(ELEMENTS)} ({hits / len(ELEMENTS):.0%})')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(redact(error), file=sys.stderr)
        sys.exit(1)
