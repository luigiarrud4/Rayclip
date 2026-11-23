import base64, json, sys, urllib.request
from websockets.sync.client import connect

def page():
    return next(t for t in json.load(urllib.request.urlopen("http://127.0.0.1:9222/json")) if t["type"] == "page")

def shot(path, clip=None):
    p = page()
    with connect(p["webSocketDebuggerUrl"], max_size=80 * 1024 * 1024) as ws:
        params = {"format": "png"}
        if clip:
            params["clip"] = clip
        ws.send(json.dumps({"id": 1, "method": "Page.captureScreenshot", "params": params}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 1:
                data = m["result"]["data"]
                open(path, "wb").write(base64.b64decode(data))
                return len(data)

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "shot.png"
    print(shot(out))
