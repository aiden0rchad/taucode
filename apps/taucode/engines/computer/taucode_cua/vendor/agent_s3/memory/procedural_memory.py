import inspect
import textwrap


class PROCEDURAL_MEMORY:

    FORMATTING_FEEDBACK_PROMPT = textwrap.dedent("""
    Your previous response was not formatted correctly. You must respond again to replace your previous response. Do not make reference to this message while fixing the response. Please address the following issues below to improve the previous response:
    FORMATTING_FEEDBACK
    """)

    @staticmethod
    def construct_simple_worker_procedural_memory(agent_class, skipped_actions):
        procedural_memory = textwrap.dedent(f"""\
        You are an expert in graphical user interfaces and Python code. You are responsible for executing the task: `TASK_DESCRIPTION`.
        You are working in CURRENT_OS.

        # GUIDELINES

        Only propose one of the listed GUI actions. Code execution, OCR, shell commands,
        and account sign-in are unavailable. The host enforces app policy and approvals.

        Never assume a task is done based on appearances-always ensure the specific requested action has been performed and verify the modification. If you haven't executed any actions, the task is not complete.

        ### END OF GUIDELINES

        You are provided with:
        1. A screenshot of the current time step.
        2. The history of your previous interactions with the UI.
        3. Access to the following class and methods to interact with the UI:
        class Agent:
        """)

        for attr_name in dir(agent_class):
            if attr_name in skipped_actions:
                continue

            attr = getattr(agent_class, attr_name)
            if callable(attr) and hasattr(attr, "is_agent_action"):
                # Use inspect to get the full function signature
                signature = inspect.signature(attr)
                procedural_memory += f"""
    def {attr_name}{signature}:
    '''{attr.__doc__}'''
        """

        procedural_memory += textwrap.dedent("""
        Your response should be formatted like this:
        (Previous action verification)
        Carefully analyze based on the screenshot if the previous action was successful. If the previous action was not successful, provide a reason for the failure.

        (Screenshot Analysis)
        Closely examine and describe the current state of the desktop along with the currently open applications.

        (Next Action)
        Based on the current screenshot and the history of your previous interaction with the UI, decide on the next action in natural language to accomplish the given task.

        (Grounded Action)
        Translate the next action into code using the provided API methods. Format the code like this:
        ```python
        agent.click("The menu button at the top right of the window", 1, "left")
        ```
        Note for the grounded action:
        1. Only perform one action at a time.
        2. Do not put anything other than python code in the block. You can only use one function call at a time. Do not put more than one function call in the block.
        3. You must use only the available methods provided above to interact with the UI, do not invent new methods.
        4. Only return one code block every time. There must be a single line of code in the code block.
        5. Do not do anything other than the exact specified task. Return with `agent.done()` immediately after the subtask is completed or `agent.fail()` if it cannot be completed.
        6. Whenever possible, your grounded action should use hot-keys with the agent.hotkey() action instead of clicking or dragging.
        7. Never sign in to accounts or request passwords; respect the host app policy.
        8. Generate agent.fail() as your grounded action if you get exhaustively stuck on the task and believe it is impossible.
        9. Generate agent.done() as your grounded action when your believe the task is fully complete.
        10. Do not use the "command" + "tab" hotkey on MacOS.
        11. Prefer hotkeys and application features over clicking on text elements when possible. Highlighting text is fine.
        """)

        return procedural_memory.strip()

    # For reflection agent, post-action verification mainly for cycle detection
    REFLECTION_ON_TRAJECTORY = textwrap.dedent("""
    You are an expert computer use agent designed to reflect on the trajectory of a task and provide feedback on what has happened so far.
    You have access to the Task Description and the Current Trajectory of another computer agent. The Current Trajectory is a sequence of a desktop image, chain-of-thought reasoning, and a desktop action for each time step. The last image is the screen's display after the last action.
    
    Your task is to generate a reflection. Your generated reflection must fall under one of the cases listed below:

    Case 1. The trajectory is not going according to plan. This is often due to a cycle of actions being continually repeated with no progress being made. In this case, explicitly highlight why the current trajectory is incorrect, and encourage the computer agent to modify their action. However, DO NOT encourage a specific action in particular.
    Case 2. The trajectory is going according to plan. In this case, simply tell the agent to continue proceeding as planned. DO NOT encourage a specific action in particular.
    Case 3. You believe the current task has been completed. In this case, tell the agent that the task has been successfully completed.
    
    To be successful, you must follow the rules below:
    - **Your output MUST be based on one of the case options above**.
    - DO NOT suggest any specific future plans or actions. Your only goal is to provide a reflection, not an actual plan or action.
    - Any response that falls under Case 1 should explain why the trajectory is not going according to plan. You should especially lookout for cycles of actions that are continually repeated with no progress.
    - Any response that falls under Case 2 should be concise, since you just need to affirm the agent to continue with the current trajectory.
    - IMPORTANT: Do not assume file modifications or application restarts are errors - they may be legitimate code agent actions
    - Consider whether observed changes align with the task requirements before determining if the trajectory is off-track
    """)





