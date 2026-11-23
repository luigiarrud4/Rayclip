"""Anima ou nao o GIF no painel de detalhes do Rayclip (e do Raycast).

Roda em segundo plano e grava o resultado em `_ref/gif3_resultado.txt`, porque o
console desta sessao tem derrubado comandos longos.
"""
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
from PIL import Image, ImageChops, ImageGrab  # noqa: E402
from test_img import cdp  # noqa: E402
from test_pm_raycast import janelas  # noqa: E402

SAIDA = r"C:\Windows\System32\_rayclip_tmp"
REL = os.path.dirname(os.path.abspath(__file__))


def rect(app):
    p = subprocess.run(["powershell", "-NoProfile", "-Command",
                        f"(Get-Process {app} -ErrorAction SilentlyContinue).Id"],
                       capture_output=True, text=True)
    m = re.search(r"\d{2,}", p.stdout or "")
    if not m:
        return None
    pid = int(m.group(0))
    for _ in range(25):
        cands = [c for c in janelas(pid) if c[0] > 20000]
        if cands:
            return cands[0][2]
        time.sleep(0.1)
    return None


def analisa(a, b):
    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    d = ImageChops.difference(ia, ib)
    w, h = d.size
    px = list(d.getdata())
    fortes = [(i % w, i // w) for i, p in enumerate(px) if sum(p) > 90]
    if not fortes:
        return 0, None
    xs = [q[0] for q in fortes]
    ys = [q[1] for q in fortes]
    return len(fortes), (min(xs), min(ys), max(xs), max(ys))


def dois_prints(app, marca):
    r = rect(app)
    if not r:
        return None
    a = os.path.join(SAIDA, f"gif3_{app}_{marca}_1.png")
    b = os.path.join(SAIDA, f"gif3_{app}_{marca}_2.png")
    ImageGrab.grab(bbox=r).save(a)
    time.sleep(0.40)
    ImageGrab.grab(bbox=r).save(b)
    return analisa(a, b)


def linha(txt):
    with open(os.path.join(REL, "gif3_resultado.txt"), "a", encoding="utf-8") as f:
        f.write(txt + "\n")
    print(txt, flush=True)


def testa_rayclip():
    novo = cdp("(async () => { const {invoke} = window.__TAURI_INTERNALS__;"
               " const l = await invoke('list_entries');"
               " const g = l.find(e => e.gif);"
               " return JSON.stringify({id: g ? g.id : null, total: l.length}); })()")
    info = json.loads(novo)
    if not info["id"]:
        linha("rayclip: nenhuma entrada de GIF no historico")
        return
    alvo = info["id"]
    linha(f"rayclip: gif id {alvo[:8]}")
    h.send_keys("esc")
    time.sleep(0.4)
    h.send_keys("alt", "space")
    time.sleep(1.7)
    atual = []
    for _ in range(8):
        sel = cdp("JSON.stringify([...document.querySelectorAll('.row')]"
                  ".filter(r => r.dataset.selected === 'true').map(r => r.dataset.id))")
        atual = json.loads(sel or "[]")
        if atual and atual[0] == alvo:
            break
        h.send_keys("down")
        time.sleep(0.3)
    linha(f"rayclip: selecionado {atual[0][:8] if atual else '—'}")
    r = dois_prints("rayclip", "gif")
    linha(f"rayclip: {r}")


def main():
    aberto = os.path.join(REL, "gif3_resultado.txt")
    if os.path.exists(aberto):
        os.remove(aberto)
    try:
        testa_rayclip()
        h.send_keys("esc")
        time.sleep(0.5)
        # Raycast: a entrada mais recente (o GIF) fica no topo, sem busca
        h.send_keys("ctrl", "shift", "v")
        time.sleep(1.8)
        r = dois_prints("raycast", "gif")
        linha(f"raycast: {r}")
    except Exception as e:
        linha(f"erro: {e!r}")


if __name__ == "__main__":
    main()
