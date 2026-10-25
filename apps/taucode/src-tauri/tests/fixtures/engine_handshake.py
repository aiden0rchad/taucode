import json
import sys

if sys.argv[1] == "fail":
    print("fixture startup failed", file=sys.stderr)
    sys.exit(1)

for line in sys.stdin:
    request = json.loads(line)
    if sys.argv[1] == "acp":
        assert request["method"] == "initialize"
        reply = {"jsonrpc": "2.0", "id": request["id"], "result": {"protocolVersion": 1, "agentInfo": {"name": "fixture", "version": "1.2.3"}}}
    else:
        assert request["type"] == "get_state"
        reply = {"type": "response", "command": "get_state", "id": request["id"], "success": True, "data": {"sessionId": "test", "version": "1.2.3"}}
    print(json.dumps(reply), flush=True)
