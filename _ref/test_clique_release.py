"""Teste de clique na build INSTALADA, sem CDP.

O obstáculo: o app se esconde quando perde o foco, e o console que hospeda o
agente retoma o foco em milissegundos — então qualquer clique sintético caía no
console (ou no Discord, na primeira tentativa). A saída aqui é **minimizar o
console** enquanto o teste roda (ele continua rodando minimizado) e restaurar no
fim. Com o console fora do caminho, o app sustenta o foco e os cliques são
entregues a ele — com guarda de `WindowFromPoint` antes de cada clique.

Oraculo: foto da janela antes/depois (PrintWindow). Selecionar outra linha muda
o realce e o painel de detalhes, então a contagem de pixels diferentes tem de
ser grande; se a seleção não se mover, não muda nada.

Resultado em %TEMP%\\clique_resultado.txt (o roteiro pode rodar com pythonw).
"""

import ctypes
import io
import os
import sys
import time
from ctypes import wintypes as w

import harness_compare as h
import keysafe as k
import probe_click_release as p

u32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
u32.WindowFromPoint.argtypes = [w.POINT]
u32.WindowFromPoint.restype = w.HWND
u32.GetAncestor.argtypes = [w.HWND, ctypes.c_uint]
u32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
u32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
kernel32.GetConsoleWindow.restype = w.HWND

TMP = p.TMP
SAIDA = os.path.join(TMP, "clique_resultado.txt")
linhas = []
SW_MINIMIZE, SW_RESTORE = 6, 9


def diz(msg):
    linhas.append(msg)
    if sys.stdout:
        print(msg)


def nome_de(hwnd):
    if not hwnd:
        return "-"
    pid = w.DWORD(0)
    u32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    try:
        return h.proc_name(pid.value)
    except Exception:
        return "?"


def topo_no_ponto(x, y):
    return u32.GetAncestor(u32.WindowFromPoint(w.POINT(x, y)), 2)


def sustenta_foco(hwnd, ms=20):
    """Foca e confirma que o foco continua com o app por `ms` milissegundos."""
    k.focar(hwnd)
    fim = time.time() + ms / 1000
    while time.time() < fim:
        if u32.GetForegroundWindow() != hwnd or not u32.IsWindowVisible(hwnd):
            return False
        time.sleep(0.03)
    return True


def clique(hwnd, x, y, tentativas=6):
    """Foca e clica no mesmo fôlego: o console retoma o foco em ~1 s."""
    ultimo = "sem tentativa"
    for _ in range(tentativas):
        if not sustenta_foco(hwnd):
            ultimo = "sem foco"
            continue
        dono = topo_no_ponto(x, y)
        if dono != hwnd:
            ultimo = f"por cima esta {nome_de(dono)}"
            continue
        p.clicar(x, y)
        time.sleep(0.45)
        if u32.IsWindowVisible(hwnd):
            return "ok"
        ultimo = "escondeu depois do clique"
    return ultimo


def main():
    console = kernel32.GetConsoleWindow()
    if console:
        u32.ShowWindow(console, SW_MINIMIZE)
        time.sleep(0.4)
    try:
        hwnd = p.mostrar()
        if not hwnd:
            diz("janela nao encontrada")
            return
        rect = p.retangulo(hwnd)
        diz(f"janela {hwnd} em ({rect.left},{rect.top})-({rect.right},{rect.bottom})")
        atual = p.foto(hwnd, rect, f"{TMP}/rel0.png")
        for rotulo, off in (("1a linha", 159), ("3a linha", 199), ("5a linha", 239)):
            estado = clique(hwnd, rect.left + 150, rect.top + off)
            nova = p.foto(hwnd, rect, f"{TMP}/rel1.png")
            diz(f"clique {rotulo:<9} y+{off}: {estado:<30} pixels mudados = {p.diff(atual, nova)}")
            atual = nova
    finally:
        if console:
            u32.ShowWindow(console, SW_RESTORE)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        diz(f"ERRO: {type(exc).__name__}: {exc}")
    finally:
        io.open(SAIDA, "w", encoding="utf-8", newline="\n").write("\n".join(linhas) + "\n")
