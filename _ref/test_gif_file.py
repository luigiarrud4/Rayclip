"""GIF copiado como ARQUIVO (CF_HDROP): o painel de detalhes anima?

Para o Rayclip: acha a entrada pelo titulo (nome do arquivo), rola a lista ate ela
aparecer, clica pela posicao real (sem teclado) e compara dois quadros.
Resultado em `_ref/gif5_resultado.txt`.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
from test_gif2 import copiar_gif, criar_gif  # noqa: E402
from test_gif4 import (cdp_js, cdp_sessao, clique_cdp, dois_prints,  # noqa: E402
                       janela_classe, janela_grande, pid_de)

REL = os.path.dirname(os.path.abspath(__file__))
LOG = os.path.join(REL, "gif5_resultado.txt")


def linha(txt):
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(txt + "\n")


def main():
    if os.path.exists(LOG):
        os.remove(LOG)

    # copia o GIF como arquivo (CF_HDROP), igual a copiar no Explorador
    gif, _ = criar_gif()
    subprocess_ok = False
    import subprocess
    r = subprocess.run(["powershell", "-NoProfile", "-Command",
                        f"Set-Clipboard -Path '{gif}'"], capture_output=True)
    linha(f"gif como arquivo: {os.path.basename(gif)} copiado (rc={r.returncode})")
    time.sleep(2.2)

    pid = pid_de("rayclip")
    if not pid:
        linha("rayclip: processo nao encontrado")
        return
    neutra = janela_classe("RayclipHarnessAlvo")
    if neutra and keysafe.focar(neutra):
        h.send_keys("alt", "space")
        time.sleep(1.6)
    jan = janela_grande(pid)
    if not jan:
        linha("rayclip: janela nao apareceu")
        return

    ws = cdp_sessao()
    nome = os.path.basename(gif)
    info = json.loads(cdp_js(ws, "(async () => { const {invoke} = window.__TAURI_INTERNALS__;"
                                 " const l = await invoke('list_entries');"
                                 " const e = l.find(x => (x.title||'').includes('%s'));"
                                 " return JSON.stringify({id: e ? e.id : null, kind: e ? e.kind : null}); })()"
                            % nome))
    linha(f"rayclip: entrada do arquivo {info}")
    if not info.get("id"):
        return
    gid = info["id"]

    cdp_js(ws, "(() => { const i = document.querySelector('input');"
               " const set = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value').set;"
               " set.call(i, '%s'); i.dispatchEvent(new Event('input', {bubbles: true})); return 1; })()" % nome)
    time.sleep(1.0)
    rolagem = cdp_js(ws, """(async () => {
      const alvo = '[data-id="%s"]';
      if (document.querySelector(alvo)) return 'ja visivel';
      const cont = [...document.querySelectorAll('div')].find(d =>
        d.scrollHeight > d.clientHeight + 50 && d.querySelector('.row'));
      if (!cont) return 'sem container';
      for (let i = 0; i < 40; i++) {
        cont.scrollTop += Math.max(120, cont.clientHeight * 0.8);
        await new Promise(r => setTimeout(r, 60));
        if (document.querySelector(alvo)) return 'achou';
      }
      return 'nao achou';
    })()""" % gid)
    linha(f"rayclip: rolagem {rolagem}")
    pos = cdp_js(ws, "(() => { const r = document.querySelector('[data-id=\"%s\"]');"
                     " if (!r) return 'null'; const b = r.getBoundingClientRect();"
                     " return JSON.stringify({x: b.x + b.width/2, y: b.y + b.height/2}); })()" % gid)
    if pos in (None, "null"):
        linha("rayclip: linha nao esta na tela")
        return
    p = json.loads(pos)
    clique_cdp(ws, p["x"], p["y"])
    time.sleep(1.0)
    sel = cdp_js(ws, "JSON.stringify([...document.querySelectorAll('.row')]"
                     ".filter(r => r.dataset.selected === 'true').map(r => r.dataset.id))")
    linha(f"rayclip: selecionado {sel}")
    linha(f"rayclip: {dois_prints('rayclip', 'file', jan[2])}")


if __name__ == "__main__":
    import traceback
    try:
        main()
    except Exception:
        linha("EXCECAO: " + traceback.format_exc())
