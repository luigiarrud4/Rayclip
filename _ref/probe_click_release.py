"""Clique de verdade (SendInput) na build instalada muda a seleção?

Sem CDP: foto da janela antes/depois do clique e comparação de pixels. A linha
selecionada muda de fundo e o painel de detalhes troca de conteúdo, então
qualquer mudança confirma que a seleção se moveu; zero mudança confirma o bug.
"""

import ctypes
import subprocess
import time
from ctypes import wintypes as w

import PIL.Image
import PIL.ImageChops

import harness_compare as h
import shot_px

user32 = ctypes.windll.user32
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]

EXE = "C:\\Users\\Lugia\\AppData\\Local\\Rayclip\\rayclip.exe"
TMP = "C:/Users/Lugia/AppData/Local/Temp"


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", ctypes.c_long),
        ("dy", ctypes.c_long),
        ("mouseData", w.DWORD),
        ("dwFlags", w.DWORD),
        ("time", w.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class INPUT(ctypes.Structure):
    _fields_ = [("type", w.DWORD), ("mi", MOUSEINPUT)]


def mostrar():
    """Sinaliza a instância viva para a janela aparecer.

    A janela se esconde ao perder o foco, e qualquer console que o teste abra já
    tira o foco dela — por isso mostrar é o primeiro passo.
    """
    subprocess.run(["cmd", "/c", "start", "", EXE], capture_output=True)
    for _ in range(12):
        time.sleep(0.5)
        hwnd = h.find_window("rayclip.exe")
        if hwnd:
            return hwnd
    return None


def retangulo(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r


def clicar(x, y):
    """Clique esquerdo absoluto, evento real do Windows."""
    vx, vy = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    ax, ay = int(x * 65535 / (vx - 1)), int(y * 65535 / (vy - 1))
    for flag in (0x0002, 0x0004):  # MOVER | DOWN, depois MOVER | UP
        inp = INPUT()
        inp.type = 0
        inp.mi = MOUSEINPUT(ax, ay, 0, flag | 0x8000, 0, None)
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        time.sleep(0.03)


def foto(hwnd, rect, destino):
    shot_px.foto(hwnd, rect, destino)
    return PIL.Image.open(destino).convert("RGB")


def diff(a, b):
    d = PIL.ImageChops.difference(a, b).convert("L")
    return sum(1 for p in d.get_flattened_data() if p > 12)


if __name__ == "__main__":
    hwnd = mostrar()
    if not hwnd:
        print("janela nao encontrada")
        raise SystemExit(1)
    rect = retangulo(hwnd)
    print("janela:", hwnd, (rect.left, rect.top, rect.right, rect.bottom))
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.6)

    atual = foto(hwnd, rect, f"{TMP}/sel_a.png")
    for rotulo, offset in (("4a linha", 199), ("5a linha", 239), ("3a linha", 159)):
        clicar(rect.left + 150, rect.top + offset)
        time.sleep(0.8)
        nova = foto(hwnd, rect, f"{TMP}/sel_b.png")
        print(f"{rotulo}: pixels mudados = {diff(atual, nova)}")
        atual = nova
