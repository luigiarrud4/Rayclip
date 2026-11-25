"""Print do painel com o filtro "Somente vídeos" ativo, via CDP (sem depender de
foco de janela: a janela esconde-se quando perde o foco)."""

import base64
import json
import sys
import time
import urllib.request

import websocket  # type: ignore

SAIDA = r"C:\Windows\System32\_rayclip_tmp\s29.png"


def alvo():
    for _ in range(40):
        try:
            with urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=2) as r:
                for t in json.load(r):
                    url = t.get("url", "")
                    if t.get("type") != "page" or "/webmeta/" in url:
                        continue
                    if url.rstrip("/").endswith("localhost"):
                        return t["webSocketDebuggerUrl"]
        except OSError:
            pass
        time.sleep(0.5)
    raise SystemExit("sem alvo CDP")


ws = websocket.create_connection(alvo(), timeout=30)
n = 0


def cmd(method, **params):
    global n
    n += 1
    ws.send(json.dumps({"id": n, "method": method, "params": params}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == n:
            return m.get("result")


def av(expr):
    r = cmd("Runtime.evaluate", expression=expr, returnByValue=True)
    return r.get("result", {}).get("value")


time.sleep(1)
av("""(() => {
  const b = document.querySelector('.filter-button');
  if (b) b.click();
  return 1;
})()""")
time.sleep(0.5)
av("""(() => {
  const it = [...document.querySelectorAll('.filter-menu .filter-item')]
    .find(e => /Videos Only|Somente v/.test(e.textContent));
  if (it) it.dispatchEvent(new MouseEvent('mousedown', {bubbles: true, cancelable: true}));
  return 1;
})()""")
time.sleep(2.5)

print(av("document.querySelector('.filter-button').textContent.trim()"))
print(av("[...document.querySelectorAll('.row .title')].map(e => e.textContent.trim())"))
print(
    av(
        """JSON.stringify([...document.querySelectorAll('.details .preview')]
             .map(e => e.className + ' :: ' + e.querySelectorAll('img,video').length))"""
    )
)

r = cmd("Page.captureScreenshot", format="png")
with open(SAIDA, "wb") as f:
    f.write(base64.b64decode(r["data"]))
print("print em", SAIDA)
sys.exit(0)
