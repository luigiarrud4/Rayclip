"""Soltar texto do Rayclip dentro do Notepad cola o texto?

Prova de ponta a ponta com um editor de verdade: abre um `.txt` vazio no Notepad,
arrasta a entrada de texto para dentro, depois Ctrl+A + Ctrl+C no Notepad e lê o
clipboard — o que estiver lá é o que o Notepad recebeu.
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

EXE = r"C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
ARQ = os.path.join(os.environ["TEMP"], "rayclip_alvo.txt")
SAIDA = os.path.join(os.environ["TEMP"], "teste_notepad.txt")
MARCA = "TESTE ARRASTO TEXTO 987654"
user32 = ctypes.windll.user32
linhas = []


def diz(m):
    linhas.append(m)
    import sys

    if sys.stdout is not None:
        print(m)


def janela_notepad():
    achadas = []
    prot = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _):
        if not user32.IsWindowVisible(hwnd):
            return True
        pid = w.DWORD(0)
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if h.proc_name(pid.value).lower() != "notepad.exe":
            return True
        titulo = ctypes.create_unicode_buffer(512)
        user32.GetWindowTextW(hwnd, titulo, 512)
        if "rayclip_alvo" in titulo.value.lower():
            r = w.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(r))
            if r.right - r.left > 200:
                achadas.append(hwnd)
        return True

    user32.EnumWindows(prot(cb), 0)
    return achadas[0] if achadas else None


def rect(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r


def clipboard():
    r = subprocess.run(
        ["powershell", "-NoProfile", "-Command", "Get-Clipboard -Raw"],
        capture_output=True,
        text=True,
    )
    return (r.stdout or "").strip()


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


def main():
    io.open(ARQ, "w", encoding="utf-8", newline="\n").write("")
    subprocess.run(["powershell", "-NoProfile", "-Command", f"Set-Clipboard -Value '{MARCA}'"])
    time.sleep(1.5)
    subprocess.Popen(["notepad.exe", ARQ])
    nota = None
    for _ in range(30):
        time.sleep(0.5)
        nota = janela_notepad()
        if nota:
            break
    if not nota:
        diz("o Notepad nao abriu")
        return
    nr = rect(nota)
    diz(f"notepad 0x{nota:x} ({nr.left},{nr.top})-({nr.right},{nr.bottom})")

    subprocess.run(["taskkill", "/IM", "rayclip.exe", "/F"], capture_output=True)
    time.sleep(0.8)
    subprocess.Popen(
        [EXE, "--open"],
        creationflags=0x08000008,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
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
    c.ev(
        "(() => { const i = document.querySelector('.search input');"
        " if (!i) return 'sem campo';"
        " const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
        " set.call(i, 'TESTE ARRASTO TEXTO');"
        " i.dispatchEvent(new Event('input', { bubbles: true })); return 'ok'; })()"
    )
    time.sleep(0.8)
    alvo = json.loads(
        c.ev(
            "(() => { const e = document.querySelectorAll('.row')[0];"
            " if (!e) return null; const b = e.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2,"
            " texto: (e.textContent||'').slice(0, 40)}); })()"
        )
        or "null"
    )
    if not alvo:
        diz("a busca nao trouxe linha")
        return
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    x, y = int(pos["x"]) + alvo["x"], int(pos["y"]) + alvo["y"]
    diz(f"linha '{alvo['texto']}' em ({x:.0f},{y:.0f})")

    entrada.mover(x, y)
    time.sleep(0.6)
    entrada.baixo()
    destino = ((nr.left + nr.right) // 2, nr.top + 180)
    for i in range(1, 26):
        entrada.mover(x + (destino[0] - x) * i // 25, y + (destino[1] - y) * i // 25)
        time.sleep(0.04)
    time.sleep(0.5)
    entrada.cima()
    time.sleep(1.2)

    # Ctrl+A e Ctrl+C no Notepad e lê o clipboard.
    keysafe.focar(nota, espera=0.2)
    entrada.tecla(0x11, cima=False)  # segura Ctrl
    entrada.tecla(0x41)  # A
    entrada.tecla(0x43)  # C
    entrada.tecla(0x11, cima=True)  # solta Ctrl
    time.sleep(0.8)
    lido = clipboard()
    diz(f"notepad tem: {lido[:60]!r}")
    diz("RESULTADO: " + ("OK o texto foi colado" if MARCA in lido else "SEM TEXTO no notepad"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
