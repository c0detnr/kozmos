"""Local-only MCP stdio fixture; never used by the application."""
import json
import os
import subprocess
import sys
import time

mode = sys.argv[1] if len(sys.argv) > 1 else "normal"
if mode == "hang":
    descendant = subprocess.Popen(["sleep", "30"])
    with open(sys.argv[2], "w") as stream:
        json.dump([os.getpid(), descendant.pid], stream)
    time.sleep(30)
    sys.exit(0)

for line in sys.stdin:
    request = json.loads(line)
    if "id" not in request:
        continue
    method = request["method"]
    response = {"jsonrpc": "2.0", "id": request["id"]}
    if method == "initialize":
        response["result"] = {
            "protocolVersion": "2025-03-26",
            "capabilities": {} if mode == "no-tools" else {"tools": {}},
            "serverInfo": {"name": "kozmos-fixture", "version": "1.0"},
        }
    elif method == "tools/list":
        if mode == "reject":
            response["error"] = {"code": -32603, "message": "private-token-DO-NOT-DISPLAY"}
        else:
            next_page = request.get("params", {}).get("cursor") == "page2"
            response["result"] = {"tools": [{
                "name": "second" if next_page else "first",
                "description": os.environ.get("KOZMOS_FIXTURE", os.getcwd()),
                "inputSchema": {"type": "object"},
            }]}
            if not next_page:
                response["result"]["nextCursor"] = "page2"
    else:
        # A diagnostic must never execute tools.
        sys.exit(42)
    print(json.dumps(response), flush=True)
