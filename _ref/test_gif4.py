"""GIF anima no painel de detalhes? Rayclip (sem teclado, via CDP) e Raycast.

Rayclip: a janela e mostrada por `ShowWindow` e a linha do GIF e clicada por
`Input.dispatchMouseEvent` — nenhuma tecla sintetica, entao nada pode cair no
console do agente.

Raycast: precisa do atalho global. Aqui ele vai com trava de foco
(`keysafe.teclas_seguro`): se o foco nao for do Raycast, nada e digitado.

Resultado vai para `_ref/gif4_resultado.txt`.
"""
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
import websocket  # noqa: E402
from PIL import Image, ImageChops, ImageGrab  # noqa: E402
from test_pm_raycast import janelas  # noqa: E402

SAIDA = r"C:\Windows\System32\_rayclip_tmp"
REL = os.path.dirname(os.path.abspath(__file__))
LOG = os.path.join(REL, "gif4_resultado.txt")


def linha(txt):
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(txt + "\n")


def pid_de(nome):
    p = subprocess.run(["powershell", "-NoProfile", "-Command",
                        f"(Get-Process {nome} -ErrorAction SilentlyContinue).Id"],
                       capture_output=True, text=True)
    m = re.search(r"\d{2,}", p.stdout or "")
    return int(m.group(0)) if m else None


def janela_grande(pid, espera=2.5, visiveis=True):
    """Maior janela do processo. `visiveis=False` inclui a janela escondida
    (o Rayclip fica oculto ate o atalho; `janelas()` so lista as visiveis)."""
    fim = time.time() + espera
    while time.time() < fim:
        cands = [c for c in janelas(pid, so_visiveis=visiveis) if c[0] > 20000]
        if cands:
            return cands[0]
        time.sleep(0.1)
    return None


def janela_classe(nome_classe):
    """Acha a janela do helper do harness (janela neutra, nossa)."""
    import ctypes
    from ctypes import wintypes as w
    u = ctypes.WinDLL("user32")
    achado = []
    Cb = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

    def cb(hwnd, _l):
        b = ctypes.create_unicode_buffer(128)
        u.GetClassNameW(hwnd, b, 128)
        if b.value == nome_classe:
            achado.append(hwnd)
        return True

    u.EnumWindows(Cb(cb), 0)
    return achado[0] if achado else None


