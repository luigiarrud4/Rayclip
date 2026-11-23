import json
import sys
import urllib.request

from websockets.sync.client import connect


def targets():
    return json.load(urllib.request.urlopen("http://127.0.0.1:9222/json"))


def evaluate(expression: str, timeout: int = 30000):
    page = next(t for t in targets() if t["type"] == "page")
    with connect(page["webSocketDebuggerUrl"], max_size=16 * 1024 * 1024) as ws:
        ws.send(
            json.dumps(
                {
                    "id": 1,
                    "method": "Runtime.evaluate",
                    "params": {
                        "expression": expression,
                        "returnByValue": True,
                        "awaitPromise": True,
                    },
                }
            )
        )
        while True:
            msg = json.loads(ws.recv())
            if msg.get("id") == 1:
                res = msg.get("result", {})
                if "exceptionDetails" in res:
                    return {"error": res["exceptionDetails"]}
                return res.get("result", {}).get("value")


if __name__ == "__main__":
    src = open(sys.argv[1], encoding="utf-8").read() if sys.argv[1].endswith(".js") else sys.argv[1]
    out = evaluate(src)
    print(json.dumps(out, ensure_ascii=False, indent=2) if not isinstance(out, str) else out)
