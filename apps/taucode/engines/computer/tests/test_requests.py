import copy
import io
import json
import time
from types import SimpleNamespace

import pytest
from taucode_cua import requests, server
from taucode_cua.requests import ProviderError, predict_budget, provider_request, redact
from taucode_cua.vendor.agent_s3.core.engine import LMMEngineOpenAI, LMMEngineAnthropic
from taucode_cua.vendor.agent_s3.utils.common_utils import call_llm_formatted


class HTTPError(Exception):
    def __init__(self, status, message='Provider rejected the request'):
        super().__init__(message)
        self.status_code = status
        self.body = {'error': {'message': message}}


@pytest.fixture(autouse=True)
def clear_routes():
    requests._no_system.clear()


def client(engine, outcomes):
    calls = []
    def create(**kwargs):
        calls.append(copy.deepcopy(kwargs))
        outcome = outcomes.pop(0)
        if isinstance(outcome, Exception):
            raise outcome
        return SimpleNamespace(choices=[SimpleNamespace(message=SimpleNamespace(content=outcome))])
    engine.llm_client = SimpleNamespace(chat=SimpleNamespace(completions=SimpleNamespace(create=create)))
    return calls


@pytest.mark.parametrize('status', [400, 401, 403, 404, 422])
def test_permanent_errors_are_not_retried_by_any_layer(monkeypatch, status):
    engine = LMMEngineOpenAI(model='fake', api_key='test')
    calls = client(engine, [HTTPError(status, 'Invalid model')])
    monkeypatch.setattr(requests.time, 'sleep', lambda _: pytest.fail('Backoff on permanent error'))
    agent = SimpleNamespace(messages=[], engine=engine, get_response=lambda **kw: engine.generate(**kw))
    with pytest.raises(ProviderError, match='Invalid model'):
        call_llm_formatted(agent, [lambda _: (False, 'Try again')])
    assert len(calls) == 1


@pytest.mark.parametrize('error', [HTTPError(s) for s in (408, 409, 429, 500, 503)] + [ConnectionError('network')])
def test_transient_errors_retry(monkeypatch, error):
    engine = LMMEngineOpenAI(model='fake', api_key='test')
    calls = client(engine, [error, 'ok'])
    sleeps = []
    monkeypatch.setattr(requests.time, 'sleep', sleeps.append)
    assert engine.generate([]) == 'ok'
    assert len(calls) == 2 and sleeps == [1]
    assert all(0 < call['timeout'] <= 60 for call in calls)


def test_one_budget_includes_multiple_calls_and_backoff(monkeypatch):
    clock = [0]
    monkeypatch.setattr(requests.time, 'monotonic', lambda: clock[0])
    monkeypatch.setattr(requests.time, 'sleep', lambda seconds: clock.__setitem__(0, clock[0] + seconds))
    engine = LMMEngineOpenAI(model='fake', api_key='test')
    calls = client(engine, ['first'] + [HTTPError(429)] * 30)
    with predict_budget():
        assert engine.generate([]) == 'first'
        clock[0] += 40
        with pytest.raises(ProviderError, match='60 second.*Provider rejected the request'):
            engine.generate([])
    assert clock[0] == 60
    assert len(calls) < 10


def test_hung_request_is_interrupted():
    class Engine:
        model = 'fake'
        @provider_request
        def generate(self, messages, **kwargs):
            time.sleep(1)
    start = time.monotonic()
    with pytest.raises(ProviderError, match='prediction limit'), predict_budget(0.03):
        Engine().generate([])
    assert time.monotonic() - start < 0.5


def test_system_role_fallback_cached_per_route_without_mutating_history():
    messages = [{'role': 'system', 'content': [{'type': 'text', 'text': 'instructions'}]},
                {'role': 'user', 'content': [{'type': 'image_url', 'image_url': {'url': 'data:image/png;base64,abc'}}, {'type': 'text', 'text': 'task'}]}]
    original = copy.deepcopy(messages)
    engine = LMMEngineOpenAI(base_url='http://fake-route/v1', model='fake', api_key='test')
    calls = client(engine, [HTTPError(400, 'System messages are not allowed'), 'ok', 'cached'])
    assert engine.generate(messages) == 'ok'
    assert engine.generate(messages) == 'cached'
    assert calls[0]['messages'] == original
    assert calls[1]['messages'] == calls[2]['messages'] == [{'role': 'user', 'content': [original[0]['content'][0]] + original[1]['content']}]
    assert messages == original
    same_route = LMMEngineOpenAI(base_url=engine.base_url, model='fake', api_key='test')
    same_calls = client(same_route, ['ok'])
    same_route.generate(messages)
    assert same_calls[0]['messages'][0]['role'] == 'user'
    for url, model in [(engine.base_url, 'other'), ('http://other-route/v1', 'fake')]:
        other = LMMEngineOpenAI(base_url=url, model=model, api_key='test')
        other_calls = client(other, ['ok'])
        other.generate(messages)
        assert other_calls[0]['messages'][0]['role'] == 'system'


