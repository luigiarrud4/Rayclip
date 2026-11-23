import json, sys
from websockets.sync.client import connect

def ev(expr):
    t = next(x for x in json.load(open("/dev/null")) ) if False else None
    import urllib.request
    page = next(t for t in json.load(urllib.request.urlopen("http://127.0.0.1:9222/json")) if t["type"]=="page")
    with connect(page["webSocketDebuggerUrl"], max_size=50*1024*1024) as ws:
        ws.send(json.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":expr,"returnByValue":True,"awaitPromise":True}}))
        while True:
            m=json.loads(ws.recv())
            if m.get("id")==1:
                r=m.get("result",{}).get("result",{})
                return r.get("value", r.get("description", m))

for e in sys.argv[1:]:
    print(ev(e))
