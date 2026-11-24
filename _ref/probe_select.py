"""Probe: clicar numa linha da lista seleciona? (CDP, mouse de verdade)

Usa `Input.dispatchMouseEvent` (evento confiável no nível do browser, dispara
`onClick` do React — `.click()` do JS não serviria).
"""

import json
import time
import urllib.request

import websocket

QSEL = '[data-selected="true"]'


class CDP:
    def __init__(self):
        tid = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]["id"]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + tid, suppress_origin=True, timeout=30
        )
        self.n = 0

    def send(self, method, params=None):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method, "params": params or {}}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                return r

    def ev(self, js):
        r = self.send(
            "Runtime.evaluate",
            {"expression": js, "awaitPromise": True, "returnByValue": True},
        )
        res = r.get("result", {})
        if "exceptionDetails" in res:
            return "EXC: " + json.dumps(res["exceptionDetails"])[:300]
        return res["result"].get("value")


def estado(c):
    js = (
        "(() => { const s = document.querySelector(%r);"
        " const rows = [...document.querySelectorAll('[data-id]')];"
        " return JSON.stringify({sel: s ? s.dataset.id : null,"
        " nsel: document.querySelectorAll(%r).length,"
        " nrows: rows.length,"
        " primeiro: rows[0] && rows[0].dataset.id}); })()" % (QSEL, QSEL)
    )
    return json.loads(c.ev(js))


def clica(c, indice):
    js = (
        "(() => { const e = [...document.querySelectorAll('[data-id]')][%d];"
        " if (!e) return null; const r = e.getBoundingClientRect();"
        " return JSON.stringify({x: r.x + r.width / 2, y: r.y + r.height / 2,"
        " id: e.dataset.id}); })()" % indice
    )
    alvo = c.ev(js)
    if not alvo:
        return None
    alvo = json.loads(alvo)
    for tipo in ("mousePressed", "mouseReleased"):
        c.send(
            "Input.dispatchMouseEvent",
            {"type": tipo, "x": alvo["x"], "y": alvo["y"], "button": "left", "clickCount": 1},
        )
    time.sleep(0.7)
    return alvo


if __name__ == "__main__":
    c = CDP()
    print("antes  :", estado(c))
    alvo = clica(c, 2)
    print("cliquei:", (alvo or {}).get("id"))
    print("depois :", estado(c))
    alvo = clica(c, 5)
    print("cliquei:", (alvo or {}).get("id"))
    print("depois :", estado(c))
