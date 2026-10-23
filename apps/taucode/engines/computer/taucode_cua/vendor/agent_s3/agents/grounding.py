"""Agent-S3 grounding only. Modified by taucode; see NOTICE."""
import re
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

        # Configure the context, UI-TARS demo does not use system prompt
        prompt = f"Query:{ref_expr}\nOutput only the coordinate of one point in your response.\n"
        self.grounding_model.add_message(
            text_content=prompt, image_content=obs["screenshot"], put_text_last=True
        )

        # Generate and parse coordinates
        response = call_llm_safe(self.grounding_model)
        print("RAW GROUNDING MODEL RESPONSE:", response)
        numericals = re.findall(r"-?\d+(?:\.\d+)?", response)
        assert len(numericals) == 2, "Expected exactly two coordinates"
        return [float(numericals[0]), float(numericals[1])]


    def assign_screenshot(self, obs: Dict):
        self.obs = obs


    def set_task_instruction(self, task_instruction: str):
        """Set the current task instruction for the code agent."""
        self.current_task_instruction = task_instruction
