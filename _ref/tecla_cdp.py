"""Seta do Windows (SendInput) muda a seleção? Verificado pelo CDP.

Separa duas coisas que se confundem no teste cego por pixels:
1. a tecla **chegou** ao app (se não chegou, nada muda e a culpa é do teste);
2. a seleção **andou** (se andou, a lógica está boa).

Roda com `pythonw` (sem console, para o foco não ser roubado) e grava
%TEMP%\\tecla_cdp.txt.
"""

import ctypes
import io
import json
import os
import sys
import time
import urllib.request
from ctypes import wintypes as w

import websocket

import entrada
tecla = entrada.tecla

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
SAIDA = os.path.join(os.environ["TEMP"], "tecla_cdp.txt")
linhas = []


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
                return r["result"]["result"].get("value")


def selecionado(c):
    return c.ev(
        "(() => { const e = document.querySelector('[data-selected=\"true\"]');"
        " return e ? e.dataset.id : null; })()"
    )


def main():
    global linhas
    diz = lambda m: linhas.append(m)  # noqa: E731
    time.sleep(3)
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        p.mostrar()
        hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela nao encontrada")
        return
    c = CDP()
    k.focar(hwnd, espera=0.4)
    time.sleep(0.3)
    diz(f"foco={u32.GetForegroundWindow() == hwnd} hwnd={hwnd}")
    diz(f"inicial: {(selecionado(c) or '')[:8]}")
    for rotulo, vk in (("seta ↓", 0x28), ("seta ↓", 0x28), ("seta ↑", 0x26)):
        k.focar(hwnd, espera=0.3)
        tecla(vk)
        time.sleep(0.35)
        diz(f"{rotulo}: {(selecionado(c) or '')[:8]} foco={u32.GetForegroundWindow() == hwnd}")

    # a tecla chega ao WebView2? Digita "x" e le o campo de busca pelo CDP.
    diz("foco do documento: " + str(c.ev("document.activeElement && document.activeElement.tagName")))
    before = c.ev("(document.querySelector('input') || {}).value ?? '(sem input)'")
    k.focar(hwnd, espera=0.3)
    tecla(0x58)  # X
    time.sleep(0.4)
    after = c.ev("(document.querySelector('input') || {}).value ?? '(sem input)'")
    diz(f"digitar x: antes={before!r} depois={after!r}")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
