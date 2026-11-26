"""Moldura: faixa nula, resize, maximizar e o caminho clássico (sabotagem).

1. Abre e mede `janela − cliente` (a faixa).
2. Arrasta a borda direita: a janela tem de crescer (o resize não pode morrer).
3. Maximiza: o cliente não pode cobrir a barra de tarefas.
4. Sabotagem: força `DWMWA_NCRENDERING_POLICY = ENABLED` (é o estado em que o
   tema clássico pinta a legenda) e manda `WM_NCACTIVATE` + as mensagens
   `WM_NCUAHDRAWCAPTION`. Depois disso a faixa tem de continuar 0 e o topo da
   janela não pode ganhar ícone/título.
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

from PIL import ImageGrab

sys.path.insert(0, "_ref")
import entrada  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402
import teste_tray as T  # noqa: E402

u = ctypes.windll.user32
dwm = ctypes.windll.dwmapi
u.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
u.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
u.SendMessageW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
u.SendMessageW.restype = ctypes.c_ssize_t
u.PostMessageW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
dwm.DwmSetWindowAttribute.argtypes = [w.HWND, w.DWORD, ctypes.c_void_p, w.DWORD]

WM_NCACTIVATE = 0x0086
WM_NCUAHDRAWCAPTION = 0x00AE
WM_NCUAHDRAWFRAME = 0x00AF
SWP_FRAMECHANGED = 0x0020
SWP_NOMOVE = 0x0002
SWP_NOSIZE = 0x0001
SWP_NOZORDER = 0x0004


def caixa(hwnd):
    jan, cli = w.RECT(), w.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(jan))
    u.GetClientRect(hwnd, ctypes.byref(cli))
    return (jan.left, jan.top, jan.right, jan.bottom), (
        (jan.bottom - jan.top) - (cli.bottom - cli.top),
        (jan.right - jan.left) - (cli.right - cli.left),
    )


def topo_marcado(hwnd):
    """Pixels claros nas primeiras 26 linhas do canto superior esquerdo.

    A legenda clássica ocupa justamente as primeiras ~20 linhas com ícone e
    título claros; a barra do app tem só um respiro escuro aqui (o texto do
    filtro começa mais abaixo). Medido: limpo **0**, com legenda **670**.
    Luminância média **não** serve: muda com o desfoque e com o papel de parede.
    """
    (l, t, r, b), _ = caixa(hwnd)
    im = ImageGrab.grab(bbox=(l, t, min(l + 260, r), t + 26)).convert("L")
    return sum(1 for p in im.getdata() if p > 140)


def main():
    import teste_tray_clique as C
    hwnd = C.achar()
    if not hwnd:
        raise SystemExit("rayclip nao esta aberto")
    u.SetWindowPos(hwnd, None, 0, 0, 1120, 720, SWP_NOMOVE | SWP_NOZORDER)
    u.ShowWindow(hwnd, 5)
    keysafe.focar(hwnd)      # sem foco o app se esconde e a captura pega o desktop
    time.sleep(2.2)          # a webview precisa terminar de pintar a barra
    jan, faixa = caixa(hwnd)
    limpo = topo_marcado(hwnd)
    print(f"1. aberta   : faixa={faixa} topo={limpo}")

    # 2. resize pela borda direita
    largura0 = jan[2] - jan[0]
    keysafe.focar(hwnd)
    entrada.mover(jan[2] - 2, (jan[1] + jan[3]) // 2)
    time.sleep(0.2)
    entrada.baixo()
    for i in range(1, 12):
        entrada.mover(jan[2] - 2 + i * 8, (jan[1] + jan[3]) // 2)
        time.sleep(0.03)
    entrada.cima()
    time.sleep(0.6)
    jan2, faixa2 = caixa(hwnd)
    largura1 = jan2[2] - jan2[0]
    print(f"2. resize   : {largura0} -> {largura1} px {'OK' if largura1 > largura0 + 20 else 'FALHOU'} faixa={faixa2}")

    # 3. maximizar não pode cobrir a barra de tarefas
    u.ShowWindow(hwnd, 3)  # SW_MAXIMIZE
    time.sleep(1.0)
    (l, t, r, b), faixa3 = caixa(hwnd)
    tela = u.GetSystemMetrics(1)
    print(f"3. maximada : {r-l}x{b-t} faixa={faixa3} tela=1080 {'OK (deixa a barra)' if b - t < tela - 20 else 'COBRE A BARRA'}")
    u.ShowWindow(hwnd, 9)  # SW_RESTORE
    keysafe.focar(hwnd)
    time.sleep(1.5)
    print("3a. visivel   :", bool(u.IsWindowVisible(hwnd)))
    # referência imediatamente antes da sabotagem, no **mesmo** estado da janela
    limpo = topo_marcado(hwnd)
    print(f"3b. referencia: topo_claro={limpo}")

    # 4. sabotagem: liga o NC no DWM e manda pintar a legenda clássica
    ativo = ctypes.c_uint(0)  # DWMNCRP_USEWINDOWSTYLE
    dwm.DwmSetWindowAttribute(hwnd, 1, ctypes.byref(ativo), ctypes.sizeof(ativo))
    u.PostMessageW(hwnd, WM_NCUAHDRAWCAPTION, 0, 0)
    u.PostMessageW(hwnd, WM_NCUAHDRAWFRAME, 0, 0)
    u.SendMessageW(hwnd, WM_NCACTIVATE, 1, 0)
    u.SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED)
    time.sleep(1.2)
    (l, t, r, b), faixa4 = caixa(hwnd)
    claro = topo_marcado(hwnd)
    ok = faixa4 == (0, 0) and claro <= limpo * 1.3 + 40
    print(f"4. sabotada : faixa={faixa4} topo_claro={claro} (limpo={limpo}) {'OK (sem legenda)' if ok else 'LEGENDA APARECEU'}")
    ImageGrab.grab(bbox=(l, t, r, t + 90)).save(r"C:\Windows\System32\_rayclip_tmp\s43_topo.png")

    # devolve a política e a posição original
    desligado = ctypes.c_uint(1)  # DWMNCRP_DISABLED
    dwm.DwmSetWindowAttribute(hwnd, 1, ctypes.byref(desligado), ctypes.sizeof(desligado))
    u.SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED)
    return 0


if __name__ == "__main__":
    sys.exit(main())
