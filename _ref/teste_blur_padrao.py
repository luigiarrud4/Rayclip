"""O fundo está sendo **desfocado** ou só ficando transparente?

Um fundo branco não distingue as duas coisas: as duas clareiam a janela. Aqui
atrás da janela vai um fundo de **listras pretas e brancas** de 10 px:

- com desfoque de verdade, as listras somem e a faixa vira um cinza uniforme
  (desvio-padrão baixo);
- sem desfoque (janela só transparente), as listras aparecem (desvio alto).

Roda com o app aberto e CDP ligado; o resultado é a média e o desvio da faixa.
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import teste_fundo  # noqa: E402
import teste_translucido as T  # noqa: E402

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
kernel32 = ctypes.windll.kernel32

WS_POPUP = 0x80000000
WS_VISIBLE = 0x10000000
WS_EX_NOACTIVATE = 0x08000000
WM_PAINT = 0x000F
WM_ERASEBKGND = 0x0014

LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t)


class PAINTSTRUCT(ctypes.Structure):
    _fields_ = [
        ("hdc", w.HDC),
        ("fErase", w.BOOL),
        ("rcPaint", w.RECT),
        ("fRestore", w.BOOL),
        ("fIncUpdate", w.BOOL),
        ("rgbReserved", ctypes.c_byte * 32),
    ]


def listras():
    user32.DefWindowProcW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
    user32.DefWindowProcW.restype = ctypes.c_ssize_t
    user32.CreateWindowExW.restype = w.HWND
    user32.CreateWindowExW.argtypes = [
        w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD, ctypes.c_int, ctypes.c_int,
        ctypes.c_int, ctypes.c_int, w.HWND, w.HMENU, ctypes.c_void_p, ctypes.c_void_p,
    ]

    def pintar(hdc, largura, altura):
        preto = gdi32.CreateSolidBrush(0x000000)
        branco = gdi32.CreateSolidBrush(0xFFFFFF)
        x = 0
        while x < largura:
            r = w.RECT(x, 0, min(x + 10, largura), altura)
            user32.FillRect(hdc, ctypes.byref(r), branco if (x // 10) % 2 else preto)
            x += 10
        gdi32.DeleteObject(preto)
        gdi32.DeleteObject(branco)

    def proc(hwnd, msg, wp, lp):
        if msg == WM_ERASEBKGND:
            return 1
        if msg == WM_PAINT:
            hdc, ps = user32.BeginPaint(hwnd, ctypes.byref(PAINTSTRUCT()))
            r = w.RECT()
            user32.GetClientRect(hwnd, ctypes.byref(r))
            pintar(hdc, r.right, r.bottom)
            user32.EndPaint(hwnd, ctypes.byref(ps))
            return 0
        return user32.DefWindowProcW(hwnd, msg, wp, lp)

    cb = WNDPROC(proc)
    wc = T.WNDCLASSW()
    wc.lpfnWndProc = cb
    kernel32.GetModuleHandleW.restype = ctypes.c_void_p
    wc.hInstance = kernel32.GetModuleHandleW(None) or 0
    wc.lpszClassName = "RayclipListras"
    user32.RegisterClassW(ctypes.byref(wc))
    hwnd = user32.CreateWindowExW(
        WS_EX_NOACTIVATE, "RayclipListras", "", WS_POPUP | WS_VISIBLE, 0, 0, 1920, 1080,
        None, None, wc.hInstance, None,
    )
    user32.SetWindowPos(hwnd, w.HWND(1), 0, 0, 1920, 1080, 0x0002 | 0x0004)
    return hwnd, cb


def estat(img):
    px = [((r * 299 + g * 587 + b * 114) / 1000) for r, g, b in img.convert("RGB").getdata()]
    media = sum(px) / len(px)
    var = sum((v - media) ** 2 for v in px) / len(px)
    return round(media, 1), round(var**0.5, 1)


def main():
    app = T.janela_do_app()
    if not app:
        raise SystemExit("rayclip nao esta aberto")
    user32.ShowWindow(app, 8)
    s = teste_fundo.Sessao()
    fundo, _cb = listras()
    user32.SetWindowPos(app, w.HWND(0), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040)
    time.sleep(0.8)

    r = w.RECT()
    user32.GetWindowRect(app, ctypes.byref(r))
    caixa = (r.left + 40, r.top + 300, r.right - 420, r.top + 420)

    # Esconde a interface: assim a faixa capturada é **só** o vidro da janela
    # (sem texto nem linhas da lista atrapalhando o desvio-padrão).
    s.js(
        "(() => { document.body.style.visibility = 'hidden';"
        " document.documentElement.style.setProperty('background','transparent');"
        " return 1; })()"
    )

    for op in (1.0, 0.5):
        teste_fundo.salvar(s, {"bgColor": "", "bgOpacity": op})
        time.sleep(1.1)
        user32.ShowWindow(app, 8)
        user32.SetWindowPos(app, w.HWND(0), 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040)
        time.sleep(0.4)
        visivel = bool(user32.IsWindowVisible(app))
        media, desvio = estat(ImageGrab.grab(bbox=caixa))
        print(f"op={op:<5} visivel={visivel} media={media:<6} desvio={desvio:<6} "
              f"({'LISTRAS VISIVEIS (sem desfoque)' if desvio > 12 else 'desfocado'})")

    s.js("(() => { document.body.style.visibility = '';"
         " document.documentElement.style.removeProperty('background'); return 1; })()")
    teste_fundo.salvar(s, {"bgColor": "", "bgOpacity": 0.55})
    user32.DestroyWindow(fundo)
    return 0


if __name__ == "__main__":
    sys.exit(main())
