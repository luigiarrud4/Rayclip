"""Diagnóstico: a janela do app fica na frente o suficiente para um clique real?

Antes de clicar (nunca clicar às cegas: na sessão anterior os cliques caíram no
Discord) é preciso saber se o app sustenta o foco e o z-order por alguns ms.
"""

import ctypes
import time
from ctypes import wintypes as w

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
u32.WindowFromPoint.argtypes = [w.POINT]
u32.WindowFromPoint.restype = w.HWND
u32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]


def nome_janela(hwnd):
    if not hwnd:
        return "-"
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    try:
        return h.proc_name(pid.value)
    except Exception:
        return "?"


def topo(pt):
    sob = u32.WindowFromPoint(w.POINT(pt[0], pt[1]))
    return u32.GetAncestor(sob, 2)


if __name__ == "__main__":
    hwnd = p.mostrar()
    rect = p.retangulo(hwnd)
    ponto = (rect.left + 150, rect.top + 159)
    print("app:", hwnd, "| ponto:", ponto)
    print("focar:", k.focar(hwnd))
    print("t   foco          visivel topo-no-ponto  raiz==app")
    for i in range(20):
        fg = u32.GetForegroundWindow()
        raiz = topo(ponto)
        print(
            f"{i*50:4d}ms {nome_janela(fg):<12} {str(bool(u32.IsWindowVisible(hwnd))):<7} "
            f"{nome_janela(raiz):<14} {raiz == hwnd}"
        )
        time.sleep(0.05)
