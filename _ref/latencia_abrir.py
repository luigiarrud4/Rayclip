"""Latência de abrir a janela, do key DOWN até a janela ficar visível.

Compensa o custo do `sem_moldura` que passou a rodar no `show_window`.
Uso: python _ref/latencia_abrir.py <rotulo>
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

sys.path.insert(0, "_ref")
import entrada as e  # noqa: E402
import harness_compare as h  # noqa: E402

user32 = ctypes.windll.user32
user32.IsWindowVisible.argtypes = [w.HWND]
kernel32 = ctypes.windll.kernel32
kernel32.QueryPerformanceCounter.argtypes = [ctypes.POINTER(ctypes.c_longlong)]
kernel32.QueryPerformanceFrequency.argtypes = [ctypes.POINTER(ctypes.c_longlong)]


def agora():
    c = ctypes.c_longlong()
    kernel32.QueryPerformanceCounter(ctypes.byref(c))
    return c.value


def kd(vk):
    e._enviar(e.INPUT(type=1, u=e._UNIAO(ki=e.KEYBDINPUT(vk, 0, 0, 0, 0))))


def ku(vk):
    e._enviar(e.INPUT(type=1, u=e._UNIAO(ki=e.KEYBDINPUT(vk, 0, 0x0002, 0, 0))))


def atalho():
    kd(0xA4)
    kd(0x20)
    ku(0x20)
    ku(0xA4)


def main():
    rotulo = sys.argv[1] if len(sys.argv) > 1 else "build"
    f = ctypes.c_longlong()
    kernel32.QueryPerformanceFrequency(ctypes.byref(f))
    freq = f.value
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        print("janela nao esta na tela")
        return 1
    amostras = []
    for _ in range(6):
        # esconde
        atalho()
        time.sleep(1.2)
        if user32.IsWindowVisible(hwnd):
            print("nao escondeu")
            return 1
        t0 = agora()
        atalho()  # key down do atalho de mostrar
        while not user32.IsWindowVisible(hwnd):
            if (agora() - t0) / freq > 3:
                print("nao mostrou em 3 s")
                return 1
        ms = (agora() - t0) * 1000.0 / freq
        amostras.append(ms)
        time.sleep(0.6)
    amostras.sort()
    mediana = amostras[len(amostras) // 2]
    print(f"{rotulo}: " + " ".join(f"{a:.1f}" for a in amostras) + f" ms  mediana={mediana:.1f} ms")
    return 0


if __name__ == "__main__":
    sys.exit(main())
