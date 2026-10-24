"""Offline pi RPC peer; no provider, account, keychain, or network access."""
import json
import os
import sys
import argparse
import pathlib
import uuid

parser = argparse.ArgumentParser()
for flag in ("session", "session-dir", "provider", "model", "mode"):
    parser.add_argument("--" + flag)
args, _ = parser.parse_known_args()
session = pathlib.Path(args.session or "session.jsonl")
header = json.loads(session.read_text().splitlines()[0]) if session.is_file() else {"id": "fake-thread"}
model = {"id": args.model or "fake", "provider": args.provider or "local", "contextWindow": 128000}
if args.session_dir:
    assert os.environ["TAUCODE_SESSION_KEY"] == header["id"]
    assert os.environ["PI_TELEMETRY"] == "0"
    assert os.environ["PI_SKIP_VERSION_CHECK"] == "1"
    assert not (pathlib.Path(os.environ["PI_CODING_AGENT_DIR"]) / "auth.json").exists()
    assert "keychain-secret-canary" not in " ".join(sys.argv)
    if os.environ.get("TAUCODE_WORK") != "1":
        assert os.environ["TAUCODE_PROVIDER_FIRST_KEY"] == "keychain-secret-canary"
    if os.environ.get("TAUCODE_CHAT") == "1":
        assert "--no-tools" in sys.argv
        assert "--no-context-files" in sys.argv
        assert "--system-prompt" in sys.argv
        assert any(arg.endswith("taucode-gate.ts") for arg in sys.argv)
        assert not any(arg.endswith("taucode-subagents.ts") for arg in sys.argv)
        if os.environ.get("TAUCODE_CHAT_WEB") != "1":
            assert "builtin:mcp" not in sys.argv
            assert json.loads((pathlib.Path(os.environ["PI_CODING_AGENT_DIR"]) / "mcp.json").read_text())["mcpServers"] == {}
        assert pathlib.Path.cwd() == pathlib.Path(header["cwd"])
        session.with_suffix(".spawn").write_text(json.dumps({"args": sys.argv, "cwd": str(pathlib.Path.cwd())}))


def record(request):
    if args.session_dir:
        with session.with_suffix(".requests").open("a") as output:
            output.write(json.dumps(request) + "\n")


def emit(value):
    print(json.dumps(value, ensure_ascii=False), flush=True)


held = None
for line in sys.stdin:
    request = json.loads(line)
    record(request)
    kind = request["type"]
    response = {"type": "response", "id": request.get("id"), "command": kind, "success": True}
    if kind == "hold":
        held = response
        continue
    if kind == "release":
        emit(response)
        emit(held)
        continue
    if kind == "get_state":
        response["data"] = {"sessionId": header["id"], "sessionFile": str(session), "model": model}
    if kind == "set_model":
        model.update(id=request["modelId"], provider=request["provider"])
        response["data"] = model
    if kind == "get_messages":
        response["data"] = {"messages": []}
    if kind == "clone":
        header = dict(header, id=str(uuid.uuid4()))
        session = session.with_name(header["id"] + ".jsonl")
        session.write_text(json.dumps(header) + "\n")
        response["data"] = {"cancelled": False}
    if kind == "compact":
        emit({"type": "compaction_start", "reason": "manual"})
        emit({"type": "compaction_end", "result": {"summary": "compacted"}, "aborted": False})
    if kind == "prompt":
        emit(response)
        emit({"type": "agent_start"})
        emit({"type": "message_start", "message": {"role": "assistant"}})
        emit({"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "contentIndex": 0, "delta": "one\u2028two\u2029three"}})
        if request.get("message") == "crash":
            sys.exit(7)
        continue
    if kind == "abort":
        emit({"type": "agent_settled", "aborted": True})
    emit(response)

marker = os.environ.get("FAKE_PI_EOF")
if marker:
    with open(marker, "w", encoding="utf-8") as output:
        output.write("graceful")
