from pathlib import Path
def read_inputs(paths, stdin):
    return [line for path in paths for line in Path(path).read_text(encoding='utf-8').splitlines()]
