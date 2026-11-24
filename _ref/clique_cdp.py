"""Clique REAL do Windows muda a seleção? Leitura da verdade pelo CDP.

Junta as três peças que estavam quebradas/no lugar errado:
- `entrada.clicar` (união `INPUT` correta: `SendInput` devolve 1);
- guarda de foco + `WindowFromPoint` antes do clique;
- oráculo pelo CDP (`[data-selected="true"]`), não por pixels — pixel não diz
  *qual* linha ficou selecionada.

Roda com `pythonw` (sem console, que rouba o foco) e grava
%TEMP%\\clique_cdp.txt.
"""

import io
import json
import os
import sys
import time
import urllib.request

import websocket

import entrada
import harness_compare as h

SAIDA = os.path.join(os.environ["TEMP"], "clique_cdp.txt")
linhas = []


def diz(m):
    linhas.append(m)
    if sys.stdout:
        print(m)


class CDP:
    def __init__(self):
        tid = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]["id"]
        self.ws = websocket.create_connection(
            "ws://127.0.0.1:9222/devtools/page/" + tid, suppress_origin=True, timeout=20
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
                    return "EXC"
                return res["result"].get("value")


SEL = "(() => { const e = document.querySelector('[data-selected=\"true\"]'); return e ? e.dataset.id : null; })()"


def linha_em(c, indice):
    """Retângulo (pixels de tela) da linha `indice` da lista, e seu id."""
    js = (
        "(() => { const e = [...document.querySelectorAll('[data-id]')][%d];"
        " if (!e) return null; const r = e.getBoundingClientRect();"
        " return JSON.stringify({x: r.x + r.width / 2, y: r.y + r.height / 2, id: e.dataset.id}); })()"
        % indice
    )
    return json.loads(c.ev(js) or "null")


def main():
    time.sleep(3)
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela nao encontrada (o app precisa estar visivel)")
        return
    c = CDP()
    diz(f"janela 0x{hwnd:x}")
    for indice in (2, 4, 1):
        alvo = linha_em(c, indice)
        if not alvo:
            diz(f"linha {indice}: nao encontrada")
            continue
        # coordenadas da linha são de tela; soma a origem da janela já vem no rect
        x, y = alvo["x"], alvo["y"]
        antes = c.ev(SEL)
        feito = "sem foco"
        for _ in range(5):
            hk = __import__("keysafe")
            hk.focar(hwnd, espera=0.25)
            if __import__("ctypes").windll.user32.GetForegroundWindow() != hwnd:
                continue
            entrada.clicar(x, y)
            time.sleep(0.45)
            depois = c.ev(SEL)
            if depois != antes:
                feito = "selecionou"
                break
            feito = "sem mudanca"
        diz(
            f"linha {indice} em ({x:.0f},{y:.0f}): {feito} | antes={(antes or '')[:8]} "
            f"depois={(c.ev(SEL) or '')[:8]} esperado={alvo['id'][:8]}"
        )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
