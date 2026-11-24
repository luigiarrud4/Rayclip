"""Qual nome/extensão/tipo o Rayclip oferece ao arrastar uma linha?

Passa o mouse na linha (o hover de 250 ms pede o payload) e lê a linha
`drag payload: nome.ext tipo N bytes` do log do app. Serve para conferir que uma
entrada de ARQUIVO `.png` do disco continua `.png` (antes saía `.txt`).
"""

import io
import json
import os
import sys
import time
import urllib.request

import entrada
import harness_compare as h
import websocket

EXE = r"C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
LOG = os.path.join(os.environ["TEMP"], "rayclip.log")
TRECHO = os.environ.get("TRECHO", "Rayclip image 2026-11-24 20-26-36")
SAIDA = os.path.join(os.environ["TEMP"], "probe_payload.txt")
linhas = []


def diz(m):
    linhas.append(m)
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
                return r["result"].get("result", {}).get("value")


def main():
    import subprocess

    subprocess.run(["taskkill", "/IM", "rayclip.exe", "/F"], capture_output=True)
    time.sleep(0.8)
    subprocess.Popen([EXE, "--open"], creationflags=0x08000008,
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    hwnd = None
    for _ in range(200):
        achada = h.find_window("rayclip.exe")
        if achada:
            hwnd = achada
            break
        time.sleep(0.02)
    if not hwnd:
        diz("app nao abriu")
        return
    time.sleep(0.4)
    import keysafe

    keysafe.focar(hwnd, espera=0.05)
    c = CDP()
    # Busca a linha pelo texto: fica isolada e não muda de posição.
    c.ev(
        "(() => { const i = document.querySelector('.search input');"
        " const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
        f" set.call(i, {json.dumps(TRECHO)});"
        " i.dispatchEvent(new Event('input', { bubbles: true })); return 'ok'; })()"
    )
    time.sleep(0.8)
    alvo = json.loads(
        c.ev(
            "(() => { const l = [...document.querySelectorAll('.row')];"
            " const e = l[0]; if (!e) return null; const b = e.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2,"
            " texto: (e.textContent||'').slice(0, 60)}); })()"
        )
        or "null"
    )
    if not alvo:
        diz("busca sem resultado")
        return
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    x, y = int(pos["x"]) + alvo["x"], int(pos["y"]) + alvo["y"]
    diz(f"linha '{alvo['texto']}'")
    # Sair da linha antes de entrar: se o cursor já está parado nela, não há
    # `mouseenter` e o hover não pede payload nenhum.
    entrada.mover(5, 5)
    time.sleep(0.3)
    entrada.mover(x, y)
    time.sleep(1.0)
    cauda = [l for l in io.open(LOG, encoding="utf-8", errors="replace").read().splitlines()
             if "drag payload" in l][-2:]
    diz("log: " + " | ".join(cauda))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
