"""Regressões depois de um arrasto para fora (itens do plano 4.2.3).

1. a janela **não** escondeu durante o arrasto e continua visível depois;
2. o clique seguinte ainda seleciona (o `arrastou` da linha não pode engolir o
   clique pra sempre);
3. `Alt+Space` volta a funcionar depois do arrasto (a flag `ARRASTANDO` soltou).

Uso: python teste_pos_arrasto.py
"""

import ctypes
import io
import json
import os
import time
import urllib.request

import entrada
import harness_compare as h
import keysafe
import websocket

SAIDA = os.path.join(os.environ["TEMP"], "teste_pos_arrasto.txt")
user32 = ctypes.windll.user32
linhas = []


def diz(m):
    linhas.append(m)
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
    hwnd = None
    # `find_window` só acha janela **visível**: logo depois do `reiniciar.ps1`
    # ela ainda está subindo.
    for _ in range(300):
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            break
        time.sleep(0.1)
    if not hwnd:
        diz("app nao esta rodando (janela invisivel)")
        return
    diz(f"janela visivel no inicio: {bool(user32.IsWindowVisible(hwnd))}")
    if not keysafe.focar(hwnd, espera=1.0):
        # Logo depois de um arrasto o app pode estar se escondendo/mostrando; o
        # foco é a única coisa que falha, o resto do teste continua válido.
        for _ in range(6):
            time.sleep(0.5)
            if keysafe.focar(hwnd, espera=1.0):
                break
    if not keysafe.focar(hwnd, espera=1.0):
        diz("nao consegui focar")
        return
    c = CDP()
    for _ in range(40):
        if c.ev("document.querySelectorAll('.row').length"):
            break
        time.sleep(0.25)

    # Limpa a busca (o teste anterior pode ter deixado filtro).
    c.ev(
        "(() => { const i=document.querySelector('.search input'); if(!i) return 'sem';"
        " const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;"
        " set.call(i,''); i.dispatchEvent(new Event('input',{bubbles:true})); return 'ok'; })()"
    )
    time.sleep(0.7)

    pos = json.loads(c.ev("JSON.stringify({x:window.screenX, y:window.screenY})"))
    medidas = json.loads(
        c.ev(
            "JSON.stringify([...document.querySelectorAll('.row')].slice(0,3).map(e => {"
            " const b = e.getBoundingClientRect();"
            " return {id: e.dataset.id, x: b.x + 110, y: b.y + b.height/2,"
            " sel: e.getAttribute('data-selected')}; }))"
        )
    )
    if len(medidas) < 2:
        diz("lista curta demais")
        return

    # 1) Seleciona a primeira linha e confere que o app está vivo e visível.
    alvo = medidas[0]
    entrada.clicar(int(pos["x"] + alvo["x"]), int(pos["y"] + alvo["y"]))
    time.sleep(0.5)
    sel = c.ev("(document.querySelector('.row[data-selected=\"true\"]')||{}).dataset?.id || null")
    diz(f"depois do clique: selecionada={str(sel)[:8]} (esperada {alvo['id'][:8]}) visivel={bool(user32.IsWindowVisible(hwnd))}")

    # 2) Clica em outra linha: a seleção tem que mudar (o `arrastou` não pode
    #    ficar preso depois de um arrasto).
    outro = medidas[1]
    entrada.clicar(int(pos["x"] + outro["x"]), int(pos["y"] + outro["y"]))
    time.sleep(0.5)
    sel2 = c.ev("(document.querySelector('.row[data-selected=\"true\"]')||{}).dataset?.id || null")
    diz(f"segundo clique: selecionada={str(sel2)[:8]} (esperada {outro['id'][:8]})")

    # 3) Alt+Space: esconde e volta (prova que `ARRASTANDO` foi solto).
    visivel_antes = bool(user32.IsWindowVisible(hwnd))
    h.send_keys("alt", "space", forcar=False)
    time.sleep(0.9)
    visivel_depois = bool(user32.IsWindowVisible(hwnd))
    diz(f"alt+space: visivel {visivel_antes} -> {visivel_depois}")
    # Com a janela escondida o foco foi para outro app e o `keysafe` recusaria
    # (é o comportamento certo). Aqui `forcar=True` é seguro: `Alt+Space` é
    # atalho **global** do Rayclip, então ele volta mesmo sem foco — e o efeito
    # colateral em outro app é só o menu de sistema.
    h.send_keys("alt", "space", forcar=True)
    time.sleep(0.9)
    visivel_volta = bool(user32.IsWindowVisible(hwnd))
    diz(f"alt+space de novo: visivel={visivel_volta}")

    ok = (
        str(sel)[:8] == alvo["id"][:8]
        and str(sel2)[:8] == outro["id"][:8]
        and visivel_antes
        and visivel_depois != visivel_antes
        and visivel_volta
    )
    diz("RESULTADO: " + ("OK" if ok else "FALHOU"))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
