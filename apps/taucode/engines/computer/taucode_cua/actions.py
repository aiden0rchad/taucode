"""Literal-only Agent-S calls and a grounding-only ACI. No input injection."""
import ast
import inspect
import json
import math

from .requests import ProviderError
from .elements import usable_elements, grounding_prompt, parse_element
from .images import downscale
from .vendor.agent_s3.utils.common_utils import call_llm_safe

from .vendor.agent_s3.agents.grounding import OSWorldACI, agent_action

ALLOWED = frozenset(('click', 'type', 'scroll', 'drag_and_drop', 'hotkey',
                     'hold_and_press', 'open', 'switch_applications', 'wait', 'done', 'fail'))


def parse_call(code):
    try:
        if not isinstance(code, str) or len(code) > 65536:
            raise ValueError('Action exceeds size limit')
        tree = ast.parse(code.strip(), mode='eval').body
        if not (isinstance(tree, ast.Call) and isinstance(tree.func, ast.Attribute)
                and isinstance(tree.func.value, ast.Name) and tree.func.value.id == 'agent'
                and tree.func.attr in ALLOWED):
            raise ValueError('Expected exactly one allowlisted agent.method(...) call')
        args = [ast.literal_eval(arg) for arg in tree.args]
        kwargs = {}
        for kw in tree.keywords:
            if kw.arg is None or kw.arg in kwargs:
                raise ValueError('Expanded or duplicate arguments are forbidden')
            kwargs[kw.arg] = ast.literal_eval(kw.value)
        inspect.signature(getattr(StructuredACI, tree.func.attr)).bind(None, *args, **kwargs)
        json.dumps([args, kwargs], allow_nan=False)
        return tree.func.attr, args, kwargs
    except (SyntaxError, TypeError, ValueError, RecursionError, OverflowError) as error:
        raise ValueError(f'Invalid action: {error}') from None


def parse_action(agent, code):
    try:
        method, args, kwargs = parse_call(code)
        action = getattr(StructuredACI, method)(agent, *args, **kwargs)
        bound = inspect.signature(getattr(StructuredACI, method)).bind(agent, *args, **kwargs)
        bound.apply_defaults()
        first = list(bound.arguments.values())[1]
        action['target'] = check_text(first) if isinstance(first, str) else ''
        json.dumps(action, allow_nan=False)
        return action
    except ProviderError:
        raise
    except Exception as error:
        return {'kind': 'fail', 'reason': str(error)}


def check_text(value):
    if not isinstance(value, str) or len(value) > 32768 or '\0' in value:
        raise ValueError('Expected bounded text without NUL')
    return value


def check_keys(value):
    if not isinstance(value, (list, tuple)) or len(value) > 16:
        raise ValueError('Expected up to 16 keys')
    return [check_text(k) for k in value]


def boolean(value):
    if type(value) is not bool:
        raise ValueError('Expected boolean')
    return value


class StructuredACI(OSWorldACI):
    def point(self, description):
        point = self.generate_coords(check_text(description), self.obs)
        if len(point) != 2 or any(type(n) not in (int, float) or not math.isfinite(n) for n in point):
            raise ValueError('Invalid grounding coordinates')
        w, h = (self.engine_params_for_grounding[k] for k in ('grounding_width', 'grounding_height'))
        if not (0 <= point[0] < w and 0 <= point[1] < h):
            raise ValueError('Grounding coordinates outside screen')
        return point

    def ground(self, description):
        description = check_text(description)
        snapshot = self.obs.get('ax_snapshot') if self.obs else None
        if self.engine_params_for_grounding.get('grounding_mode') == 'elements' and snapshot:
            elements = usable_elements(snapshot)
            if elements:
                image, (width, height) = downscale(self.obs.get('grounding_screenshot', self.obs['screenshot']))
                self.engine_params_for_grounding.update(grounding_width=width, grounding_height=height)
                self.grounding_model.reset()
                self.grounding_model.add_message(text_content=grounding_prompt(description, elements), image_content=image, put_text_last=True)
                index = parse_element(call_llm_safe(self.grounding_model), elements)
                if index is not None:
                    e = next(e for e in elements if e['index'] == index)
                    x, y, w, h = e['frame']
                    if not (0 <= x + w / 2 < width and 0 <= y + h / 2 < height):
                        raise ValueError('Element center outside screenshot')
                    return dict(x=x + w / 2, y=y + h / 2, element_index=index, snapshot_id=snapshot['snapshot_id'])
        x, y = self.point(description)
        return dict(x=x, y=y)

    @agent_action
    def click(self, element_description: str, num_clicks: int = 1, button_type: str = 'left', hold_keys: list = ()):
        """Click a described element; button left/right/middle, 1–3 clicks."""
        if type(num_clicks) is not int or not 1 <= num_clicks <= 3 or button_type not in ('left', 'right', 'middle'):
            raise ValueError('Invalid click')
        return dict(self.ground(element_description), kind='click', clicks=num_clicks, button=button_type, hold_keys=check_keys(hold_keys))

    @agent_action
    def type(self, element_description: str | None = None, text: str = '', overwrite: bool = False, enter: bool = False):
        """Type Unicode text, optionally in a described field; overwrite/enter are booleans."""
        action = dict(kind='type', text=check_text(text), overwrite=boolean(overwrite), enter=boolean(enter))
        if element_description is not None:
            action.update(self.ground(element_description))
        return action

    @agent_action
    def scroll(self, element_description: str, clicks: int, shift: bool = False):
        """Scroll at a described element; positive up, negative down; shift for horizontal."""
        if type(clicks) is not int or abs(clicks) > 100:
            raise ValueError('Invalid scroll distance')
        return dict(self.ground(element_description), kind='scroll', clicks=clicks, shift=boolean(shift))

    @agent_action
    def drag_and_drop(self, starting_description: str, ending_description: str, hold_keys: list = ()):
        """Drag between two described elements, optionally holding modifier keys."""
        return {'kind': 'drag', 'from': self.point(starting_description), 'to': self.point(ending_description), 'hold_keys': check_keys(hold_keys)}

    @agent_action
    def hotkey(self, keys: list):
        """Press a combination of named keys, e.g. ['command', 'c']."""
        return dict(kind='hotkey', keys=check_keys(keys))

    @agent_action
    def hold_and_press(self, hold_keys: list, press_keys: list):
        """Hold modifier keys and press a sequence of named keys."""
        return dict(kind='hold_and_press', hold=check_keys(hold_keys), press=check_keys(press_keys))

    @agent_action
    def open(self, app_or_filename: str):
        """Request opening an application by bundle ID or a file; host policy applies."""
        return dict(kind='open', app_or_file=check_text(app_or_filename))

    @agent_action
    def switch_applications(self, app_code: str):
        """Switch to an allowed application by bundle ID."""
        return dict(kind='switch_app', app=check_text(app_code))

    @agent_action
    def wait(self, time: float):
        """Wait up to 10 seconds."""
        if type(time) not in (int, float) or not math.isfinite(time) or not 0 <= time <= 10:
            raise ValueError('Invalid wait duration')
        return dict(kind='wait', seconds=time)

    @agent_action
    def done(self, summary: str = 'Task completed'):
        """Finish the task with a summary."""
        return dict(kind='done', summary=check_text(summary))

    @agent_action
    def fail(self, reason: str = 'Planner could not complete task'):
        """Stop with a failure reason."""
        return dict(kind='fail', reason=check_text(reason))
