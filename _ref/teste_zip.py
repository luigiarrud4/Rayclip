"""Clicar/passar o mouse numa entrada de arquivo grande congela o app?

Antes: o payload era lido de forma síncrona no hover; um `.zip` de 120 MB virava
base64 na hora e a janela ia para "não respondendo". Agora a leitura é
assíncrona e tem teto de 32 MB (acima disso o arrasto é recusado com log).

A checagem de "travado" não depende de olhar a tela: `IsHungAppWindow` +
`SendMessageTimeout(WM_NULL)`, medidos antes e depois de clicar na linha.
"""

import ctypes
import io
import json
import os
import subprocess
import time
from ctypes import wintypes as w

import entrada
import harness_compare as h

ZIP = os.path.join(os.environ["TEMP"], "rayclip_teste_grande.zip")
SAIDA = os.path.join(os.environ["TEMP"], "teste_zip.txt")
user32 = ctypes.windll.user32
linhas = []


def diz(m):
    linhas.append(m)
    import sys

    if sys.stdout is not None:
        print(m)


def vivo(hwnd):
    """Responde a WM_NULL em 1 s? (janela travada não responde.)"""
    res = ctypes.c_size_t(0)
    ok = user32.SendMessageTimeoutW(hwnd, 0, 0, 0, 0x0002, 1000, ctypes.byref(res))
    return bool(ok) and not user32.IsHungAppWindow(hwnd)


def main():
    if not os.path.isfile(ZIP):
        diz("zip de teste nao existe")
        return
    # Copia o ARQUIVO para o clipboard (entrada com `paths`), não o texto.
    subprocess.run(
        ["powershell", "-NoProfile", "-Command", f"Set-Clipboard -Path '{ZIP}'"],
        capture_output=True,
    )
    time.sleep(2.5)
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("app nao esta visivel")
        return
    diz(f"app 0x{hwnd:x} responde={vivo(hwnd)}")

    import keysafe

    keysafe.focar(hwnd, espera=0.2)
    c = CDP()
    alvo = json.loads(
        c.ev(
            "(() => { const alvo = [...document.querySelectorAll('.row')]"
            ".find(e => (e.textContent||'').toLowerCase().includes('rayclip_teste_grande'));"
            " if (!alvo) return null; const b = alvo.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2}); })()"
        )
        or "null"
    )
    if not alvo:
        diz("linha do zip nao encontrada na lista")
        return
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    x, y = int(pos["x"]) + alvo["x"], int(pos["y"]) + alvo["y"]
    diz(f"linha do zip em ({x:.0f},{y:.0f})")

    # Hover: é aqui que o payload é pedido (250 ms parado).
    entrada.mover(x, y)
    time.sleep(1.2)
    diz(f"depois do hover: responde={vivo(hwnd)}")
    entrada.clicar(x, y)
    time.sleep(1.2)
    diz(f"depois do clique: responde={vivo(hwnd)} visivel={bool(user32.IsWindowVisible(hwnd))}")
    # E a lista continua respondendo a teclado?
    entrada.tecla(0x28)  # seta para baixo
    time.sleep(0.4)
    diz(f"depois da seta: responde={vivo(hwnd)}")


class CDP:
    def __init__(self):
        import urllib.request

        import websocket

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


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
