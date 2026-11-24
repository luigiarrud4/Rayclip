"""Relação das janelas do Rayclip: qual é a de topo, qual é o WebView2, e qual
`GetAncestor(GA_ROOT)` devolve para cada uma.

É isso que o `click_outside` compara (`root_of(target) == main_hwnd`) quando
decide se o clique foi dentro ou fora. Se a raiz do Chrome_WidgetWin_1 não for a
janela de topo, todo clique dentro do app esconde a janela.
"""

import ctypes
import io
import os
import subprocess
import time
from ctypes import wintypes as w

import harness_compare as h

u32 = ctypes.windll.user32
u32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
u32.GetAncestor.restype = w.HWND
u32.GetParent.argtypes = [w.HWND]
u32.GetParent.restype = w.HWND
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]

EXE = os.path.join(os.environ["LOCALAPPDATA"], "Rayclip", "rayclip.exe")
SAIDA = os.path.join(os.environ["TEMP"], "janelas_rayclip.txt")
linhas = []
CB = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def diz(m):
    linhas.append(m)


def classe(hwnd):
    b = ctypes.create_unicode_buffer(128)
    u32.GetClassNameW(hwnd, b, 128)
    return b.value


def retangulo(hwnd):
    r = w.RECT()
    u32.GetWindowRect(hwnd, ctypes.byref(r))
    return (r.left, r.top, r.right, r.bottom)


def main():
    hwnd_topo = h.find_window("rayclip.exe")
    if not hwnd_topo:
        subprocess.Popen(
            [EXE], creationflags=0x08000008,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        for _ in range(12):
            time.sleep(0.5)
            hwnd_topo = h.find_window("rayclip.exe")
            if hwnd_topo:
                break
    if not hwnd_topo:
        diz("janela nao encontrada")
        return
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd_topo, ctypes.byref(pid))
    diz(f"janela visivel de topo: 0x{hwnd_topo:x} pid={pid.value} classe={classe(hwnd_topo)}")
    diz(f"  retangulo {retangulo(hwnd_topo)}")

    achados = []

    def cb(hwnd, _l):
        p2 = w.DWORD(0)
        u32.GetWindowThreadProcessId(hwnd, ctypes.byref(p2))
        if p2.value != pid.value:
            return True
        raiz = u32.GetAncestor(hwnd, 2)
        achados.append(
            (
                hwnd,
                classe(hwnd),
                u32.GetParent(hwnd) or 0,
                raiz or 0,
                bool(u32.IsWindowVisible(hwnd)),
            )
        )
        return True

    u32.EnumWindows(CB(cb), 0)
    for hwnd, cls, pai, raiz, vis in achados:
        diz(
            f"0x{hwnd:x} {cls:<28} pai=0x{pai:x} raiz=0x{raiz:x} "
            f"visivel={vis} raiz==topo:{raiz == hwnd_topo}"
        )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
        print("\n".join(linhas))
