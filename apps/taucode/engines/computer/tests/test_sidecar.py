import ast
import base64
import io
import json
from pathlib import Path
import pytest
from PIL import Image
from taucode_cua.actions import StructuredACI, parse_action, parse_call, ALLOWED
from taucode_cua.server import Server, engine_params, KINDS
from taucode_cua.vendor.agent_s3.utils.common_utils import create_pyautogui_code

@pytest.fixture
def aci():
    agent = object.__new__(StructuredACI)
    agent.obs = {}
    agent.engine_params_for_grounding = {'grounding_width': 1920, 'grounding_height': 1080}
    agent.generate_coords = lambda desc, obs: [100, 200]
    return agent

@pytest.mark.parametrize('code', [
    'agent.click.__call__("x")', 'agent.__class__()', 'other.click("x")',
    'agent.click(name)', 'agent.click(str("x"))', 'agent.click("x"); agent.done()',
    'agent.click(*["x"])', 'agent.click(**{"element_description":"x"})',
    '__import__("os").system("id")', 'agent.call_code_agent()', 'agent.set_cell_values({})',
    'agent.highlight_text_span("a", "b")', '[agent.done()]', '(lambda: agent.done())()',
    'agent.wait(float("nan"))', 'agent.wait(1e999)', 'agent.done(summary="x",summary="y")',
])
def test_rejects_nonliteral_calls(aci, code):
    result = parse_action(aci, code)
    assert result['kind'] == 'fail' and result['reason']

@pytest.mark.parametrize('code,kind', [
    ('agent.click("Send button", 2, "right", ["shift"])','click'),
    ('agent.type("Message field", "Unicode: 🐝", True, False)','type'),
    ('agent.type(text="hello")','type'),
    ('agent.scroll("List", -3, True)','scroll'),
    ('agent.drag_and_drop("start", "end", ["ctrl"])','drag'),
    ('agent.hotkey(["command", "c"])','hotkey'),
    ('agent.hold_and_press(["shift"], ["left"])','hold_and_press'),
    ('agent.open("com.apple.TextEdit")','open'),
    ('agent.switch_applications("com.apple.TextEdit")','switch_app'),
    ('agent.wait(0.1)','wait'), ('agent.done("Saved")','done'), ('agent.fail("Missing button")','fail'),
])
def test_every_action_is_json_and_uses_grounding_space(aci, code, kind):
    result = create_pyautogui_code(aci, code, {})
    assert result['kind'] == kind
    assert json.loads(json.dumps(result)) == result
    if 'x' in result: assert result['x'] == 100 and result['y'] == 200

@pytest.mark.parametrize('code', ['agent.wait(-1)', 'agent.wait(11)', 'agent.click("x", True)', 'agent.type(text=3)', 'agent.type(enter=1)', 'agent.scroll("x", 101)', 'agent.hotkey("command")'])
def test_invalid_action_arguments(aci,code):
    assert parse_action(aci,code)['kind']=='fail'

def test_out_of_bounds_grounding(aci):
    aci.generate_coords=lambda *a: [1920, 1]
    assert parse_action(aci,'agent.click("x")')['kind']=='fail'

@pytest.mark.parametrize('code,target', [
    ('agent.click("The = button in Calculator")', 'The = button in Calculator'),
    ('agent.click("Delete account")', 'Delete account'),
    ('agent.click(num_clicks=1, element_description="Send")', 'Send'),
    ('agent.type(text="Send")', ''),
    ('agent.drag_and_drop("Start", "End")', 'Start'),
    ('agent.wait(1)', ''),
])
def test_target_comes_from_literal_call_argument(aci, code, target):
    assert parse_action(aci, code)['target'] == target

@pytest.mark.parametrize('kind,engine', KINDS.items())
def test_registry_mapping(monkeypatch,kind,engine):
    monkeypatch.setenv('TAUCODE_CUA_PLANNER_KEY','secret')
    params=engine_params({'kind':kind,'model':'test','base_url':'http://localhost/v1'},'planner')
    assert params == dict(engine_type=engine,model='test',base_url='http://localhost/v1',api_key='secret')

def test_credentials_cannot_arrive_in_rpc():
    with pytest.raises(ValueError):engine_params({'kind':'openai','model':'test','api_key':'secret'},'planner')

def test_no_execution_or_input_modules():
    root=Path(__file__).parents[1]/'taucode_cua'
    for path in root.rglob('*.py'):
        for node in ast.walk(ast.parse(path.read_text())):
            if isinstance(node,ast.Call) and isinstance(node.func,ast.Name):assert node.func.id not in {'eval','exec','compile','__import__'}
            if isinstance(node,(ast.Import,ast.ImportFrom)):
                names=[a.name for a in node.names] if isinstance(node,ast.Import) else [node.module or '']
                assert not any(n.split('.')[0] in {'pyautogui','subprocess','pytesseract','cv2','paddle','paddleocr'} for n in names)
    lock=(root.parent/'uv.lock').read_text().lower()
    for forbidden in ('paddle','opencv','pyautogui','pytesseract'):assert forbidden not in lock

def test_rpc_prediction_without_screen_or_network(aci):
    server=Server()
    class Fake:
        def predict(self,instruction,observation):
            assert instruction=='test' and observation['screenshot'].startswith(b'\x89PNG')
            return {'plan':'agent.done()', 'reflection':'complete'},[{'kind':'done','summary':'ok'}]
    server.agent=Fake();server.size=[1920,1080]
    buffer=io.BytesIO();Image.new('RGB',(4,4)).save(buffer,format='PNG')
    params={'instruction':'test','screenshot_png_b64':base64.b64encode(buffer.getvalue()).decode(),'screen_size':[4,4]}
    result=server.dispatch('predict',params)
    assert result['action']['grounding_width']==4 and result['reflection']=='complete'
    params['screen_size']=[8,8]
    with pytest.raises(ValueError):server.dispatch('predict',params)

def test_planner_constructs_without_code_agent():
    server=Server()
    config={'kind':'openai','model':'test','base_url':'http://localhost:1234/v1'}
    assert server.dispatch('configure',{'planner':config,'grounding':config,'platform':'darwin'})=={'ok':True}
    prompt=server.agent.executor.generator_agent.system_prompt
    for name in ALLOWED:assert f'def {name}' in prompt
    for name in ('call_code_agent','highlight_text_span','set_cell_values'):assert f'def {name}' not in prompt
    assert server.dispatch('reset',{})=={'ok':True}

def test_rejected_plan_is_returned_as_fail_and_fed_back(monkeypatch):
    from taucode_cua.vendor.agent_s3.agents import worker
    monkeypatch.setattr(worker, 'call_llm_formatted', lambda *a, **k: '```python\nagent.type(text=__import__("os").getcwd())\n```')
    server=Server()
    config={'kind':'openai','model':'test','base_url':'http://localhost:1234/v1'}
    server.dispatch('configure',{'planner':config,'grounding':config,'platform':'darwin'})
    buf=io.BytesIO();Image.new('RGB',(4,4)).save(buf,format='PNG')
    _, actions=server.agent.predict('test',{'screenshot':buf.getvalue()})
    assert actions[0]['kind']=='fail'
    messages=server.agent.executor.generator_agent.messages
    assert 'Action rejected:' in json.dumps(messages[-1])
