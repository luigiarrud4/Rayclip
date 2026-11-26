"""A faixa de título volta quando a janela é ativada **de fora** (tray).

O clique no ícone da bandeja faz o Windows ativar a janela por conta própria.
Aqui a janela é escondida e reaberta por `ShowWindow` + `SetForegroundWindow`
vindos de outro processo — o mesmo tipo de ativação que a bandeja produz —, e
depois medimos `janela − cliente` (a faixa) e o `DWMWA_NCRENDERING_ENABLED`.
"""

import ctypes
import subprocess
import sys
import time
from ctypes import wintypes as w

sys.path.insert(0, "_ref")
import harness_compare as h

user32 = ctypes.windll.user32
dwmapi = ctypes.windll.dwmapi
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]

DWMWA_NCRENDERING_ENABLED = 1


def medidas(hwnd):
    jan, cli = w.RECT(), w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(jan))
    user32.GetClientRect(hwnd, ctypes.byref(cli))
    faixa = (jan.bottom - jan.top) - (cli.bottom - cli.top), (jan.right - jan.left) - (cli.right - cli.left)
    v = ctypes.c_int()
    dwmapi.DwmGetWindowAttribute(hwnd, DWMWA_NCRENDERING_ENABLED, ctypes.byref(v), ctypes.sizeof(v))
    return faixa, bool(v.value)


def cauda(n=12):
    import os

    p = os.path.join(os.environ["TEMP"], "rayclip.log")
    try:
        with open(p, "r", encoding="utf-8", errors="replace") as f:
            return [l.rstrip() for l in f.readlines()[-n:]]
    except OSError:
        return ["(sem log)"]


def main():
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        raise SystemExit("rayclip nao esta aberto")
    print("inicio       :", *medidas(hwnd))

    # 1) esconde como o app faz ao perder o foco
    user32.ShowWindow(hwnd, 0)
    time.sleep(0.6)
    print("escondida    :", *medidas(hwnd))

    # 2) a bandeja reabre: ShowWindow + ativacao vindos de fora
    user32.ShowWindow(hwnd, 5)  # SW_SHOW
    time.sleep(0.15)
    user32.SetForegroundWindow(hwnd)
    time.sleep(1.2)
    print("reaberta     :", *medidas(hwnd))

    # 3) e uma segunda ativacao, como o segundo clique na bandeja
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.8)
    print("re-ativada   :", *medidas(hwnd))

    print("--- log ---")
    for l in cauda():
        print(" ", l)
    return 0


if __name__ == "__main__":
    sys.exit(main())
