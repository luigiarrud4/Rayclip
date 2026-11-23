"""Raycast com GIF vindo do clipboard (formato cru "GIF"), recem copiado.

Compara dois quadros da janela do Raycast 400 ms depois de abrir: se o preview
mexer, ele anima. Teclas so com trava de foco (keysafe).
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
from test_gif2 import copiar_gif, criar_gif  # noqa: E402
from test_gif4 import analisa, dois_prints, janela_classe, janela_grande, linha, LOG  # noqa: E402


def main():
    if os.path.exists(LOG):
        os.remove(LOG)
    gif, primeiro = criar_gif()
    copiar_gif(gif, primeiro)
    linha(f"gif copiado: {os.path.basename(gif)}")
    time.sleep(2.2)

    pid = None
    p = subprocess.run(["powershell", "-NoProfile", "-Command",
                        "(Get-Process raycast -ErrorAction SilentlyContinue).Id"],
                       capture_output=True, text=True)
    m = re.search(r"\d{2,}", p.stdout or "")
    if not m:
        linha("raycast: processo nao encontrado")
        return
    pid = int(m.group(0))

    neutra = janela_classe("RayclipHarnessAlvo")
    if not neutra:
        linha("raycast: janela neutra do harness nao existe")
        return
    if not keysafe.focar(neutra):
        linha("raycast: nao consegui focar a janela neutra")
        return
    h.send_keys("ctrl", "shift", "v")
    time.sleep(1.9)
    jan = janela_grande(pid)
    if not jan:
        linha("raycast: janela nao abriu")
        return
    # a selecao inicial nao e a primeira linha: sobe ate a entrada de imagem
    # (o GIF, recem-copiado) — teclas com foco confirmado no Raycast
    try:
        keysafe.teclas_seguro(pid, h.send_keys, "up")
        time.sleep(0.5)
    except RuntimeError as e:
        linha(f"raycast: seta NAO enviada — {e}")
    linha(f"raycast: {dois_prints('raycast', 'gif5', jan[2])}")


if __name__ == "__main__":
    main()
