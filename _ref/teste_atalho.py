"""Atalho global de verdade (Alt+Space) e o topo da janela.

Manda o atalho com entrada sintética — sem tocar em `ShowWindow` —, mede a faixa
`janela − cliente`, conta os pixels claros das primeiras 26 linhas (a legenda
clássica desenha ícone e título ali) e guarda a captura do topo.
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import entrada  # noqa: E402
import teste_tray as T  # noqa: E402
import teste_tray_clique as C  # noqa: E402

u = ctypes.windll.user32
u.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
VK_MENU, VK_SPACE, VK_ESCAPE = 0x12, 0x20, 0x1B


def nc_render(hwnd):
    """`DWMWA_NCRENDERING_ENABLED`: True é o estado em que o DWM desenha a legenda."""
    v = ctypes.c_int()
    ctypes.windll.dwmapi.DwmGetWindowAttribute(hwnd, 1, ctypes.byref(v), ctypes.sizeof(v))
    return bool(v.value)


def topo(hwnd, altura=26):
    r = w.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(r))
    im = ImageGrab.grab(bbox=(r.left, r.top, r.right, r.top + altura)).convert("L")
    return sum(1 for p in im.getdata() if p > 140)


def captura(hwnd, nome):
    r = w.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(r))
    ImageGrab.grab(bbox=(r.left, r.top, r.right, r.top + 95)).save(
        rf"C:\Windows\System32\_rayclip_tmp\{nome}.png"
    )


def abrir_atalho():
    entrada.tecla(VK_MENU, cima=False)
    entrada.tecla(VK_SPACE, cima=False)
    entrada.tecla(VK_SPACE, cima=True)
    entrada.tecla(VK_MENU, cima=True)


def main():
    hwnd = C.achar()
    if not hwnd:
        raise SystemExit("rayclip nao esta aberto")
    # o terminal está em primeiro plano aqui: é o estado real do atalho
    print("estado inicial:", T.medidas(hwnd), "visivel:", bool(u.IsWindowVisible(hwnd)))
    for volta in range(1, 7):
        abrir_atalho()
        time.sleep(1.6)
        visivel = bool(u.IsWindowVisible(hwnd))
        faixa, _ = T.medidas(hwnd)
        claro = topo(hwnd) if visivel else -1
        print(
            f"volta {volta}: visivel={visivel} faixa={faixa} nc_render={nc_render(hwnd)} "
            f"topo_claro={claro}"
        )
        if visivel:
            captura(hwnd, f"atalho_{volta}")
        entrada.tecla(VK_ESCAPE)
        time.sleep(1.2)
    print("--- log ---")
    for l in T.cauda(12):
        print(" ", l)
    return 0


if __name__ == "__main__":
    sys.exit(main())
