"""Teclado na build INSTALADA: seta muda a seleção? (sem mouse envolvido)

Se as setas movem a seleção e o painel de detalhes muda, a lógica de seleção da
build instalada está boa — e o que atrapalha o teste de clique é o gancho de
mouse com eventos injetados (o `WindowFromPoint` no instante do down não devolve
a nossa janela).
"""

import ctypes
import io
import os
import sys
import time
from ctypes import wintypes as w

import PIL.Image
import PIL.ImageChops

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
TMP = p.TMP
SAIDA = os.path.join(TMP, "tecla_resultado.txt")
linhas = []


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", w.WORD),
        ("wScan", w.WORD),
        ("dwFlags", w.DWORD),
        ("time", w.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class INPUT(ctypes.Structure):
    _fields_ = [("type", w.DWORD), ("ki", KEYBDINPUT)]


VK = {"Down": 0x28, "Up": 0x26}


def diz(msg):
    linhas.append(msg)
    if sys.stdout:
        print(msg)


def tecla(vk):
    for flag in (0, 0x0002):  # keydown, keyup
        inp = INPUT()
        inp.type = 1  # INPUT_KEYBOARD
        inp.ki = KEYBDINPUT(vk, 0, flag, 0, None)
        u32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        time.sleep(0.02)


def painel(img):
    larg, alt = img.size
    return img.crop((int(larg * 0.58), int(alt * 0.12), larg, alt))


def diff_painel(a, b):
    d = PIL.ImageChops.difference(painel(a), painel(b)).convert("L")
    return sum(1 for v in d.get_flattened_data() if v > 12)


def main():
    time.sleep(4)
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        p.mostrar()
        hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        diz("janela nao encontrada")
        return
    rect = p.retangulo(hwnd)
    k.focar(hwnd, espera=0.4)
    time.sleep(0.4)
    diz(f"janela {hwnd} em ({rect.left},{rect.top})")
    atual = p.foto(hwnd, rect, f"{TMP}/k0.png")
    for rotulo, nome in (("seta ↓", "Down"), ("seta ↓", "Down"), ("seta ↑", "Up")):
        k.focar(hwnd, espera=0.3)
        if u32.GetForegroundWindow() != hwnd:
            diz(f"{rotulo}: sem foco, pulado")
            continue
        tecla(VK[nome])
        time.sleep(0.4)
        nova = p.foto(hwnd, rect, f"{TMP}/k1.png")
        diz(
            f"{rotulo}: visivel={bool(u32.IsWindowVisible(hwnd))} "
            f"painel mudou = {diff_painel(atual, nova)} px"
        )
        atual = nova


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
