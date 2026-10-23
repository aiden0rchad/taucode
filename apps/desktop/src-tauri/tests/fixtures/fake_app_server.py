#!/usr/bin/env python3
import json
import sys


def send(message):
    print(json.dumps(message, separators=(",", ":")), flush=True)


order = []
initialized = False
concurrent = []
server_request_caller = None

for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")

    if not order and method != "initialize":
        send({"id": message.get("id"), "error": {"code": -32001, "message": "initialize must be first"}})
        sys.exit(2)

    if method == "initialize":
        order.append("initialize")
        client = message.get("params", {}).get("clientInfo")
        send({"id": message["id"], "result": {"clientInfo": client}})
    elif method == "initialized":
        order.append("initialized")
        initialized = True
    elif method == "test/handshake":
        send({"id": message["id"], "result": {"order": order, "initialized": initialized}})
    elif method == "test/concurrent":
        concurrent.append(message)
        if len(concurrent) == 2:
            for request in reversed(concurrent):
                send({"id": request["id"], "result": request["params"]})
            concurrent.clear()
    elif method == "test/notifications":
        for sequence in range(3):
            send({"method": "test/notification", "params": {"sequence": sequence}})
        send({"id": message["id"], "result": {}})
    elif method == "test/server-request":
        server_request_caller = message["id"]
        send({"id": 700, "method": "item/commandExecution/requestApproval", "params": {
            "threadId": "thread-1", "turnId": "turn-1", "itemId": "item-1",
            "startedAtMs": 1, "environmentId": None, "command": "echo hello"
        }})
    elif method == "test/error":
        send({"id": message["id"], "error": {"code": -32000, "message": "boom"}})
    elif method == "test/pending-exit":
        sys.exit(23)
    elif method is None and message.get("id") == 700 and server_request_caller is not None:
        send({"id": server_request_caller, "result": {"response": message}})
        server_request_caller = None
    elif "id" in message:
        send({"id": message["id"], "result": message.get("params")})
