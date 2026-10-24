import base64
import io
from types import SimpleNamespace

import pytest
from PIL import Image
from taucode_cua.actions import StructuredACI
from taucode_cua.images import downscale
from taucode_cua.server import Server, engine_params
from taucode_cua.vendor.agent_s3.agents.grounding import parse_coordinates


def png(size):
    output = io.BytesIO()
    Image.new('RGB', size).save(output, format='PNG')
    return output.getvalue()


@pytest.mark.parametrize('response', ['(123, 45)', 'x=123, y=45', 'x = 123 y = 45', 'On this 1920x1080 image, the center is (123, 45).', 'The answer is x=123, y=45 pixels.', '123 45'])
def test_coordinate_parsing(response):
    assert parse_coordinates(response) == [123, 45]


@pytest.mark.parametrize('response', ['No coordinates', '(1, 2) or (3, 4)', 'nan, inf', '(1, 2, 3)'])
def test_invalid_coordinates(response):
    with pytest.raises(ValueError):
        parse_coordinates(response)


@pytest.mark.parametrize('size,expected', [((3840, 2160), (1920, 1080)), ((1800, 2880), (1200, 1920)), ((800, 600), (800, 600)), ((3024, 1964), (1920, 1247))])
def test_downscale(size, expected):
    sent, actual = downscale(png(size))
    assert actual == expected
    assert Image.open(io.BytesIO(sent)).size == expected


@pytest.mark.parametrize('mode', ['elements', 'pixels', 'ui-tars'])
def test_grounding_prompt_image_and_space(mode):
    config = engine_params({'kind': 'openai', 'model': 'fake', 'mode': mode, 'width': 1000, 'height': 1000}, 'grounding')
    aci = StructuredACI(None, 'darwin', {}, config)
    seen = {}
    aci.grounding_model = SimpleNamespace(reset=lambda: None, add_message=lambda **kwargs: seen.update(kwargs), get_response=lambda **_: 'The center is (900, 450).')
    original = png((3200, 2000))
    aci.assign_screenshot({'screenshot': downscale(original)[0], 'grounding_screenshot': original})
    assert aci.click('Search field')['x'] == 900
    if mode != 'ui-tars':
        assert seen['text_content'] == 'This is a 1920x1200 screenshot. Give the pixel coordinates of the center of Search field. Answer ONLY as (x, y) in pixels.'
        assert Image.open(io.BytesIO(seen['image_content'])).size == (1920, 1200)
        assert (config['grounding_width'], config['grounding_height']) == (1920, 1200)
    else:
        assert seen['text_content'] == 'Query:Search field\nOutput only the coordinate of one point in your response.\n'
        assert seen['image_content'] == original
        assert (config['grounding_width'], config['grounding_height']) == (1000, 1000)


@pytest.mark.parametrize('mode', ['elements', 'pixels', 'ui-tars'])
def test_server_resizes_planner_and_returns_correct_coordinate_space(mode):
    server = Server()
    config = {'kind': 'openai', 'model': 'fake', 'mode': mode, 'width': 1000, 'height': 1000}
    server.dispatch('configure', {'platform': 'darwin', 'planner': config, 'grounding': config})
    original = png((3200, 2000))
    def predict(instruction, obs):
        assert Image.open(io.BytesIO(obs['screenshot'])).size == (1920, 1200)
        assert obs['grounding_screenshot'] == original
        return {}, [{'kind': 'done', 'summary': 'ok'}]
    server.agent.predict = predict
    result = server.dispatch('predict', {'instruction': 'test', 'screen_size': [3200, 2000], 'screenshot_png_b64': base64.b64encode(original).decode()})
    action = result['action']
    assert (action['grounding_width'], action['grounding_height']) == ((1920, 1200) if mode != 'ui-tars' else (1000, 1000))


def test_mode_default_and_validation():
    config = {'kind': 'openai', 'model': 'fake'}
    assert engine_params(config, 'grounding')['grounding_mode'] == 'elements'
    with pytest.raises(ValueError, match='Invalid grounding mode'):
        engine_params({**config, 'mode': 'code'}, 'grounding')
