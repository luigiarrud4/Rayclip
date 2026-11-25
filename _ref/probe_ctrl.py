"""Sonda: o Ctrl chega no WebView como `ctrlKey` durante o clique?

Instala um listener de captura no `document` via CDP, clica numa linha, aperta
Ctrl (SendInput), clica em outra linha e lê o que o navegador viu.
"""
import ctypes
import io
import json
import os
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import entrada
import harness_compare as h
import keysafe
import websocket

user32 = ctypes.windll.user32
SAIDA = os.path.join(os.environ["TEMP"], "probe_ctrl.txt")
linhas = []


def diz(m):
    linhas.append(str(m))
    print(m, flush=True)


def cdp():
    alvo = json.load(urllib.request.urlopen("http://127.0.0.1:9222/json/list", timeout=10))[0]
    ws = websocket.create_connection(
        "ws://127.0.0.1:9222/devtools/page/" + alvo["id"], suppress_origin=True, timeout=30
    )
    estado = {"n": 0}

    def ev(js):
        estado["n"] += 1
        ws.send(
            json.dumps(
                {
                    "id": estado["n"],
                    "method": "Runtime.evaluate",
                    "params": {"expression": js, "awaitPromise": True, "returnByValue": True},
                }
            )
        )
        while True:
            r = json.loads(ws.recv())
            if r.get("id") == estado["n"]:
                return r["result"].get("result", {}).get("value")

    return ev


def main():
    hwnd = None
    for _ in range(200):
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            break
        time.sleep(0.1)
    if not hwnd:
        diz("app invisivel")
        return
    diz(f"focar: {keysafe.focar(hwnd, espera=1.0)}")
    ev = cdp()
    for _ in range(40):
        if ev("document.querySelectorAll('.row').length"):
            break
        time.sleep(0.25)
    ev(
        "(() => { window.__ev=[];"
        " ['mousedown','mouseup','click','keydown','keyup'].forEach(t =>"
        "   document.addEventListener(t, e => window.__ev.push([t, e.ctrlKey, e.shiftKey, (e.target.className||'').slice(0,12)]), true));"
        " return 'ok'; })()"
    )
    pos = json.loads(ev("JSON.stringify({x:window.screenX, y:window.screenY})"))
    med = json.loads(
        ev(
            "JSON.stringify([...document.querySelectorAll('.row')].slice(0,3).map(e => {"
            " const b=e.getBoundingClientRect();"
            " return {id:e.dataset.id, x:b.x+110, y:b.y+b.height/2}; }))"
        )
    )
    entrada.clicar(int(pos["x"] + med[0]["x"]), int(pos["y"] + med[0]["y"]))
    time.sleep(0.4)
    diz(f"pid em foco: {keysafe.pid_do_foco()}")
    entrada.tecla(0x11, cima=False)
    time.sleep(0.15)
    diz(f"ctrl apertado, estado={bool(user32.GetAsyncKeyState(0x11) & 0x8000)}")
    entrada.clicar(int(pos["x"] + med[1]["x"]), int(pos["y"] + med[1]["y"]))
    time.sleep(0.4)
    entrada.tecla(0x11, cima=True)
    time.sleep(0.3)
    diz("eventos vistos pelo WebView: " + str(ev("JSON.stringify(window.__ev)")))
    diz("selecionadas: " + str(ev("document.querySelectorAll('.row[data-selected=\"true\"]').length")))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
