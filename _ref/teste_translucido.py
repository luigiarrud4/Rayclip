"""O translúcido está forte o bastante? Medido, não "no olho".

Monta um fundo **branco** atrás da janela do Rayclip e mede a luminância média de
uma faixa dentro do app em três opacidades. Com desfoque de verdade, o branco de
trás atravessa e a luminância sobe junto com a transparência; sem desfoque (janela
opaca, ou efeitos de transparência do Windows desligados) ela não muda.

Também confere o atributo 38 do DWM (`DWMWA_SYSTEMBACKDROP_TYPE`): 3 = acrílico,
1 = nenhum.
"""

import ctypes
import json
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import harness_compare as h  # noqa: E402
import teste_fundo  # noqa: E402

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
kernel32 = ctypes.windll.kernel32
dwmapi = ctypes.windll.dwmapi

user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]

WS_POPUP = 0x80000000
WS_VISIBLE = 0x10000000
WS_EX_NOACTIVATE = 0x08000000  # o fundo nunca rouba o foco (o app se esconde)
HWND_TOP = 0
LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [
        ("style", ctypes.c_uint),
        ("lpfnWndProc", WNDPROC),
        ("cbClsExtra", ctypes.c_int),
        ("cbWndExtra", ctypes.c_int),
        ("hInstance", w.HINSTANCE),
        ("hIcon", w.HICON),
        ("hCursor", w.HANDLE),
        ("hbrBackground", w.HBRUSH),
        ("lpszMenuName", w.LPCWSTR),
        ("lpszClassName", w.LPCWSTR),
    ]


def fundo_branco():
    """Janela branca ocupando a tela inteira, jogada para o fim do z-order."""
    user32.DefWindowProcW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
    user32.DefWindowProcW.restype = ctypes.c_ssize_t

    def proc(hwnd, msg, wp, lp):
        return user32.DefWindowProcW(hwnd, msg, wp, lp)

    cb = WNDPROC(proc)
    wc = WNDCLASSW()
    wc.lpfnWndProc = cb
    wc.hInstance = kernel32.GetModuleHandleW(None)
    wc.lpszClassName = "RayclipFundoBranco"
    wc.hbrBackground = gdi32.CreateSolidBrush(0xFFFFFF)
    user32.RegisterClassW(ctypes.byref(wc))
    user32.CreateWindowExW.restype = w.HWND
    hwnd = user32.CreateWindowExW(
        WS_EX_NOACTIVATE, "RayclipFundoBranco", "", WS_POPUP | WS_VISIBLE, 0, 0, 1920, 1080,
        None, None, wc.hInstance, None,
    )
    # HWND_BOTTOM = 1
    user32.SetWindowPos(hwnd, w.HWND(1), 0, 0, 1920, 1080, 0x0002 | 0x0004)
    return hwnd, cb


def luminancia(img):
    px = list(img.convert("RGB").getdata())
    return round(sum((r * 299 + g * 587 + b * 114) / 1000 for r, g, b in px) / len(px), 1)


def janela_do_app():
    """HWND principal mesmo quando a janela está escondida (o app se esconde
    ao perder o foco, e `find_window` só vê janela visível)."""
    achado = h.find_window("rayclip.exe")
    if achado:
        return achado
    pids = [p.info["pid"] for p in __import__("psutil").process_iter(["pid", "name"])
            if (p.info["name"] or "").lower() == "rayclip.exe"]
    alvos = set(pids)

    def cb(hwnd, _lp):
        pid = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value in alvos:
            c = w.RECT()
            user32.GetClientRect(hwnd, ctypes.byref(c))
            if c.right - c.left > 300:
                lista.append(hwnd)
        return True

    lista = []
    proc_t = ctypes.WINFUNCTYPE(ctypes.c_bool, w.HWND, w.LPARAM)(cb)
    user32.EnumWindows(proc_t, 0)
    return lista[0] if lista else None


def main():
    app = janela_do_app()
    if not app:
        raise SystemExit("rayclip nao esta aberto")
    user32.ShowWindow(app, 8)  # SW_SHOWNA: mostra sem roubar o foco
    s = teste_fundo.Sessao()

    # fundo branco atrás + app na frente (sem roubar o foco do app)
    branco, _cb = fundo_branco()
    user32.SetWindowPos(app, w.HWND(HWND_TOP), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040)
    user32.SetForegroundWindow(app)
    time.sleep(0.8)

    tipo = ctypes.c_int()
    dwmapi.DwmGetWindowAttribute(app, 38, ctypes.byref(tipo), ctypes.sizeof(tipo))
    print("DWMWA_SYSTEMBACKDROP_TYPE:", tipo.value, "(3 = acrílico, 1 = nenhum)")
    print("translúcido no Windows:", teste_fundo.Sessao().js(
        "(() => document.documentElement.dataset.translucency)()"
    ))

    r = w.RECT()
    user32.GetWindowRect(app, ctypes.byref(r))
    # faixa no meio da lista (fundo da janela, longe de texto/badges)
    caixa = (r.left + 40, r.top + 300, r.right - 420, r.top + 420)

    def medir(**patch):
        teste_fundo.salvar(s, patch)
        time.sleep(1.0)
        user32.SetWindowPos(app, w.HWND(HWND_TOP), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040)
        time.sleep(0.4)
        img = ImageGrab.grab(bbox=caixa)
        return luminancia(img)

    solido = medir(bgColor="", bgOpacity=1.0)
    padrao = medir(bgColor="", bgOpacity=0.55)
    forte = medir(bgColor="", bgOpacity=0.35)
    print(f"luminância da faixa com fundo branco atrás:")
    print(f"  opacidade 1.00 (sólido): {solido}")
    print(f"  opacidade 0.55 (padrão): {padrao}")
    print(f"  opacidade 0.35 (máximo): {forte}")

    teste_fundo.salvar(s, {"bgColor": "", "bgOpacity": 0.55})
    user32.DestroyWindow(branco)
    ok = padrao > solido + 6 and forte > padrao + 4
    print("RESULTADO:", "OK" if ok else "SEM TRANSFERENCIA VISIVEL")
    return 0 if ok else 2


if __name__ == "__main__":
    sys.exit(main())
