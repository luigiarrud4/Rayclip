"""Clique real do Windows muda a seleção? Verdade medida pelo CDP.

Três coisas que já quebraram este teste e agora estão certas:
- `entrada.py` (união `INPUT` de 40 bytes e coordenadas absolutas normalizadas
  com origem/tamanho da área virtual, sem a divisão por -1 que jogava todo
  clique na barra de tarefas);
- origem da janela somada ao retângulo da linha (o `getBoundingClientRect` é da
  página, não da tela);
- o app se esconde ao perder o foco, então a janela é reaberta pelo próprio
  `.exe` antes de cada clique.
"""

import io
import json
import os
import subprocess
import cdp_env
import sys
import time
import urllib.request

import websocket

import entrada
import harness_compare as h
import keysafe

EXE = r"C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
SAIDA = os.path.join(os.environ["TEMP"], "clique_cdp.txt")
linhas = []
SEL = "(() => { const e = document.querySelector('[data-selected=\"true\"]'); return e ? e.dataset.id : null; })()"


def diz(m):
    linhas.append(m)
    if sys.stdout:
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
        self.ws.send(json.dumps({"id": self.n, "method": "Runtime.evaluate",
                                 "params": {"expression": js, "awaitPromise": True, "returnByValue": True}}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                res = r["result"]
                if "exceptionDetails" in res:
                    return "EXC"
                return res["result"].get("value")


def mostrar(espera=6.0):
    hwnd = h.find_window("rayclip.exe")
    if hwnd:
        return hwnd
    subprocess.Popen([EXE], creationflags=0x08000008, env=cdp_env.env_cdp(),
                     stdin=subprocess.DEVNULL,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    fim = time.time() + espera
    while time.time() < fim:
        time.sleep(0.25)
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            return hwnd
    return None


def linha_em(c, indice):
    js = ("(() => { const e = [...document.querySelectorAll('.row')][%d];"
          " if (!e) return null; const r = e.getBoundingClientRect();"
          " return JSON.stringify({x: r.x + 110, y: r.y + r.height / 2, id: e.dataset.id}); })()" % indice)
    return json.loads(c.ev(js) or "null")


def main():
    import ctypes
    from ctypes import wintypes as w

    u32 = ctypes.windll.user32
    time.sleep(1.5)
    c = CDP()
    for indice in (2, 4, 1):
        hwnd = mostrar()
        if not hwnd:
            diz("janela nao encontrada")
            return
        rect = w.RECT()
        u32.GetWindowRect(hwnd, ctypes.byref(rect))
        alvo = linha_em(c, indice)
        if not alvo:
            diz(f"linha {indice}: nao encontrada")
            continue
        x, y = alvo["x"] + rect.left, alvo["y"] + rect.top
        antes = c.ev(SEL)
        keysafe.focar(hwnd, espera=0.2)
        fg = u32.GetForegroundWindow()
        vis = bool(u32.IsWindowVisible(hwnd))
        b = ctypes.create_unicode_buffer(80)
        u32.GetClassNameW(fg, b, 80)
        pid = w.DWORD(0)
        u32.GetWindowThreadProcessId(fg, ctypes.byref(pid))
        diz(f"  [diag] hwnd=0x{hwnd:x} visivel={vis} fg=0x{fg:x} classe={b.value} pid_fg={pid.value}")
        pronto = fg == hwnd and vis
        if pronto:
            entrada.clicar(x, y)
            time.sleep(0.35)
        depois = c.ev(SEL)
        diz(
            f"linha {indice} em ({x:.0f},{y:.0f}): janela_pronta={pronto} "
            f"antes={(antes or '')[:8]} depois={(depois or '')[:8]} esperado={alvo['id'][:8]} "
            f"{'OK' if depois == alvo['id'] else 'FALHOU'}"
        )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
