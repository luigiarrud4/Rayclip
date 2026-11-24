"""`SendInput` correto (união de verdade) — a armadilha que custou o teste todo.

A versão anterior montava `INPUT` só com o campo do mouse/teclado: a struct saía
com **32 bytes** em vez de 40, `SendInput` falhava (devolvia 0) e **nada era
enviado**. Resultado: o app "se escondia no clique" (era o console roubando o
foco, sem clique nenhum) e as teclas não faziam nada. Toda a conclusão de que o
ambiente não conseguia dirigir o app estava errada.

Aqui a união é explícita e o retorno é conferido.
"""

import ctypes
from ctypes import wintypes as w

user32 = ctypes.windll.user32


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", ctypes.c_long),
        ("dy", ctypes.c_long),
        ("mouseData", w.DWORD),
        ("dwFlags", w.DWORD),
        ("time", w.DWORD),
        ("dwExtraInfo", ctypes.c_size_t),
    ]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", w.WORD),
        ("wScan", w.WORD),
        ("dwFlags", w.DWORD),
        ("time", w.DWORD),
        ("dwExtraInfo", ctypes.c_size_t),
    ]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [("uMsg", w.DWORD), ("wParamL", w.WORD), ("wParamH", w.WORD)]


class _UNIAO(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("hi", HARDWAREINPUT)]


class INPUT(ctypes.Structure):
    _anonymous_ = ("u",)
    _fields_ = [("type", w.DWORD), ("u", _UNIAO)]


assert ctypes.sizeof(INPUT) == 40, f"INPUT com {ctypes.sizeof(INPUT)} bytes (esperado 40)"
assert ctypes.sizeof(MOUSEINPUT) == 32, f"MOUSEINPUT com {ctypes.sizeof(MOUSEINPUT)} bytes"


def _enviar(inp):
    enviados = user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
    if enviados != 1:
        raise RuntimeError(f"SendInput falhou (erro {ctypes.get_last_error()})")


def clicar(x, y):
    """Clique esquerdo em coordenadas absolutas de tela."""
    vx, vy = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    ax, ay = int(x * 65535 / (vx - 1)), int(y * 65535 / (vy - 1))
    for flag in (0x0001 | 0x8000, 0x0002 | 0x8000, 0x0004 | 0x8000):
        # MOVE | ABSOLUTE, depois DOWN e UP na posição
        inp = INPUT(type=0, u=_UNIAO(mi=MOUSEINPUT(ax, ay, 0, flag, 0, 0)))
        _enviar(inp)


def tecla(vk, gap=0.02):
    """Aperta e solta uma tecla virtual."""
    import time

    for flag in (0, 0x0002):
        _enviar(INPUT(type=1, u=_UNIAO(ki=KEYBDINPUT(vk, 0, flag, 0, 0))))
        time.sleep(gap)
