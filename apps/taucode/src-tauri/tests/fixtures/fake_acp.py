"""Offline ACP fixture: all methods use the pinned adapter's wire shapes."""
import json
import os
import sys
import uuid

assert "--hide-claude-auth" in sys.argv
assert os.environ["ANTHROPIC_API_KEY"] == "fixture-api-secret"
assert "fixture-api-secret" not in " ".join(sys.argv)
assert {key for key in os.environ if key.startswith("CLAUDE_")} == {"CLAUDE_CONFIG_DIR", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"}
sessions = {}
pending = {}
serial = 0


def send(value):
    print(json.dumps({"jsonrpc": "2.0", **value}), flush=True)


def response(request, result):
    send({"id": request["id"], "result": result})


def update(session, update_kind, **fields):
    send({"method": "session/update", "params": {"sessionId": session, "update": {"sessionUpdate": update_kind, **fields}}})


def capabilities(session):
    return {"sessionId": session, "modes": {"currentModeId": "default", "availableModes": [{"id": "default", "name": "Default"}, {"id": "plan", "name": "Plan"}]}, "configOptions": [{"id": "model", "category": "model", "currentValue": "model", "type": "select", "options": [{"value": "model", "name": "Model"}, {"value": "other", "name": "Other"}]}]}


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    params = request.get("params", {})
    session = params.get("sessionId")
    if method == "initialize":
        assert params["clientCapabilities"] == {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": False, "_meta": {"terminal_output_delta": True}}
        response(request, {"protocolVersion": 1, "agentCapabilities": {"loadSession": True, "promptCapabilities": {"image": True}, "mcpCapabilities": {"http": True}, "sessionCapabilities": {"fork": {}}}, "_meta": {"steering": {"supported": True}}})
    elif method in ("session/new", "session/fork"):
        serial += 1
        session = str(uuid.uuid4())
        sessions[session] = params
        assert params["_meta"]["claudeCode"]["options"]["allowDangerouslySkipPermissions"] is False
        response(request, capabilities(session))
    elif method == "session/load":
        sessions[session] = params
        update(session, "user_message_chunk", content={"type": "text", "text": "restored question"})
        update(session, "agent_message_chunk", content={"type": "text", "text": "restored answer"})
        response(request, capabilities(session))
    elif method in ("session/set_mode", "session/set_config_option", "_session/steering"):
        assert params.get("modeId") != "bypassPermissions"
        response(request, {})
    elif method == "session/prompt":
        text = " ".join(p.get("text", "") for p in params["prompt"])
        if text == "auth":
            send({"id": request["id"], "error": {"code": -32000, "message": "Authentication required: sign in"}})
            continue
        if text in ("beta-error", "chunk-error", "result-error"):
            detail = "API Error: 400 Routing and Anthropic beta override headers are disabled for Claude. fixture-api-secret"
            if text != "beta-error":
                update(session, "agent_message_chunk", content={"type": "text", "text": detail})
            if text == "result-error":
                response(request, {"stopReason": "error"})
            else:
                send({"id": request["id"], "error": {"code": -32603, "message": "Internal error: " + detail if text == "beta-error" else "Internal error", "data": {"errorKind": "provider_error"}}})
            continue
        if text == "wait":
            pending[session] = request
            continue
        update(session, "agent_thought_chunk", content={"type": "text", "text": "Thinking"})
        update(session, "agent_message_chunk", content={"type": "text", "text": "Hello "})
        update(session, "agent_message_chunk", content={"type": "text", "text": "world"})
        for kind, title in [("execute", "Bash"), ("edit", "Edit"), ("read", "Read"), ("search", "Grep"), ("fetch", "WebFetch"), ("other", "mcp__codegraph__explore"), ("other", "Task")]:
            update(session, "tool_call", toolCallId=title, kind=kind, name=title, title=title, rawInput={"command": "echo hi", "file_path": "file.txt", "prompt": "inspect"}, status="in_progress")
            content = [{"type": "diff", "path": "file.txt", "oldText": "old\n", "newText": "new\n"}] if kind == "edit" else [{"type": "content", "content": {"type": "text", "text": "output"}}]
            update(session, "tool_call_update", toolCallId=title, status="completed", content=content, rawOutput={"exitCode": 0})
        update(session, "plan", entries=[{"content": "Implement", "priority": "high", "status": "in_progress"}])
        update(session, "usage_update", used=42, size=200000)
        pending["permission"] = request
        send({"id": "permission", "method": "session/request_permission", "params": {"sessionId": session, "toolCall": {"toolCallId": "approval", "title": "Bash", "kind": "execute", "rawInput": {"command": "echo approved"}}, "options": [{"optionId": "once", "name": "Allow", "kind": "allow_once"}, {"optionId": "always", "name": "Always", "kind": "allow_always"}, {"optionId": "no", "name": "Reject", "kind": "reject_once"}]}})
    elif method == "session/cancel":
        original = pending.pop(session, None) or pending.pop("permission", None)
        if original:
            response(original, {"stopReason": "cancelled"})
    elif method is None and request["id"] == "permission":
        assert request["result"]["outcome"].get("optionId") != "always"
        original = pending.pop("permission", None)
        if original:
            response(original, {"stopReason": "end_turn"})
    else:
        send({"id": request.get("id"), "error": {"code": -32601, "message": "Method not found"}})
