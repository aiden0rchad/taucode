"""Agent-S3 grounding only. Modified by taucode; see NOTICE."""
import re
from taucode_cua.images import downscale
from typing import Dict, List
from taucode_cua.vendor.agent_s3.core.mllm import LMMAgent
from taucode_cua.vendor.agent_s3.utils.common_utils import call_llm_safe


class ACI:
    def __init__(self):
        self.notes: List[str] = []


def agent_action(func):
    func.is_agent_action = True
    return func


class OSWorldACI(ACI):
    def __init__(self, env, platform, engine_params_for_generation, engine_params_for_grounding, width=1920, height=1080):
        super().__init__()
        self.env = None
        self.platform = platform
        self.width, self.height = width, height
        self.notes = []
        self.obs = None
        self.grounding_model = LMMAgent(engine_params_for_grounding)
        self.engine_params_for_grounding = engine_params_for_grounding


    def generate_coords(self, ref_expr: str, obs: Dict) -> List[int]:

        # Reset the grounding model state
        self.grounding_model.reset()

        image = obs.get("grounding_screenshot", obs["screenshot"])
        if self.engine_params_for_grounding.get("grounding_mode", "pixels") == "pixels":
            image, (width, height) = downscale(image)
            self.engine_params_for_grounding.update(grounding_width=width, grounding_height=height)
            prompt = f"This is a {width}x{height} screenshot. Give the pixel coordinates of the center of {ref_expr}. Answer ONLY as (x, y) in pixels."
        else:
            prompt = f"Query:{ref_expr}\nOutput only the coordinate of one point in your response.\n"
        self.grounding_model.add_message(text_content=prompt, image_content=image, put_text_last=True)
        return parse_coordinates(call_llm_safe(self.grounding_model))


    def assign_screenshot(self, obs: Dict):
        self.obs = obs


    def set_task_instruction(self, task_instruction: str):
        """Set the current task instruction for the code agent."""
        self.current_task_instruction = task_instruction


def parse_coordinates(response):
    number = r"(-?\d+(?:\.\d+)?)"
    pairs = re.findall(r"\(\s*" + number + r"\s*,\s*" + number + r"\s*\)", response)
    if not pairs:
        pairs = re.findall(r"\bx\s*=\s*" + number + r"\s*,?\s*y\s*=\s*" + number, response, re.I)
    if not pairs:
        numbers = re.findall(number, response)
        pairs = [numbers] if len(numbers) == 2 else []
    if len(pairs) != 1:
        raise ValueError("Expected one coordinate pair from grounding model")
    return [float(n) for n in pairs[0]]
