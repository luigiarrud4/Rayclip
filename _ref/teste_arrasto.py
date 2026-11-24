"""Arrastar uma entrada da lista para uma pasta do Explorer cria o arquivo?

Detalhes que fazem a diferença:
- o app esconde ao perder o foco, mas o `dragstart` desliga isso (flag
  `ARRASTANDO`), então o arrasto precisa **começar** enquanto a janela ainda está
  visível: o teste entra em laço de 20 ms e prende o botão no primeiro instante em
  que a janela aparece;
- o mouse do Windows chega na janela sob o cursor mesmo sem foco, então o alvo do
  arrasto pode ser o Explorer no fundo;
- quem escreve o arquivo é o Explorer (o Chromium entrega `CF_HDROP`).
"""

import ctypes
import io
import json
import os
import subprocess
import time
import urllib.request
from ctypes import wintypes as w

import entrada
import harness_compare as h
import websocket

IDX = int(os.environ.get("RAYCLIP_ROW", "2"))
TRECHO = os.environ.get("RAYCLIP_ROW_TXT", "")
EXE = r"C:\Users\Lugia\Projects\rayclip\src-tauri\target\debug\rayclip.exe"
PASTA = os.path.join(os.environ["TEMP"], "alvo_arrasto")
SAIDA = os.path.join(os.environ["TEMP"], "teste_arrasto.txt")

user32 = ctypes.windll.user32
linhas = []


def diz(m):
    linhas.append(m)
    import sys

    if sys.stdout is not None:
        print(m)


def janela_explorer(pasta):
    alvo = []
    nome = os.path.basename(pasta)
    prot = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == "CabinetWClass" and user32.IsWindowVisible(hwnd):
            titulo = ctypes.create_unicode_buffer(512)
            user32.GetWindowTextW(hwnd, titulo, 512)
            if nome.lower() in titulo.value.lower():
                alvo.append(hwnd)
        return True

    user32.EnumWindows(prot(cb), 0)
    return alvo[0] if alvo else None


def rect(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r


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
    os.makedirs(PASTA, exist_ok=True)
    for f in os.listdir(PASTA):
        os.remove(os.path.join(PASTA, f))

    subprocess.Popen(["explorer.exe", PASTA])
    exp = None
    for _ in range(40):
        time.sleep(0.5)
        exp = janela_explorer(PASTA)
        if exp:
            break
    if not exp:
        diz("sem janela do Explorer")
        return
    time.sleep(1.5)
    er = rect(exp)
    diz(f"explorer 0x{exp:x} ({er.left},{er.top})-({er.right},{er.bottom})")

    subprocess.run(["taskkill", "/IM", "rayclip.exe", "/F"], capture_output=True)
    time.sleep(1.0)
    # Uma única instância: abrir outra sinaliza a primeira e a página troca no
    # meio do teste (foi o que fez o arrasto "não acontecer" de vez em quando).
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
        diz("janela nunca ficou visivel")
        return
    time.sleep(0.35)
    c = CDP()
    # Com um trecho de busca, filtra a lista: a linha fica parada (a lista recebe
    # entradas novas o tempo todo e as linhas andam).
    if TRECHO:
        c.ev(
            "(() => { const i = document.querySelector('.search input');"
            " if (!i) return 'sem campo';"
            " const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
            f" set.call(i, {json.dumps(TRECHO)});"
            " i.dispatchEvent(new Event('input', { bubbles: true })); return 'ok'; })()"
        )
        time.sleep(0.8)
    # Posição da janela e da linha, medidos com o app já carregado.
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    linha = json.loads(
        c.ev(
            "(() => { const busca = " + json.dumps(TRECHO) + ";"
            " const linhas = [...document.querySelectorAll('.row')];"
            " const e = busca ? linhas.find(x => (x.textContent||'').includes(busca)) : linhas[" + str(IDX) + "];"
            " if (!e) return null; const b = e.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2, id: e.dataset.id}); })()"
        )
        or "null"
    )
    if not linha:
        diz("lista vazia")
        return
    sx, sy = int(pos["x"]), int(pos["y"])
    x, y = sx + linha["x"], sy + linha["y"]
    diz(f"janela em ({sx},{sy}); linha {linha['id'][:8]} em ({x:.0f},{y:.0f})")

    import keysafe

    # O arrasto só começa se a janela tem foco (o primeiro mousedown sem foco
    # apenas ativa a janela) e se o payload da linha já está pronto (buscado no
    # hover/mousedown). Daí: focar, re-medir a linha (a lista muda sozinha),
    # pairar 600 ms e só então prender o botão.
    keysafe.focar(hwnd, espera=0.05)
    pos = json.loads(c.ev("JSON.stringify({x: window.screenX, y: window.screenY})"))
    linha = json.loads(
        c.ev(
            "(() => { const l = [...document.querySelectorAll('.row')];"
            " const e = " + ("l[0]" if TRECHO else "l[" + str(IDX) + "]") + " || null;"
            " if (!e) return null; const b = e.getBoundingClientRect();"
            " return JSON.stringify({x: b.x + 110, y: b.y + b.height / 2, id: e.dataset.id}); })()"
        )
        or "null"
    )
    if not linha:
        diz("lista vazia")
        return
    x, y = int(pos["x"]) + linha["x"], int(pos["y"]) + linha["y"]
    diz(f"agora a linha {linha['id'][:8]} está em ({x:.0f},{y:.0f})")
    entrada.mover(x, y)
    time.sleep(0.6)
    entrada.baixo()
    diz("botao preso; arrastando até o Explorer")

    # Lista de arquivos (o ponto que funcionou): centro horizontal, alto.
    destino = ((er.left + er.right) // 2, er.top + 160)
    for i in range(1, 25):
        entrada.mover(
            x + (destino[0] - x) * i // 24,
            y + (destino[1] - y) * i // 24,
        )
        time.sleep(0.04)
    time.sleep(0.4)
    diz(
        f"durante: arquivos={os.listdir(PASTA)} "
        f"janela_visivel={bool(user32.IsWindowVisible(hwnd))}"
    )
    entrada.cima()
    # O Explorer grava em segundo plano; 2,5 s não bastavam.
    depois = []
    for _ in range(24):
        time.sleep(0.5)
        depois = os.listdir(PASTA)
        if depois:
            break
    diz(f"depois de soltar: {depois}")
    for nome in depois:
        diz(f"  {nome} -> {os.path.getsize(os.path.join(PASTA, nome))} bytes")
    diz("RESULTADO: " + ("OK" if depois else "NADA FOI CRIADO"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
