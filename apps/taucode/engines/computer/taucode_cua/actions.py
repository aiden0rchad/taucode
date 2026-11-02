"""Literal-only Agent-S calls and a grounding-only ACI. No input injection."""
import ast
import inspect
import json
import math
import sys

from .requests import ProviderError, redact
from .elements import usable_elements, grounding_prompt, parse_element
from .images import downscale
from .vendor.agent_s3.utils.common_utils import call_llm_safe

from .vendor.agent_s3.agents.grounding import OSWorldACI, agent_action

ALLOWED = frozenset(('click', 'click_sequence', 'search_items', 'type', 'scroll', 'drag_and_drop', 'hotkey',
                     'hold_and_press', 'open', 'switch_applications', 'wait', 'done', 'fail'))
# The host sends these when the last proposal was not cleanly executed.
RECOVERY_PREFIXES = ('Previous action was NOT executed:', 'Previous action was interrupted after input started:', 'READ-ONLY VERIFICATION:')
MAX_SEQUENCE = 12


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
        action['target'] = check_text(first) if isinstance(first, str) else action.get('target', '')
        json.dumps(action, allow_nan=False)
        return action
    except ProviderError:
        raise
    except (ValueError, TypeError) as error:
        return {'kind': 'fail', 'reason': f'Action not executed: {error}'}
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
    def assign_screenshot(self, obs):
        super().assign_screenshot(obs)
        self.grounding_decisions = []

    def clear_sequence(self):
        self.queued_clicks, self.sequence_clicked, self.sequence_note = [], [], ''

    def continue_sequence(self, instruction, obs):
        """Next queued click_sequence step, grounded by exact label on the fresh snapshot without a model call.

        Returns None, and leaves a note for the planner, when there is nothing queued, the host reports
        the last step did not run cleanly, or the next label no longer matches exactly one element."""
        queued, clicked = getattr(self, 'queued_clicks', []), getattr(self, 'sequence_clicked', [])
        if not queued:
            return None
        self.queued_clicks = []
        stop = 'the previous step did not run cleanly' if instruction.startswith(RECOVERY_PREFIXES) else None
        if not stop:
            self.assign_screenshot(obs)
            decision = self.decision()
            snapshot = obs.get('ax_snapshot')
            target = self.exact_element(queued[0], usable_elements(snapshot), snapshot, decision) if snapshot and decision['configured_mode'] == 'elements' else None
            if target:
                decision['model_result'] = 'sequence'
                self.record_grounding(decision)
                self.queued_clicks, self.sequence_clicked = queued[1:], clicked + [queued[0]]
                position = len(self.sequence_clicked)
                self.sequence_note = '' if self.queued_clicks else f'Your click_sequence finished: clicked {json.dumps(self.sequence_clicked)} in order. Verify the result.'
                return dict(target, kind='click', clicks=1, button='left', hold_keys=[], target=queued[0],
                            plan=f'Continuing click_sequence ({position} of {position + len(self.queued_clicks)}): {queued[0]}')
            stop = 'it no longer matches exactly one element'
        self.sequence_clicked = []
        self.sequence_note = (f'Your click_sequence stopped: clicked {json.dumps(clicked)}; did not click {json.dumps(queued)} '
                              f'because {stop}. Check the screen before continuing.')
        return None

    def exact_element(self, description, elements, snapshot, decision):
        """The one usable element whose label equals the description, ignoring case and outer spaces."""
        key = description.strip().casefold()
        matches = [e for e in elements if key and e['label'].strip().casefold() == key]
        if len(matches) != 1:
            return None
        _, (width, height) = downscale(self.obs.get('grounding_screenshot', self.obs['screenshot']))
        x, y, w, h = matches[0]['frame']
        if not (0 <= x + w / 2 < width and 0 <= y + h / 2 < height):
            return None
        decision['element_index'] = matches[0]['index']
        return dict(x=x + w / 2, y=y + h / 2, element_index=matches[0]['index'], snapshot_id=snapshot['snapshot_id'])

    def decision(self):
        snapshot = self.obs.get('ax_snapshot') if self.obs else None
        mode = self.engine_params_for_grounding.get('grounding_mode', 'pixels')
        return dict(configured_mode=mode, mode=mode,
                    usable_elements=len(usable_elements(snapshot)) if snapshot else 0,
                    snapshot_id=snapshot['snapshot_id'] if snapshot else None,
                    snapshot_status=snapshot.get('status', 'unknown') if snapshot else 'missing',
                    window_lookup=snapshot.get('window_lookup') if snapshot else None,
                    model_result='not_requested', raw_reply='', element_index=None,
                    pixel_fallback_reason=None)

    def record_grounding(self, decision):
        if not hasattr(self, 'grounding_decisions'):
            self.grounding_decisions = []
        self.grounding_decisions.append(decision)
        print('grounding ' + json.dumps(decision, ensure_ascii=False, separators=(',', ':')), file=sys.stderr, flush=True)

    def point(self, description, decision=None):
        decision = self.decision() if decision is None else decision
        decision['mode'] = 'ui-tars' if decision['configured_mode'] == 'ui-tars' else 'pixels'
        if decision['configured_mode'] == 'elements' and not decision['pixel_fallback_reason']:
            decision['pixel_fallback_reason'] = 'coordinate_only_action'
        self.grounding_reply = ''
        try:
            point = self.generate_coords(check_text(description), self.obs)
            if len(point) != 2 or any(type(n) not in (int, float) or not math.isfinite(n) for n in point):
                raise ValueError('Invalid grounding coordinates')
            w, h = (self.engine_params_for_grounding[k] for k in ('grounding_width', 'grounding_height'))
            if not (0 <= point[0] < w and 0 <= point[1] < h):
                raise ValueError('Grounding coordinates outside screen')
            decision['pixel_result'] = 'coordinates'
            return point
        except Exception as error:
            decision['pixel_result'] = 'provider_error' if isinstance(error, ProviderError) else 'unparseable'
            decision['error'] = redact(error)
            raise
        finally:
            decision['pixel_raw_reply'] = redact(self.grounding_reply, raw=True)
            self.record_grounding(decision)

    def ground(self, description):
        description = check_text(description)
        decision = self.decision()
        snapshot = self.obs.get('ax_snapshot') if self.obs else None
        if decision['configured_mode'] == 'elements':
            elements = usable_elements(snapshot) if snapshot else []
            if elements:
                try:
                    # An exact, unique label needs no model call.
                    target = self.exact_element(description, elements, snapshot, decision)
                    if target:
                        decision['model_result'] = 'exact_label'
                        self.record_grounding(decision)
                        return target
                    image, (width, height) = downscale(self.obs.get('grounding_screenshot', self.obs['screenshot']))
                    self.engine_params_for_grounding.update(grounding_width=width, grounding_height=height)
                    self.grounding_model.reset()
                    self.grounding_model.add_message(text_content=grounding_prompt(description, elements), image_content=image, put_text_last=True)
                    reply = call_llm_safe(self.grounding_model)
                    decision['raw_reply'] = redact(reply, raw=True)
                    decision['model_result'] = 'unparseable'
                    index = parse_element(reply, elements)
                    decision['model_result'] = 'none' if index is None else 'index'
                    if index is not None:
                        e = next(e for e in elements if e['index'] == index)
                        x, y, w, h = e['frame']
                        if not (0 <= x + w / 2 < width and 0 <= y + h / 2 < height):
                            raise ValueError('Element center outside screenshot')
                        decision['element_index'] = index
                        self.record_grounding(decision)
                        return dict(x=x + w / 2, y=y + h / 2, element_index=index, snapshot_id=snapshot['snapshot_id'])
                    decision['pixel_fallback_reason'] = 'model_returned_none'
                except Exception as error:
                    if isinstance(error, ProviderError):
                        decision['model_result'] = 'provider_error'
                        decision['error'] = redact(error)
                        self.record_grounding(decision)
                        raise
                    decision['model_result'] = 'unparseable'
                    decision['error'] = redact(error)
                    decision['pixel_fallback_reason'] = 'unparseable_element_reply'
            else:
                decision['pixel_fallback_reason'] = 'no_usable_elements' if snapshot else 'missing_snapshot'
        x, y = self.point(description, decision)
        return dict(x=x, y=y)

    @agent_action
    def click(self, element_description: str, num_clicks: int = 1, button_type: str = 'left', hold_keys: list = ()):
        """Click a described element; button left/right/middle, 1–3 clicks."""
        if type(num_clicks) is not int or not 1 <= num_clicks <= 3 or button_type not in ('left', 'right', 'middle'):
            raise ValueError('Invalid click')
        return dict(self.ground(element_description), kind='click', clicks=num_clicks, button=button_type, hold_keys=check_keys(hold_keys))

    @agent_action
    def click_sequence(self, element_labels: list):
        """Left-click 2-12 elements in order on a screen that stays the same between clicks (keypads, fixed buttons); each label must be an exact element label from the accessibility list."""
        if not isinstance(element_labels, (list, tuple)) or not 2 <= len(element_labels) <= MAX_SEQUENCE:
            raise ValueError(f'click_sequence needs 2 to {MAX_SEQUENCE} element labels')
        labels = [check_text(label) for label in element_labels]
        action = dict(self.ground(labels[0]), kind='click', clicks=1, button='left', hold_keys=[], target=labels[0])
        self.queued_clicks, self.sequence_clicked, self.sequence_note = labels[1:], labels[:1], ''
        return action

    @agent_action
    def search_items(self, query: str):
        """Notes and Reminders only: count notes or reminders whose title or text contains the query and list up to 20 titles, without using the app's window; the result arrives with the next screenshot. Use it instead of the app's search field."""
        query = check_text(query)
        if not query.strip() or len(query) > 500:
            raise ValueError('search_items needs a query of 1 to 500 characters')
        return dict(kind='search_items', query=query)

    @agent_action
    def type(self, element_description: str | None = None, text: str = '', overwrite: bool = False, enter: bool = False):
        """Type Unicode text, optionally in a described field; overwrite/enter are booleans."""
        action = dict(kind='type', text=check_text(text), overwrite=boolean(overwrite), enter=boolean(enter))
        if element_description is not None:
            action.update(self.ground(element_description))
        return action

    @agent_action
    def scroll(self, element_description: str, clicks: int, shift: bool = False):
        """Scroll by integer clicks from -100 to 100; positive up, negative down; shift for horizontal."""
        if type(clicks) is not int or abs(clicks) > 100:
            raise ValueError('Scroll clicks must be an integer from -100 to 100; use smaller steps')
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
