"""Print da janela de um processo + estatistica dos pixels.

Serve para provar que a janela ficou BRANCA (renderer morto) e voltou: janela
branca e quase uniforme (media ~255, desvio ~0); a interface de verdade e escura
(media < 60). Usa PrintWindow, entao funciona mesmo com a janela coberta.
"""

import ctypes
import sys
from ctypes import wintypes as w

from PIL import Image

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

user32.EnumWindows.argtypes = [ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM), w.LPARAM]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.PrintWindow.argtypes = [w.HWND, w.HDC, w.UINT]
user32.GetDC.argtypes = [w.HWND]
user32.ReleaseDC.argtypes = [w.HWND, w.HDC]
user32.SetProcessDPIAware()

gdi32.CreateCompatibleDC.argtypes = [w.HDC]
gdi32.CreateCompatibleBitmap.argtypes = [w.HDC, ctypes.c_int, ctypes.c_int]
gdi32.SelectObject.argtypes = [w.HDC, w.HGDIOBJ]
gdi32.DeleteObject.argtypes = [w.HGDIOBJ]
gdi32.DeleteDC.argtypes = [w.HDC]

CB = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def janela(pid, somente_visiveis=True):
    achadas = []

    def cb(h, _l):
        p = w.DWORD()
        user32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and (not somente_visiveis or user32.IsWindowVisible(h)):
            r = w.RECT()
            user32.GetWindowRect(h, ctypes.byref(r))
            area = (r.right - r.left) * (r.bottom - r.top)
            if area > 20000:
                achadas.append((area, h, r))
        return True

    user32.EnumWindows(CB(cb), 0)
    achadas.sort(reverse=True)
    return achadas[0][1:] if achadas else (None, None)


def foto(hwnd, rect, destino):
    larg = rect.right - rect.left
    alt = rect.bottom - rect.top
    hdc = user32.GetDC(0)
    mdc = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, larg, alt)
    gdi32.SelectObject(mdc, bmp)
    # 2 = PW_RENDERFULLCONTENT: pega o que o WebView2 compoe
    user32.PrintWindow(hwnd, mdc, 2)

    class BITMAPINFOHEADER(ctypes.Structure):
        _fields_ = [
            ("biSize", w.DWORD),
            ("biWidth", ctypes.c_long),
            ("biHeight", ctypes.c_long),
            ("biPlanes", w.WORD),
            ("biBitCount", w.WORD),
            ("biCompression", w.DWORD),
            ("biSizeImage", w.DWORD),
            ("biXPelsPerMeter", ctypes.c_long),
            ("biYPelsPerMeter", ctypes.c_long),
            ("biClrUsed", w.DWORD),
            ("biClrImportant", w.DWORD),
        ]

    bi = BITMAPINFOHEADER(ctypes.sizeof(BITMAPINFOHEADER), larg, -alt, 1, 32, 0, 0, 0, 0, 0, 0)
    buf = ctypes.create_string_buffer(larg * alt * 4)
    gdi32.GetDIBits.argtypes = [
        w.HDC, w.HBITMAP, w.UINT, w.UINT, ctypes.c_void_p,
        ctypes.POINTER(BITMAPINFOHEADER), w.UINT,
    ]
    gdi32.GetDIBits(mdc, bmp, 0, alt, buf, ctypes.byref(bi), 0)
    img = Image.frombuffer("RGBA", (larg, alt), buf, "raw", "BGRA", 0, 1).convert("RGB")
    if destino:
        img.save(destino)
    gdi32.DeleteObject(bmp)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(0, hdc)
    return img


def estatistica(pid):
    hwnd, rect = janela(pid)
    if not hwnd:
        return None
    img = foto(hwnd, rect, None)
    cinza = img.convert("L")
    px = list(cinza.getdata())
    media = sum(px) / len(px)
    var = sum((v - media) ** 2 for v in px) / len(px)
    return {"media": round(media, 1), "desvio": round(var ** 0.5, 1), "tam": img.size}


if __name__ == "__main__":
    pid = int(sys.argv[1])
    saida = sys.argv[2] if len(sys.argv) > 2 else None
    hwnd, rect = janela(pid)
    if not hwnd:
        print("sem janela visivel")
        sys.exit(1)
    img = foto(hwnd, rect, saida)
    cinza = img.convert("L")
    px = list(cinza.getdata())
    media = sum(px) / len(px)
    var = sum((v - media) ** 2 for v in px) / len(px)
    print(f"hwnd={hwnd} {img.size} media={media:.1f} desvio={var ** 0.5:.1f}")
