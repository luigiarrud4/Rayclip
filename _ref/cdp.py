import json, sys, urllib.request
from websockets.sync.client import connect

def targets():
    return json.load(urllib.request.urlopen("http://127.0.0.1:9222/json"))

def evaluate(expr):
    page = next(t for t in targets() if t["type"] == "page")
    with connect(page["webSocketDebuggerUrl"], max_size=50*1024*1024) as ws:
        ws.send(json.dumps({"id":1,"method":"Runtime.evaluate",
            "params":{"expression":expr,"returnByValue":True,"awaitPromise":True}}))
        while True:
            msg = json.loads(ws.recv())
            if msg.get("id") == 1:
                r = msg.get("result",{}).get("result",{})
                return r.get("value", r.get("description", msg))

if __name__ == "__main__":
    print(evaluate(sys.argv[1] if len(sys.argv)>1 else "document.title"))
