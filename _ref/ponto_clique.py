"""Quem está no ponto do clique e qual a raiz dessa janela?

`click_outside` esconde quando `root_of(target) != main_hwnd`. Aqui a gente vê
exatamente o que o `WindowFromPoint` devolve no meio da janela do Rayclip e sobe
a cadeia de pais, para saber se o teste de raiz do app bate.
"""

import ctypes
import io
import os
from ctypes import wintypes as w

import harness_compare as h

u32 = ctypes.windll.user32
u32.WindowFromPoint.argtypes = [w.POINT]
u32.WindowFromPoint.restype = w.HWND
u32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
u32.GetAncestor.restype = w.HWND
u32.GetParent.argtypes = [w.HWND]
u32.GetParent.restype = w.HWND
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]

SAIDA = os.path.join(os.environ["TEMP"], "ponto_clique.txt")
linhas = []


def classe(hwnd):
    b = ctypes.create_unicode_buffer(128)
    u32.GetClassNameW(hwnd, b, 128)
    return b.value


def dono(hwnd):
    p = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
    try:
        return f"{h.proc_name(p.value)}({p.value})"
    except Exception:
        return f"?({p.value})"


def main():
    topo = h.find_window("rayclip.exe")
    if not topo:
        linhas.append("janela nao encontrada")
        return
    r = w.RECT()
    u32.GetWindowRect(topo, ctypes.byref(r))
    linhas.append(f"topo 0x{topo:x} {classe(topo)} {dono(topo)}")
    for dy in (179, 219, 300):
        pt = w.POINT(r.left + 150, r.top + dy)
        alvo = u32.WindowFromPoint(pt)
        linhas.append(f"\nponto ({pt.x},{pt.y}) -> 0x{alvo:x} {classe(alvo)} {dono(alvo)}")
        raiz = u32.GetAncestor(alvo, 2)
        linhas.append(f"  GA_ROOT = 0x{raiz:x} {classe(raiz) if raiz else '-'} raiz==topo:{raiz == topo}")
        atual = alvo
        for _ in range(6):
            pai = u32.GetParent(atual)
            if not pai:
                break
            linhas.append(f"  pai de 0x{atual:x} -> 0x{pai:x} {classe(pai)} {dono(pai)}")
            atual = pai


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        linhas.append(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
        print("\n".join(linhas))
