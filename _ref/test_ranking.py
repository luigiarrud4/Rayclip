"""Paridade da busca: mesma query nos dois apps, mesma ordem?

Cinco textos com a mesma palavra rara em posicoes diferentes, copiados do **menos**
relevante para o **mais** relevante — assim a ordem por relevancia fica o inverso
exato da ordem por recencia, e da para saber qual regra cada app usa.

  #1 exact        qkxxxx
  #2 prefix       qkxxxx beta
  #3 word-start   beta qkxxxx
  #4 word-inside  beta qkxxxxbum
  #5 middle       xqkxxxxx        (copiado por ultimo = mais recente)

Ordem por relevancia esperada: #1 #2 #3 #4 #5
Ordem por recencia:             #5 #4 #3 #2 #1
"""
import io
import json
import os
import subprocess
import sys
import time
import urllib.request
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

import harness_compare as h  # noqa: E402
import websocket  # noqa: E402
from PIL import ImageGrab  # noqa: E402
from test_pm_raycast import janelas  # noqa: E402

SAIDA_DIR = r"C:\Windows\System32\_rayclip_tmp"


def textos(p):
    return [
        ("#1 exact", p),
        ("#2 prefix", f"{p} beta"),
        ("#3 word-start", f"beta {p}"),
        ("#4 word-inside", f"beta {p}bum"),
        ("#5 middle", f"x{p}x"),
    ]


def cdp(expr):
    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=5) as r:
        url = [t for t in json.load(r) if t.get("type") == "page"][0]["webSocketDebuggerUrl"]
    ws = websocket.create_connection(url, timeout=40)
    ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                        "params": {"expression": expr, "awaitPromise": True, "returnByValue": True}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == 1:
            ws.close()
            if "exceptionDetails" in m["result"]:
                raise RuntimeError(json.dumps(m["result"]["exceptionDetails"])[:400])
            return m["result"]["result"].get("value")


JS_BUSCA = """
(async () => {
  const inp = document.querySelector('input');
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
  setter.call(inp, %s);
  inp.dispatchEvent(new Event('input', { bubbles: true }));
  await new Promise(r => setTimeout(r, 700));
  const linhas = [...document.querySelectorAll('.row')].map(r => {
    const t = r.querySelector('.title');
    return t ? t.textContent.trim() : '';
  });
  return JSON.stringify(linhas);
})()
"""


def ordem_rayclip(p):
    linhas = json.loads(cdp(JS_BUSCA % json.dumps(p)) or "[]")
    return [x for x in linhas if p in x or x.count(" ")]


def ordem_raycast(pid, p, print_saida):
    h.send_keys("esc")
    time.sleep(0.6)
    for _ in range(3):
        h.send_keys("ctrl", "shift", "v")
        time.sleep(1.8)
        h.type_text(p)
        time.sleep(1.3)
        cands = janelas(pid)
        grande = [c for c in cands if c[0] > 20000]
        if grande:
            _, _, (l, t, r, b) = grande[0]
            ImageGrab.grab(bbox=(l, t, r, b)).save(print_saida)
            return print_saida
        h.send_keys("esc")
        time.sleep(0.5)
    return None


def main():
    p = "qk" + uuid.uuid4().hex[:4]
    casos = textos(p)
    print("query:", p)
    for rotulo, txt in casos:
        h.set_clipboard_text(txt)
        print("copiado", rotulo, "->", txt)
        time.sleep(1.7)

    pid = int(subprocess.run(["powershell", "-NoProfile", "-Command",
                              "(Get-Process Raycast).Id"],
                             capture_output=True, text=True).stdout.strip())
    print("raycast pid:", pid)

    pr = ordem_raycast(pid, p, os.path.join(SAIDA_DIR, f"rank_raycast_{p}.png"))
    print("PRINT raycast:", pr)
    try:
        print("rayclip (CDP):", json.dumps(ordem_rayclip(p), ensure_ascii=False))
    except Exception as e:
        print("rayclip (CDP) falhou:", e)
    print("TEXTOS:", json.dumps(casos, ensure_ascii=False))


if __name__ == "__main__":
    main()