def analisa(a, b):
    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    d = ImageChops.difference(ia, ib)
    w, _ = d.size
    fortes = [(i % w, i // w) for i, p in enumerate(d.getdata()) if sum(p) > 90]
    if not fortes:
        return 0, None
    xs = [q[0] for q in fortes]
    ys = [q[1] for q in fortes]
    return len(fortes), (min(xs), min(ys), max(xs), max(ys))


def dois_prints(app, marca, rect):
    a = os.path.join(SAIDA, f"gif4_{app}_{marca}_1.png")
    b = os.path.join(SAIDA, f"gif4_{app}_{marca}_2.png")
    ImageGrab.grab(bbox=rect).save(a)
    time.sleep(0.40)
    ImageGrab.grab(bbox=rect).save(b)
    return analisa(a, b)


def cdp_sessao():
    import urllib.request
    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=5) as r:
        url = [t for t in json.load(r) if t.get("type") == "page"][0]["webSocketDebuggerUrl"]
    return websocket.create_connection(url, timeout=40, suppress_origin=True)


def cdp_js(ws, expr):
    ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                        "params": {"expression": expr, "awaitPromise": True, "returnByValue": True}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == 1:
            return m["result"]["result"].get("value")


def clique_cdp(ws, x, y):
    for tipo in ("mousePressed", "mouseReleased"):
        ws.send(json.dumps({"id": 9, "method": "Input.dispatchMouseEvent",
                            "params": {"type": tipo, "x": x, "y": y, "button": "left",
                                       "clickCount": 1}}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 9:
                break


def testa_rayclip():
    pid = pid_de("rayclip")
    if not pid:
        linha("rayclip: processo nao encontrado")
        return
    ws = cdp_sessao()
    info = json.loads(cdp_js(ws, "(async () => { const {invoke} = window.__TAURI_INTERNALS__;"
                                 " const l = await invoke('list_entries');"
                                 " const g = l.find(e => e.gif);"
                                 " return JSON.stringify({id: g ? g.id : null}); })()"))
    if not info["id"]:
        linha("rayclip: nenhum GIF no historico")
        return
    gid = info["id"]
    # mostra a janela pelo atalho global, com o foco na janela neutra do harness
    neutra = janela_classe("RayclipHarnessAlvo")
    if not neutra or not keysafe.focar(neutra):
        linha("rayclip: janela neutra indisponivel — janela nao foi aberta")
        return
    h.send_keys("alt", "space")   # vai para a neutra (menu do sistema) e abre o Rayclip
    time.sleep(1.6)
    jan = janela_grande(pid)
    if not jan:
        linha("rayclip: janela nao apareceu")
        return
    # A lista e virtualizada: a linha so existe no DOM quando esta renderizada.
    # Nao da para filtrar por texto (imagem nao tem titulo no banco), entao
    # limpamos a busca e rolamos ate a linha aparecer.
    limpa = cdp_js(ws, "(() => { const i = document.querySelector('input');"
                       " if (!i.value) return 'vazio';"
                       " const set = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value').set;"
                       " set.call(i, ''); i.dispatchEvent(new Event('input', {bubbles: true}));"
                       " return 'limpou'; })()")
    time.sleep(0.6)
    rolagem = cdp_js(ws, """(async () => {
      const alvo = '[data-id="%s"]';
      if (document.querySelector(alvo)) return 'ja visivel';
      const cont = [...document.querySelectorAll('div')].find(d =>
        d.scrollHeight > d.clientHeight + 50 && d.querySelector('.row'));
      if (!cont) return 'sem container de rolagem';
      for (let i = 0; i < 40; i++) {
        cont.scrollTop += Math.max(120, cont.clientHeight * 0.8);
        await new Promise(r => setTimeout(r, 60));
        if (document.querySelector(alvo)) return 'achou em ' + Math.round(cont.scrollTop);
      }
      return 'nao achou';
    })()""" % gid)
    linha(f"rayclip: busca {limpa}, rolagem {rolagem}")
    # clica na linha do GIF pela posicao real (sem teclado)
    pos = cdp_js(ws, "(() => { const r = document.querySelector('[data-id=\"%s\"]');"
                     " if (!r) return 'null'; const b = r.getBoundingClientRect();"
                     " return JSON.stringify({x: b.x + b.width/2, y: b.y + b.height/2}); })()" % gid)
    if pos in (None, "null"):
        linha("rayclip: linha do GIF nao esta na tela")
        return
    p = json.loads(pos)
    clique_cdp(ws, p["x"], p["y"])
    time.sleep(0.9)
    sel = cdp_js(ws, "JSON.stringify([...document.querySelectorAll('.row')]"
                     ".filter(r => r.dataset.selected === 'true').map(r => r.dataset.id))")
    linha(f"rayclip: selecionado {sel} (gif {gid[:8]})")
    if gid not in (sel or ""):
        linha("rayclip: selecao nao e o GIF — medida ignorada")
        return
    linha(f"rayclip: {dois_prints('rayclip', 'gif', jan[2])}")


def testa_raycast():
    pid = pid_de("raycast")
    if not pid:
        linha("raycast: processo nao encontrado")
        return
    # atalho global: enviado com o foco na janela do helper do harness (nossa e
    # neutra). Ctrl+Shift+V ali e um "colar" inofensivo; nunca vai para o console.
    alvo = janela_classe("RayclipHarnessAlvo")
    if not alvo:
        linha("raycast: helper do harness nao esta rodando — teclas nao enviadas")
        return
    if not keysafe.focar(alvo, espera=0.6):
        linha("raycast: nao consegui focar a janela neutra — teclas nao enviadas")
        return
    try:
        h.send_keys("ctrl", "shift", "v")
    except RuntimeError as e:
        linha(f"raycast: atalho NAO enviado — {e}")
        return
    time.sleep(1.8)
    jan = janela_grande(pid)
    if not jan:
        linha("raycast: janela nao abriu")
        return
    linha(f"raycast: {dois_prints('raycast', 'gif', jan[2])}")


def main():
    if os.path.exists(LOG):
        os.remove(LOG)
    for f in (testa_rayclip, testa_raycast):
        try:
            f()
        except Exception as e:
            linha(f"{f.__name__}: erro {e!r}")


if __name__ == "__main__":
    main()
