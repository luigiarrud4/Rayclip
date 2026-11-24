"""Colagem: 3 rodadas de 5 por app, para ver se a diferença de 2 ms se mantém.

Escreve em `_ref/colagem_resultado.txt` (uma linha por rodada) e imprime a
mediana geral. Usa a trava de foco do próprio harness.
"""
import os
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness_compare as h  # noqa: E402

LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "colagem_resultado.txt")


def diz(txt):
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(txt + "\n")
    print(txt, flush=True)


def main():
    if os.path.exists(LOG):
        os.remove(LOG)
    alvo = h.Alvo()
    h.ALVO_HWND = alvo.hwnd
    alvo.focus()
    diz(f"colagem — {time.strftime('%d/%m %H:%M')}")
    tudo = {}
    for r in range(3):
        for name in h.APPS:
            res = h.paste_latency(alvo, name)
            vals = [x[0] for x in res if x[1]]
            tudo.setdefault(name, []).extend(vals)
            med = round(statistics.median(vals), 1) if vals else None
            diz(f"  rodada {r + 1} {name:8s} mediana={med}  {[round(v) for v in vals]}"
                f"  ok={len(vals)}/{len(res)}")
    for name, vals in tudo.items():
        diz(f"  TOTAL {name:8s} mediana={round(statistics.median(vals), 1)}"
            f"  min={round(min(vals))} max={round(max(vals))} n={len(vals)}")
    for name in h.APPS:
        h.close_app(name, alvo)


if __name__ == "__main__":
    main()