def test_system_fallback_only_once(monkeypatch):
    engine = LMMEngineOpenAI(model='fake', api_key='test')
    calls = client(engine, [HTTPError(400, 'System role unsupported')] * 3)
    monkeypatch.setattr(requests.time, 'sleep', lambda _: pytest.fail('Backoff on 400'))
    with pytest.raises(ProviderError, match='System role unsupported'):
        engine.generate([{'role': 'system', 'content': 'instructions'}, {'role': 'user', 'content': 'task'}])
    assert len(calls) == 2


@pytest.mark.parametrize('engine_class,constructor', [(LMMEngineOpenAI, 'OpenAI'), (LMMEngineAnthropic, 'Anthropic')])
def test_sdk_retries_disabled_and_timeout_passed(monkeypatch, engine_class, constructor):
    from taucode_cua.vendor.agent_s3.core import engine as module
    seen = {}
    def make(**kwargs):
        seen.update(kwargs)
        def create(**kwargs):
            seen['timeout'] = kwargs['timeout']
            raise HTTPError(401, 'Unauthorized')
        return SimpleNamespace(chat=SimpleNamespace(completions=SimpleNamespace(create=create)), messages=SimpleNamespace(create=create))
    monkeypatch.setattr(module, constructor, make)
    engine = engine_class(model='fake', api_key='test', base_url='http://fake-route/v1')
    with pytest.raises(ProviderError, match='Unauthorized'):
        engine.generate([{'role': 'system', 'content': [{'type': 'text', 'text': 'system'}]}])
    assert seen['max_retries'] == 0 and 0 < seen['timeout'] <= 60


def test_redaction_and_error_categories(monkeypatch):
    monkeypatch.setenv('TAUCODE_CUA_PLANNER_KEY', 'short-secret')
    message = 'Bad model Bearer abc.def sk-short-secret ' + 'a' * 64 + ' ' + 'Ab3+/' * 15 + ' short-secret ' + 'word ' * 100
    safe = redact(message)
    assert len(safe) <= 300
    for secret in ('abc.def', 'sk-short-secret', 'a' * 64, 'Ab3+/' * 15, 'short-secret'):
        assert secret not in safe
    for error, method, prefix in [(ValueError('Invalid model configuration (credentials must use environment)'), 'configure', 'Configuration error:'), (ValueError('Expected PNG matching screen size'), 'predict', 'Input error:'), (ProviderError('Provider error: Bad model'), 'predict', 'Provider error:')]:
        assert server.rpc_error(error, method)['message'].startswith(prefix)
        assert str(error) in server.rpc_error(error, method)['message']


def test_stdio_keeps_config_and_input_messages(monkeypatch):
    requests_in = [
        {'jsonrpc': '2.0', 'id': 1, 'method': 'configure', 'params': {'platform': 'darwin', 'planner': {'api_key': 'secret'}}},
        {'jsonrpc': '2.0', 'id': 2, 'method': 'configure', 'params': {'platform': 'darwin', 'planner': {'kind': 'openai', 'model': 'fake'}, 'grounding': {'kind': 'openai', 'model': 'fake'}}},
        {'jsonrpc': '2.0', 'id': 3, 'method': 'predict', 'params': {'instruction': 'test', 'screen_size': [1, 1], 'screenshot_png_b64': 'invalid'}},
    ]
    output = io.StringIO()
    monkeypatch.setattr(server.sys, 'stdin', SimpleNamespace(buffer=io.BytesIO(('\n'.join(map(json.dumps, requests_in)) + '\n').encode())))
    monkeypatch.setattr(server.sys, 'stdout', output)
    server.main()
    responses = list(map(json.loads, output.getvalue().splitlines()))
    assert 'Invalid model configuration (credentials must use environment)' in responses[0]['error']['message']
    assert responses[1]['result'] == {'ok': True}
    assert responses[2]['error']['message'].startswith('Input error:')
