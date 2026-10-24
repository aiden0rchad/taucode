"""Host-only JSON-RPC over stdio. Screenshots in, structured proposals out."""
import base64
import contextlib
import io
import json
import logging
import os
import sys
from PIL import Image
from .actions import StructuredACI
from .images import downscale
from .elements import usable_elements, observation_summary
from .requests import ProviderError, predict_budget, redact
from .vendor.agent_s3.agents.agent_s import AgentS3

KINDS = {'openai': 'openai', 'anthropic': 'anthropic', 'google': 'gemini',
         'openrouter': 'open_router', 'deepseek': 'deepseek', 'ollama': 'ollama',
         **dict.fromkeys(('lmstudio', 'groq', 'xai', 'mistral', 'zai', 'openai-compatible'), 'openai')}


def engine_params(config, role):
    if not isinstance(config, dict) or set(config) - {'kind', 'model', 'base_url', 'width', 'height', 'mode'}:
        raise ValueError('Invalid model configuration (credentials must use environment)')
    if config.get('kind') not in KINDS or not isinstance(config.get('model'), str) or not config['model']:
        raise ValueError('Select a registry provider and model')
    params = dict(engine_type=KINDS[config['kind']], model=config['model'],
                  api_key=os.environ.get(f'TAUCODE_CUA_{role.upper()}_KEY') or 'local')
    if config.get('base_url'):
        params['base_url'] = config['base_url']
    if role == 'grounding':
        mode = config.get('mode', 'elements')
        if mode not in ('elements', 'pixels', 'ui-tars'):
            raise ValueError('Invalid grounding mode')
        params['grounding_mode'] = mode
        for key in ('width', 'height'):
            n = config.get(key, 1920 if key == 'width' else 1080)
            if type(n) is not int or not 1 <= n <= 8192:
                raise ValueError('Invalid grounding size')
            params[f'grounding_{key}'] = n
    return params


class Server:
    def __init__(self):
        self.agent = None
        self.mode = "elements"

    def dispatch(self, method, params):
        if method == 'health':
            return {'ok': True, 'configured': self.agent is not None}
        if method == 'configure':
            if params.get('platform') != 'darwin':
                raise ValueError('Only macOS is supported')
            planner = engine_params(params['planner'], 'planner')
            grounding = engine_params(params['grounding'], 'grounding')
            aci = StructuredACI(None, 'darwin', planner, grounding)
            self.agent = AgentS3(planner, aci, platform='darwin')
            self.size = [grounding['grounding_width'], grounding['grounding_height']]
            self.mode = grounding['grounding_mode']
            return {'ok': True}
        if self.agent is None:
            raise ValueError('Configure the planner first')
        if method == 'reset':
            self.agent.reset()
            self.agent.grounding_agent.notes.clear()
            return {'ok': True}
        if method != 'predict':
            raise ValueError('Unknown method')
        instruction = params['instruction']
        if not isinstance(instruction, str) or not 0 < len(instruction) <= 32768:
            raise ValueError('Invalid instruction')
        size = params['screen_size']
        if not isinstance(size, list) or len(size) != 2 or any(type(n) is not int or not 1 <= n <= 16384 for n in size):
            raise ValueError('Invalid screen size')
        png = base64.b64decode(params['screenshot_png_b64'], validate=True)
        with Image.open(io.BytesIO(png)) as img:
            if img.format != 'PNG' or list(img.size) != size:
                raise ValueError('Expected PNG matching screen size')
            img.verify()
        snapshot = params.get('ax_snapshot')
        if snapshot is not None:
            usable_elements(snapshot)
        with predict_budget():
            planner_png, size = downscale(png)
            # Frames arrive in window-local points; scale to the planner image.
            if snapshot is not None:
                bounds = snapshot.get('size')
                if not isinstance(bounds, list) or len(bounds) != 2 or any(type(n) not in (int, float) or not 0 < n <= 16384 for n in bounds):
                    raise ValueError('Invalid snapshot size')
                snapshot = {**snapshot, 'elements': [{**e, 'frame': [v * size[i % 2] / bounds[i % 2] for i, v in enumerate(e['frame'])] if e.get('frame') else None} for e in snapshot['elements']]}
            info, actions = self.agent.predict(instruction, {'screenshot': planner_png, 'grounding_screenshot': png, 'ax_snapshot': snapshot,
                'element_summary': observation_summary(snapshot['elements'], snapshot.get('menus')) if snapshot else ''})
        action = actions[0]
        action['grounding_width'], action['grounding_height'] = self.size if self.mode == 'ui-tars' else size
        return {'action': action, 'reflection': info.get('reflection') or '', 'plan_text': info.get('plan') or '',
                'grounding': getattr(getattr(self.agent, 'grounding_agent', None), 'grounding_decisions', [])}


def rpc_error(error, method):
    if isinstance(error, ProviderError):
        return {'code': -32000, 'message': redact(error)}
    if method == 'configure':
        return {'code': -32602, 'message': 'Configuration error: ' + redact(error)}
    if isinstance(error, (ValueError, TypeError, KeyError, OSError)):
        return {'code': -32602, 'message': 'Input error: ' + redact(error)}
    return {'code': -32603, 'message': 'Planner error: ' + redact(error)}


def main():
    server = Server()
    # Upstream libraries print diagnostics; never let these corrupt JSON-RPC stdout.
    output = sys.stdout
    logging.basicConfig(stream=sys.stderr, level=logging.WARNING)
    while line := sys.stdin.buffer.readline(32 * 1024 * 1024 + 1):
        request = {}
        try:
            if len(line) > 32 * 1024 * 1024:
                raise ValueError('Request too large')
            request = json.loads(line)
            if not isinstance(request, dict) or request.get('jsonrpc') != '2.0':
                raise ValueError('Invalid JSON-RPC request')
            with contextlib.redirect_stdout(sys.stderr):
                result = server.dispatch(request['method'], request.get('params', {}))
            response = {'jsonrpc': '2.0', 'id': request.get('id'), 'result': result}
        except Exception as error:
            response = {'jsonrpc': '2.0', 'id': request.get('id') if isinstance(request, dict) else None,
                        'error': {**rpc_error(error, request.get('method') if isinstance(request, dict) else None),
                                  'data': {'grounding': getattr(getattr(server.agent, 'grounding_agent', None), 'grounding_decisions', [])}}}
        output.write(json.dumps(response, allow_nan=False) + '\n')
        output.flush()


if __name__ == '__main__':
    main()
