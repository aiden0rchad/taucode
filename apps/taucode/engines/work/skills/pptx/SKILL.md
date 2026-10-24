---
name: pptx
description: Create and edit PowerPoint presentations with python-pptx from notes or research.
compatibility: Python 3.12 with the taucode Work runtime.
---

Use only the Python interpreter in `TAUCODE_WORK_PYTHON`. If that variable is empty,
the interpreter is missing, or importing pptx fails, tell the user the document
library is unavailable. Offer Markdown/CSV and suggest retrying Work runtime setup
in Settings. Do not install packages, download dependencies, or silently claim success.

Start with an outline and one main point per slide. Use concise titles, consistent typography and margins. Split dense slides. Inspect slide counts and text overflow before delivery.

Plan first. Keep deliverables in `outputs/`. Ask before deleting or overwriting any
pre-existing user file outside `outputs/`, even with full access. Extra folders are
read-only. Bash and connectors retain taucode approvals.

A starter renderer is at `../render.py`, relative to this skill. Write a UTF-8 JSON
input inside the space using this schema:

```json
{"slides":[{"title":"Summary","bullets":["First point"]}]}
```

Run with the skill's absolute script path (replace `<skill-directory>`):
```sh
"$TAUCODE_WORK_PYTHON" "<skill-directory>/../render.py" pptx input.json outputs/result.pptx
```

The helper creates a simple starting document. For polished or complex deliverables,
use pptx directly through the same interpreter. Verify the actual artifact,
summarize what was produced, and link its relative output path.

