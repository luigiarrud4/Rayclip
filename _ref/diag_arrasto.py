"""Que eventos de arrasto o WebView2 realmente dispara? (diagnóstico via CDP)"""

import json
import os
import subprocess
import time
import urllib.request
from ctypes import wintypes as w
import ctypes

import entrada
import harness_compare as h
import websocket

EXE = r"C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
SAIDA = os.path.join(os.environ["TEMP"], "diag_arrasto.txt")
linhas = []


def diz(m):
    linhas.append(m)
    import sys

    if sys.stdout is not None:
        print(m)


class CDP:
    def __init__(self):
        alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=30
        )
        self.n = 0

    def ev(self, js):
        self.n += 1
        self.ws.send(
            json.dumps(
                {
                    "id": self.n,
                    "method": "Runtime.evaluate",
                    "params": {"expression": js, "awaitPromise": True, "returnByValue": True},
                }
            )
        )
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                res = r["result"]
                if "exceptionDetails" in res:
                    return "EXC: " + json.dumps(res["exceptionDetails"])[:300]
                return res["result"].get("value")


INSTRUMENTO = """
(() => {
  window.__dragLog = [];
  const log = (m) => window.__dragLog.push(m);
  for (const tipo of ['mousedown', 'mousemove', 'dragstart', 'drag', 'dragend', 'dragenter', 'mouseup', 'click']) {
    window.addEventListener(tipo, (e) => {
      if (tipo === 'mousemove' || tipo === 'drag') {
        if (window.__dragLog.filter(x => x.startsWith(tipo)).length > 3) return;
      }
      const alvo = e.target && e.target.className ? String(e.target.className).slice(0, 20) : '?';
      log(`${tipo} alvo=${alvo} botoes=${e.buttons}`);
      if (tipo === 'dragstart') {
        log('  tipos: ' + JSON.stringify(Array.from(e.dataTransfer.types)));
        for (const t of ['text/plain', 'text/uri-list']) {
          try { log('  ' + t + ' = ' + JSON.stringify(String(e.dataTransfer.getData(t)).slice(0, 60))); }
          catch (err) { log('  ' + t + ' erro ' + err); }
        }
        try {
          const p = e.dataTransfer.getData('DownloadURL');
          log('  DownloadURL lido de volta: ' + String(p).slice(0, 40));
        } catch (err) { log('  erro ao ler: ' + err); }
      }
    }, true);
  }
  return 'ok';
})()
"""


def main():
    c = CDP()
    diz(c.ev(INSTRUMENTO))
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    linha = json.loads(
        c.ev(
            "(() => { const e = [...document.querySelectorAll('.row')][2];"
            " if (!e) return null; const b = e.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2}); })()"
        )
        or "null"
    )
    if not linha:
        diz("lista vazia")
        return
    x, y = int(pos["x"]) + linha["x"], int(pos["y"]) + linha["y"]
    diz(f"alvo ({x:.0f},{y:.0f})")
    entrada.mover(x, y)
    time.sleep(0.4)
    entrada.baixo()
    for i in range(1, 15):
        entrada.mover(x + i * 10, y + i * 4)
        time.sleep(0.04)
    time.sleep(0.3)
    entrada.cima()
    time.sleep(0.6)
    diz("eventos: " + json.dumps(json.loads(c.ev("JSON.stringify(window.__dragLog)") or "[]"), indent=1))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        import io

        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
