"""Por que a barra de título fica "presa" — e por que `SWP_FRAMECHANGED` a apaga.

A janela do Rayclip nasce com `WS_CAPTION` no estilo (é o tao quem monta assim) e
o `WM_NCCALCSIZE` que chega **durante** o `CreateWindowExW` pode cair antes de o
estado do tao estar pronto lá dentro — e aí o `DefWindowProc` reserva a faixa do
título. Depois disso ninguém mais recalcula a área não-cliente: ela fica presa no
tamanho antigo **mesmo que a política mude**. Era por isso que maximizar e
restaurar (que manda um recálculo) fazia a barra desaparecer.

Este teste cria uma janela de brinquedo com o mesmo estilo e mede, em pixels, a
diferença entre a janela inteira e a área cliente (é exatamente a altura da faixa
do título):

1. criada com `WS_CAPTION` -> diferença > 0 (a faixa existe);
2. trocado o comportamento para "sem moldura" (`WM_NCCALCSIZE` -> 0) **sem**
   forçar recálculo -> a diferença continua > 0: a faixa fica presa;
3. depois de `SetWindowPos(SWP_FRAMECHANGED)` -> diferença 0.

O passo 3 é o que o app faz logo depois de criar/mostrar a janela.
"""

import ctypes
import sys
from ctypes import wintypes as w

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32

user32.DefWindowProcW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
user32.DefWindowProcW.restype = ctypes.c_ssize_t
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.CreateWindowExW.restype = w.HWND

WS_CAPTION = 0x00C00000
WS_SYSMENU = 0x00080000
WS_THICKFRAME = 0x00040000
WS_VISIBLE = 0x10000000

WM_NCCALCSIZE = 0x0083
WM_DESTROY = 0x0002

SWP_FLAGS = 0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020  # +SWP_FRAMECHANGED

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


estado = {"sem_moldura": False}


def criar(classe):
    def proc(hwnd, msg, wp, lp):
        if msg == WM_NCCALCSIZE and wp and estado["sem_moldura"]:
            return 0
        if msg == WM_DESTROY:
            user32.PostQuitMessage(0)
            return 0
        return user32.DefWindowProcW(hwnd, msg, wp, lp)

    cb = WNDPROC(proc)
    wc = WNDCLASSW()
    wc.lpfnWndProc = cb
    wc.hInstance = kernel32.GetModuleHandleW(None)
    wc.lpszClassName = classe
    user32.RegisterClassW(ctypes.byref(wc))
    estilo = WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_VISIBLE
    hwnd = user32.CreateWindowExW(
        0, classe, "Rayclip", estilo, 140, 140, 500, 240, None, None, wc.hInstance, None
    )
    return hwnd, cb


def faixa(hwnd):
    """Altura da faixa não-cliente no topo, em pixels."""
    r, c = w.RECT(), w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    user32.GetClientRect(hwnd, ctypes.byref(c))
    return (r.bottom - r.top) - c.bottom


def bomba(ms=400):
    msg = w.MSG()
    fim = kernel32.GetTickCount() + ms
    while kernel32.GetTickCount() < fim:
        while user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, 1):
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))


def main():
    hwnd, cb = criar("RayclipTesteFaixa")
    print("hwnd:", hwnd, "erro:", kernel32.GetLastError())
    bomba(500)
    antes = faixa(hwnd)
    print(f"1) janela criada com WS_CAPTION: faixa = {antes}px")

    estado["sem_moldura"] = True
    bomba(300)
    presa = faixa(hwnd)
    print(f"2) sem recálculo (a faixa fica presa): faixa = {presa}px")

    user32.SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FLAGS)
    bomba(400)
    depois = faixa(hwnd)
    print(f"3) depois de SWP_FRAMECHANGED: faixa = {depois}px")

    user32.DestroyWindow(hwnd)
    bomba(200)
    _ = cb  # mantém o ponteiro vivo até o fim

    ok = antes > 0 and presa > 0 and depois == 0
    print("RESULTADO:", "OK" if ok else "FALHOU")
    return 0 if ok else 2


if __name__ == "__main__":
    sys.exit(main())
