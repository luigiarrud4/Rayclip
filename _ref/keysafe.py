"""Teclado sintetico com trava de foco.

O motivo deste arquivo: `SendInput` manda a tecla para a **janela em foco**, seja
ela qual for. Os roteiros daqui mandavam Esc / Alt+Espaco / setas sem conferir o
foco — quando o app de destino nao abria (ou ja estava fechado), as teclas caíam
no console que hospeda o agente e a sessao era cancelada (Esc/Alt+Espaco no
console). Regra: so enviar tecla depois de confirmar que o foco esta no processo
certo; texto nunca vai para o console por acidente.
"""
import ctypes
import time
from ctypes import wintypes as w

user32 = ctypes.WinDLL("user32", use_last_error=True)
user32.GetForegroundWindow.restype = w.HWND
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
user32.IsWindowVisible.argtypes = [w.HWND]


def pid_do_foco():
    h = user32.GetForegroundWindow()
    if not h:
        return None, None
    p = w.DWORD()
    user32.GetWindowThreadProcessId(h, ctypes.byref(p))
    return h, p.value


user32.AttachThreadInput.argtypes = [w.DWORD, w.DWORD, w.BOOL]
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
kernel32.GetCurrentThreadId.restype = w.DWORD


def focar(hwnd, espera=0.6):
    """Traz a janela para frente e confirma que o foco ficou com ela.

    `SetForegroundWindow` sozinho falha quando o processo chamador nao e o dono
    do foco (o Windows bloqueia troca de foco "roubada"). O jeito que funciona e
    anexar a thread ao thread da janela em foco, trocar e desanexar.
    """
    user32.ShowWindow(hwnd, 5)  # SW_SHOW
    fim = time.time() + espera
    while time.time() < fim:
        atual = user32.GetForegroundWindow()
        if atual == hwnd:
            return True
        t_atual = user32.GetWindowThreadProcessId(atual, None) if atual else 0
        t_minha = kernel32.GetCurrentThreadId()
        if t_atual and t_atual != t_minha:
            user32.AttachThreadInput(t_atual, t_minha, True)
        user32.SetForegroundWindow(hwnd)
        user32.SetFocus(hwnd)
        if t_atual and t_atual != t_minha:
            user32.AttachThreadInput(t_atual, t_minha, False)
        time.sleep(0.1)
    return user32.GetForegroundWindow() == hwnd


def teclas_seguro(pid, send_keys, *teclas, gap=0.012):
    """So envia se a janela em foco pertence a `pid`; senao levanta erro.

    Nunca digita as cegas: sem essa checagem, uma tecla perdida ia parar no
    console do agente (foi o que derrubou varias operacoes)."""
    _, pid_foco = pid_do_foco()
    if pid_foco != pid:
        raise RuntimeError(
            f"foco fora do app (foco={pid_foco}, esperado={pid}); teclas NAO enviadas"
        )
    send_keys(*teclas, gap=gap)


def texto_seguro(pid, type_text, texto):
    _, pid_foco = pid_do_foco()
    if pid_foco != pid:
        raise RuntimeError(
            f"foco fora do app (foco={pid_foco}, esperado={pid}); texto NAO digitado"
        )
    type_text(texto)
