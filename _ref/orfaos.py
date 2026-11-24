"""Existe janela órfã do WebView2 por cima da janela do Rayclip?

Um `msedgewebview2.exe` sobrevivente de uma instância morta continua com a janela
hospedeira viva, invisível, **por cima** do app — e o clique do usuário cai nela.
Foi o que o diagnóstico do `click-hook` mostrou: no ponto do clique o alvo era
`Chrome_RenderWidgetHostHWND` de outro processo, e a raiz também.

Aqui listamos todas as janelas `Chrome_*` com retângulo, dono e se cobrem o app.
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
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
SAIDA = os.path.join(os.environ["TEMP"], "orfaos.txt")
linhas = []
CB = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def diz(m):
    linhas.append(m)


def classe(hwnd):
    b = ctypes.create_unicode_buffer(128)
    u32.GetClassNameW(hwnd, b, 128)
    return b.value


def proc(pid):
    try:
        return h.proc_name(pid)
    except Exception:
        return "?"


def main():
    app = h.find_window("rayclip.exe")
    if not app:
        diz("janela do app nao encontrada")
        return
    r = w.RECT()
    u32.GetWindowRect(app, ctypes.byref(r))
    diz(f"app 0x{app:x} ({r.left},{r.top})-({r.right},{r.bottom})")

    # dono do app e do webview dele
    pid_app = w.DWORD(0)
    u32.GetWindowThreadProcessId(app, ctypes.byref(pid_app))
    webview_ok = set()

    def cb(hwnd, _l):
        cls = classe(hwnd)
        if not cls.startswith("Chrome"):
            return True
        p = w.DWORD(0)
        u32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        rr = w.RECT()
        u32.GetWindowRect(hwnd, ctypes.byref(rr))
        larg, alt = rr.right - rr.left, rr.bottom - rr.top
        vis = bool(u32.IsWindowVisible(hwnd))
        diz(
            f"{cls:<30} 0x{hwnd:x} {proc(p.value)}({p.value}) "
            f"({rr.left},{rr.top})-({rr.right},{rr.bottom}) {larg}x{alt} visivel={vis}"
        )
        if p.value != pid_app.value and vis and larg > 100 and alt > 100:
            # candidato a órfão: cobre parte do app?
            if rr.left < r.right and rr.right > r.left and rr.top < r.bottom and rr.bottom > r.top:
                diz(f"   !! cobre a area do app e nao e do processo do app")
        return True

    u32.EnumWindows(CB(cb), 0)

    # o que esta no ponto do meio?
    for dy in (179, 219, 300, 380):
        pt = w.POINT(r.left + 150, r.top + dy)
        alvo = u32.WindowFromPoint(pt)
        raiz = u32.GetAncestor(alvo, 2)
        p = w.DWORD(0)
        u32.GetWindowThreadProcessId(raiz, ctypes.byref(p))
        diz(
            f"ponto ({pt.x},{pt.y}) -> {classe(alvo)} raiz 0x{raiz:x} "
            f"{proc(p.value)}({p.value}) raiz==app:{raiz == app}"
        )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
        print("\n".join(linhas))
