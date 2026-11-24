"""Clique real na build INSTALADA — com oráculo no painel de detalhes.

Mesma mecânica do `clique_instalada.py` (pythonw, espera o console sossegar,
foca, guarda de `WindowFromPoint`, clique), mas comparando **só a coluna de
detalhes** (lado direito) e salvando uma foto por clique, para conferir depois
qual linha ficou selecionada.

Ruído zero é o esperado entre duas fotos da janela parada; qualquer mudança no
painel direito significa que a seleção trocou (o painel mostra título e texto da
entrada selecionada).
"""

import ctypes
import io
import os
import sys
import time
from ctypes import wintypes as w

import PIL.Image
import PIL.ImageChops

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
u32.WindowFromPoint.argtypes = [w.POINT]
u32.WindowFromPoint.restype = w.HWND
u32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]

TMP = p.TMP
SAIDA = os.path.join(TMP, "clique_resultado.txt")
linhas = []


def diz(msg):
    linhas.append(msg)
    if sys.stdout:
        print(msg)


def nome_de(hwnd):
    if not hwnd:
        return "-"
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    try:
        return h.proc_name(pid.value)
    except Exception:
        return "?"


def topo_no_ponto(x, y):
    return u32.GetAncestor(u32.WindowFromPoint(w.POINT(x, y)), 2)


def painel(img):
    """Recorte da coluna de detalhes (direita)."""
    larg, alt = img.size
    return img.crop((int(larg * 0.58), int(alt * 0.12), larg, alt))


def diff_painel(a, b):
    d = PIL.ImageChops.difference(painel(a), painel(b)).convert("L")
    return sum(1 for v in d.get_flattened_data() if v > 12)


def foca_e_clica(hwnd, x, y):
    k.focar(hwnd, espera=0.3)
    if u32.GetForegroundWindow() != hwnd or not u32.IsWindowVisible(hwnd):
        return "sem foco"
    dono = topo_no_ponto(x, y)
    if dono != hwnd:
        return f"por cima esta {nome_de(dono)}"
    p.clicar(x, y)
    time.sleep(0.45)
    if not u32.IsWindowVisible(hwnd):
        return "escondeu"
    return "ok"


def main():
    time.sleep(4)
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        p.mostrar()
        hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela nao encontrada")
        return
    rect = p.retangulo(hwnd)
    diz(f"janela {hwnd} em ({rect.left},{rect.top})-({rect.right},{rect.bottom})")
    k.focar(hwnd, espera=0.4)
    time.sleep(0.4)

    zero = p.foto(hwnd, rect, f"{TMP}/c_zero.png")
    time.sleep(0.5)
    parado = p.foto(hwnd, rect, f"{TMP}/c_parado.png")
    diz(f"ruido com a janela parada: {p.diff(zero, parado)} px (esperado 0)")

    atual = parado
    for i, (rotulo, off) in enumerate((("2a linha", 179), ("4a linha", 219), ("6a linha", 259)), 1):
        estado = foca_e_clica(hwnd, rect.left + 150, rect.top + off)
        nova = p.foto(hwnd, rect, f"{TMP}/c_{i}.png")
        diz(
            f"clique {rotulo:<9} y+{off}: {estado:<12} "
            f"painel mudou = {diff_painel(atual, nova):>6} px | janela = {p.diff(atual, nova):>6} px"
        )
        atual = nova


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
