# Computer-use sidecar

The host supplies PNG bytes and registry model configuration over stdio JSON-RPC.
The sidecar returns structured proposals; only the Rust host enforces policy,
approvals and input. Credentials arrive through `TAUCODE_CUA_PLANNER_KEY` and
`TAUCODE_CUA_GROUNDING_KEY`, never through RPC.

## macOS permissions

Use **Request access** in Settings → Computer Use to request Screen Recording and
Accessibility, then grant any missing access in System Settings. Screen Recording
takes effect only after quitting and reopening taucode; use **Quit & Reopen**.
Development builds are ad-hoc signed, so macOS may ask again after rebuilds.

`configure.grounding.mode` accepts `pixels` (default) or `ui-tars`. Pixel mode
uses the actual dimensions of an aspect-preserving image capped at 1920 pixels
on its long side. UI-TARS uses `grounding.width`/`height` (1920×1080 or 1000×1000)
and its specialized prompt. Planner images are capped at 1920 in both modes.

Each prediction has one 60-second deadline across reflection, planning,
formatting corrections, grounding and retry backoff. Permanent provider errors
fail immediately; OpenAI-compatible routes rejecting system roles get one
compatibility retry and retain that preference for the process lifetime.

## Live grounding eval (opt-in)

From this directory, with Chrome/Chromium installed and the existing venv:

```sh
.venv/bin/python evals/grounding_eval.py
```

Set these environment variables beforehand:

| Variable | Value |
| --- | --- |
| `TAUCODE_EVAL_BASE_URL` | Your OpenAI-compatible registry gateway endpoint, without embedded credentials |
| `TAUCODE_EVAL_MODELS` | Comma-separated model IDs to compare |
| `TAUCODE_EVAL_KEY` | Gateway API key (a placeholder is sufficient for a local unauthenticated provider) |
| `TAUCODE_EVAL_KEY_COMMAND` | Alternative command whose stdout supplies the key; parsed as argv without a shell |

`TAUCODE_EVAL_KEY` takes precedence over the command. No endpoints, model IDs or
credentials are embedded in the eval. It uses `websockets`, already present in
the locked environment through `google-genai`; no dependencies are added.

The eval launches an isolated headless Chrome profile, connects to its CDP
WebSocket **without an Origin header**, renders a synthetic 1920×1080 workspace
at Retina scale, and reads element boxes from the DOM. It sends that synthetic
PNG through the sidecar's real pixel-grounding path and prints each hit/miss
and the hit rate per model. Provider failures count as misses and print redacted
messages. Only this generated page is captured; no desktop capture, macOS
permissions, mouse/keyboard injection or account sessions are involved.

This eval makes real provider requests and is intentionally outside pytest and
the verification command. Run it where Chrome and the configured gateway are
accessible. Tests use fake clients and synthetic PNGs without network access.
