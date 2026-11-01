"""One retry budget shared by every provider call in a prediction."""
import copy
import logging
import os
import re
import signal
import time
from contextlib import contextmanager
from contextvars import ContextVar
from functools import wraps

from anthropic import APIConnectionError as AnthropicConnectionError
from openai import APIConnectionError


class ProviderError(Exception):
    pass


class PredictionTimeout(ProviderError):
    pass


def redact(message, *, raw=False):
    text = str(message)
    for name in ('TAUCODE_CUA_PLANNER_KEY', 'TAUCODE_CUA_GROUNDING_KEY'):
        key = os.environ.get(name)
        if key:
            text = text.replace(key, '[redacted]')
    text = re.sub(r'(?i)\bBearer\s+[^\s\'"<>]+|\bsk-[\w-]+|[A-Za-z0-9_+/=-]{32,}', '[redacted]', text)
    return text[:200] if raw else ' '.join(text.split())[:300]


def provider_message(error):
    body = getattr(error, 'body', None)
    if isinstance(body, dict):
        body = body.get('error', body)
        if isinstance(body, dict) and isinstance(body.get('message'), str):
            return body['message']
    return str(error)


# Some backends expose hosted tools (image generation) the planner cannot use. A model that
# calls one gets this backend error inside its reply and then plans around it. Nothing has
# been sent to the app during prediction, so the request is safe to repeat.
HOSTED_TOOL_FAILURE = re.compile(r'Prompt is required for imagegen tool|Error parsing function call')
HOSTED_TOOL_RETRIES = 2
HOSTED_TOOL_NOTE = ('Your previous reply tried to call a tool. You have no tools, including image generation. '
                    'Reply with text only and end with exactly one agent action.')


def with_note(messages, note):
    """Copy of messages with a note appended to the last user turn (keeps role alternation)."""
    messages = copy.deepcopy(messages)
    last = next((m for m in reversed(messages) if m['role'] == 'user'), None)
    if last is None:
        return messages + [{'role': 'user', 'content': note}]
    if isinstance(last['content'], str):
        last['content'] += '\n\n' + note
    else:
        last['content'].append({'type': 'text', 'text': note})
    return messages

_deadline = ContextVar('predict_deadline', default=None)  # (monotonic deadline, limit in seconds)
_no_system = set()


def limit_reached(seconds):
    return PredictionTimeout(f'Provider request exceeded the {seconds:g} second prediction limit')


def remaining():
    deadline, limit = _deadline.get()
    seconds = deadline - time.monotonic()
    if seconds <= 0:
        raise limit_reached(limit)
    return seconds


@contextmanager
def predict_budget(seconds=60):
    if _deadline.get() is not None:
        yield
        return
    token = _deadline.set((time.monotonic() + seconds, seconds))
    # The macOS stdio sidecar is single-threaded. This also bounds slow/streaming
    # responses whose per-read SDK timeout alone cannot enforce a total deadline.
    def expired(*_):
        raise limit_reached(seconds)
    previous = signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)
        _deadline.reset(token)


def fold_system(messages):
    messages = copy.deepcopy(messages)
    system = []
    kept = []
    for message in messages:
        if message['role'] == 'system':
            content = message['content']
            system.extend([{'type': 'text', 'text': content}] if isinstance(content, str) else content)
        else:
            kept.append(message)
    if system:
        user = next((m for m in kept if m['role'] == 'user'), None)
        if user is None:
            kept.insert(0, {'role': 'user', 'content': system})
        else:
            content = user['content']
            user['content'] = system + ([{'type': 'text', 'text': content}] if isinstance(content, str) else content)
    return kept


def provider_request(generate):
    @wraps(generate)
    def request(self, messages, *args, **kwargs):
        with predict_budget():
            last_error = None
            try:
                route = (getattr(self, 'base_url', None), getattr(self, 'model', 'tgi'))
                delay = 1
                hosted_tool_retries = 0
                while True:
                    kwargs['timeout'] = remaining()
                    sent = fold_system(messages) if getattr(self, 'system_fallback', False) and route in _no_system else messages
                    if hosted_tool_retries:
                        sent = with_note(sent, HOSTED_TOOL_NOTE)
                    try:
                        result = generate(self, sent, *args, **kwargs)
                        if not (isinstance(result, str) and HOSTED_TOOL_FAILURE.search(result)):
                            return result
                        if hosted_tool_retries >= HOSTED_TOOL_RETRIES:
                            raise ProviderError('Provider error: the planner model kept calling a hosted tool it cannot use (image generation); no input was sent')
                        hosted_tool_retries += 1
                        last_error = 'planner model called an unavailable hosted tool'
                        logging.warning('Planner reply contained a hosted-tool error; requesting again (%d/%d)', hosted_tool_retries, HOSTED_TOOL_RETRIES)
                        continue
                    except ProviderError:
                        raise
                    except Exception as error:
                        status = getattr(error, 'status_code', None)
                        message = provider_message(error)
                        # Only OpenAI-compatible transports get this compatibility retry.
                        if (status == 400 and getattr(self, 'system_fallback', False)
                                and route not in _no_system
                                and any(m['role'] == 'system' for m in messages)
                                and re.search(r'system.{0,40}(?:message|role)|(?:message|role).{0,40}system', message, re.I)):
                            _no_system.add(route)
                            continue
                        retryable = (status in (408, 409, 429) or isinstance(status, int) and 500 <= status <= 599
                                     or status is None and isinstance(error, (APIConnectionError, AnthropicConnectionError, ConnectionError, TimeoutError)))
                        if not retryable:
                            raise ProviderError('Provider error: ' + redact(message)) from None
                        last_error = redact(message)
                        time.sleep(min(delay, remaining()))
                        delay = min(delay * 2, 8)
            except PredictionTimeout as error:
                if last_error:
                    raise ProviderError(f'{error}: {last_error}') from None
                raise
    return request
