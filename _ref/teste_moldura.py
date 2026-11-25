"""Prova que a moldura nativa não pode voltar (v0.1.45).

O sintoma relatado: em alguns PCs (e na VM) aparecia a barra de título clássica
— a "do Windows 7" — e ela piscava ao mover/clicar.

Causa: o tao monta o estilo de toda janela sem moldura com `WS_CAPTION`
(`to_window_styles()`), e quem esconde a barra é só o DWM. A política
`DWMWA_NCRENDERING_POLICY = DWMNCRP_DISABLED` era pedida **uma vez**, no
`setup`. Se naquele instante o DWM ainda não compunha (autostart no logon,
máquina lenta, VM/RDP) a chamada voltava `DWM_E_COMPOSITIONDISABLED`, seguia sem
nova tentativa, e o tema clássico desenhava a barra dentro do cliente — repintada
a cada ativação/movimento.

Agora: `WM_NCPAINT -> 0` (o tema clássico não pinta nada) e a política do DWM é
reafirmada em `WM_DWMCOMPOSITIONCHANGED`, `WM_THEMECHANGED` e `WM_STYLECHANGED`.
O estilo **não** é tocado, senão o redimensionamento pela borda morre (medido).

Mede:
1. `DWMWA_NCRENDERING_ENABLED` = FALSE (o DWM não desenha a área não-cliente);
2. esconder/mostrar (caminho que reaplica estilos no tao) — continua FALSE;
3. sabotagem: forçar `DWMNCRP_ENABLED` e mandar `WM_THEMECHANGED` -> a subclasse
   reafirma e o atributo volta a FALSE (prova de que a subclasse está viva);
4. arrastar a borda direita redimensiona (o estilo continua com WS_THICKFRAME e
   WS_CAPTION, que é o que dá a alça de resize neste app).
"""

import ctypes
import sys
import time
from ctypes import wintypes as w

sys.path.insert(0, "_ref")
import entrada as e  # noqa: E402
import harness_compare as h  # noqa: E402
import keysafe  # noqa: E402

user32 = ctypes.windll.user32
dwmapi = ctypes.windll.dwmapi

user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetWindowLongPtrW.argtypes = [w.HWND, ctypes.c_int]
user32.GetWindowLongPtrW.restype = ctypes.c_ssize_t
user32.SendMessageW.argtypes = [w.HWND, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
user32.SendMessageW.restype = ctypes.c_ssize_t

DWMWA_NCRENDERING_ENABLED = 1
DWMWA_NCRENDERING_POLICY = 2
DWMNCRP_ENABLED = 2
WM_THEMECHANGED = 0x031A


def ncr_habilitado(hwnd):
    """`DwmGetWindowAttribute(DWMWA_NCRENDERING_ENABLED)` -> bool."""
    v = ctypes.c_int(-1)
    hr = dwmapi.DwmGetWindowAttribute(
        hwnd, DWMWA_NCRENDERING_ENABLED, ctypes.byref(v), ctypes.sizeof(v)
    )
    return (hr, bool(v.value))


def poe_politica(hwnd, valor):
    v = ctypes.c_uint(valor)
    return dwmapi.DwmSetWindowAttribute(
        hwnd, DWMWA_NCRENDERING_POLICY, ctypes.byref(v), ctypes.sizeof(v)
    )


def tamanho(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return (r.right - r.left, r.bottom - r.top)


def kd(vk):
    e._enviar(e.INPUT(type=1, u=e._UNIAO(ki=e.KEYBDINPUT(vk, 0, 0, 0, 0))))


def ku(vk):
    e._enviar(e.INPUT(type=1, u=e._UNIAO(ki=e.KEYBDINPUT(vk, 0, 0x0002, 0, 0))))


def atalho():
    kd(0xA4)
    time.sleep(0.06)
    kd(0x20)
    time.sleep(0.04)
    ku(0x20)
    time.sleep(0.04)
    ku(0xA4)
    time.sleep(1.3)


def main():
    hwnd = h.find_window("rayclip.exe")
    if not hwnd:
        print("app nao esta na tela (use --open)")
        return 1

    hr, ativo = ncr_habilitado(hwnd)
    print(f"1) NC renderizado pelo DWM: {ativo} (hr=0x{hr & 0xFFFFFFFF:08x})")
    ok1 = hr == 0 and not ativo

    atalho()  # esconde
    atalho()  # mostra
    hwnd = h.find_window("rayclip.exe")
    hr2, ativo2 = ncr_habilitado(hwnd)
    print(f"2) depois de esconder/mostrar: {ativo2} (hr=0x{hr2 & 0xFFFFFFFF:08x})")
    ok2 = hr2 == 0 and not ativo2

    # sabotagem: o DWM passa a desenhar o NC; a subclasse tem de desfazer isso
    # quando o tema/DWM "mudam" (é o que acontece num RDP, num tema trocado ou
    # quando o DWM reinicia).
    poe_politica(hwnd, DWMNCRP_ENABLED)
    time.sleep(0.4)
    _, sabotado = ncr_habilitado(hwnd)
    user32.SendMessageW(hwnd, WM_THEMECHANGED, 0, 0)
    time.sleep(0.6)
    _, depois = ncr_habilitado(hwnd)
    print(f"3) sabotagem: forçado={sabotado} apos WM_THEMECHANGED={depois}")
    ok3 = sabotado and not depois

    st = user32.GetWindowLongPtrW(hwnd, -16) & 0xFFFFFFFF
    print(f"   estilo=0x{st:08x} thickframe={bool(st & 0x00040000)}")
    ok_estilo = bool(st & 0x00040000)

    antes = tamanho(hwnd)
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    keysafe.focar(hwnd)
    time.sleep(0.5)
    px, py = r.right - 3, (r.top + r.bottom) // 2
    e.mover(px, py)
    time.sleep(0.3)
    e.baixo(px, py)
    for i in range(1, 21):
        e.mover(px + i * 4, py)
        time.sleep(0.05)
    e.cima()
    time.sleep(1.5)
    depois_tam = tamanho(hwnd)
    print(f"4) arrastar a borda direita: {antes} -> {depois_tam}")
    ok4 = depois_tam[0] > antes[0]

    ok = ok1 and ok2 and ok3 and ok_estilo and ok4
    print("RESULTADO:", "OK" if ok else "FALHOU")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
