"""Clique **de verdade** no ícone da bandeja (o caminho que o usuário usa).

O app pergunta ao próprio tauri onde está o ícone (`tray_rect`), o teste abre a
gaveta de ícones ocultos e clica nele com entrada sintética. Depois mede a faixa
`janela − cliente`, a luminância das primeiras 18 linhas (a legenda clássica
clareia esse pedaço) e guarda a captura do topo.
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import teste_tray as T  # noqa: E402
from teste_fundo import Sessao  # noqa: E402

u = ctypes.windll.user32
u.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]

CHEVRON = (1725, 1055)


def topo(hwnd, altura=18):
    r = w.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(r))
    im = ImageGrab.grab(bbox=(r.left, r.top, r.right, r.top + altura)).convert("L")
    d = list(im.getdata())
    return round(sum(d) / len(d), 1)


def recorte(hwnd, altura=90):
    r = w.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(r))
    return ImageGrab.grab(bbox=(r.left, r.top, r.right, r.top + altura))


def achar(exe="rayclip.exe"):
    """Janela principal **mesmo escondida** (a do teste é a que está oculta)."""
    achados = []

    def cb(hwnd, _l):
        pid = w.DWORD(0)
        u.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == 0:
            return True
        if h.proc_name(pid.value) != exe:
            return True
        if h.class_of(hwnd) != "Tauri Window":
            return True
        achados.append(hwnd)
        return True

    u.EnumWindows(h.WNDENUMPROC(cb), 0)
    return achados[0] if achados else None


def main():
    hwnd = achar()
    if not hwnd:
        raise SystemExit("rayclip nao esta aberto")
    s = Sessao()

    if u.IsWindowVisible(hwnd):
        s.js("window.__TAURI_INTERNALS__.invoke('hide_window')")
        time.sleep(0.8)
    limpo = None

    for volta in (1, 2):
        # a gaveta precisa estar aberta para o ícone ser clicável
        entrada.clicar(*CHEVRON)
        time.sleep(1.4)
        r = s.js("window.__TAURI_INTERNALS__.invoke('tray_rect')")
        if not r:
            raise SystemExit("tray_rect nao respondeu")
        x, y, ww, hh = r
        entrada.clicar(x + ww // 2, y + hh // 2)
        time.sleep(2.0)
        visivel = bool(u.IsWindowVisible(hwnd))
        faixa, _ = T.medidas(hwnd)
        if visivel:
            atual = topo(hwnd)
            if limpo is None:
                limpo = atual
            print(
                f"volta {volta}: icone=({x+ww//2},{y+hh//2}) visivel={visivel} "
                f"faixa={faixa} topo={atual} (referencia={limpo}) "
                f"{'OK' if faixa == (0, 0) and atual < limpo + 10 else 'LEGENDA!'}"
            )
            recorte(hwnd).save(rf"C:\Windows\System32\_rayclip_tmp\tray_topo_{volta}.png")
            s.js("window.__TAURI_INTERNALS__.invoke('hide_window')")
            time.sleep(0.8)
        else:
            print(f"volta {volta}: o clique nao abriu a janela (visivel=False)")

    print("--- log ---")
    for l in T.cauda(14):
        print(" ", l)
    return 0


if __name__ == "__main__":
    sys.exit(main())
