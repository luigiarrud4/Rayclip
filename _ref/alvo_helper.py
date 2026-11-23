"""Janela alvo do harness, rodando em integridade **média**.

Por que um processo separado: o Raycast (integridade média) não consegue
digitar numa janela de processo elevado (UIPI). O harness roda elevado (para
poder fechar/reabrir o Rayclip de release), então a janela onde o texto é
colado precisa morar num processo de integridade média — este.

Conversa por arquivos em `_ref/`:

  alvo_cmd.txt    harness -> helper   ("focus", "clear", "mark <qpc>")
  alvo_state.txt  helper -> harness   ("T <qpc> <repr do texto>")

`time.perf_counter()` no Windows é QueryPerformanceCounter, que é global do
sistema: os dois processos medem na mesma linha do tempo.
"""

import ctypes
import ctypes.wintypes as w
import os
import sys
import time

REF = os.path.dirname(os.path.abspath(__file__))
CMD = os.path.join(REF, "alvo_cmd.txt")
STATE = os.path.join(REF, "alvo_state.txt")

user32 = ctypes.WinDLL("user32", use_last_error=True)
LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, w.HWND, w.UINT, w.WPARAM, w.LPARAM)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [
        ("style", w.UINT), ("lpfnWndProc", WNDPROC), ("cbClsExtra", ctypes.c_int),
        ("cbWndExtra", ctypes.c_int), ("hInstance", w.HINSTANCE),
        ("hIcon", w.HICON), ("hCursor", w.HANDLE), ("hbrBackground", w.HBRUSH),
        ("lpszMenuName", w.LPCWSTR), ("lpszClassName", w.LPCWSTR),
    ]


user32.CreateWindowExW.restype = w.HWND
user32.CreateWindowExW.argtypes = [
    w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD, ctypes.c_int, ctypes.c_int,
    ctypes.c_int, ctypes.c_int, w.HWND, w.HMENU, w.HINSTANCE, w.LPVOID,
]
user32.DefWindowProcW.restype = LRESULT
user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.SetWindowTextW.argtypes = [w.HWND, w.LPCWSTR]
user32.SetFocus.argtypes = [w.HWND]
user32.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]

WM_APP_FOCUS = 0x8000
PM_REMOVE = 1

edit_hwnd = None
win_hwnd = None


def proc(hwnd, msg, wp, lp):
    if msg in (0x0007, 0x0006):  # WM_SETFOCUS / WM_ACTIVATE
        user32.SetFocus(edit_hwnd)
        return 0
    if msg == WM_APP_FOCUS:
        user32.SetForegroundWindow(hwnd)
        user32.SetFocus(edit_hwnd)
        return 0
    return user32.DefWindowProcW(hwnd, msg, wp, lp)


def text():
    buf = ctypes.create_unicode_buffer(2048)
    user32.GetWindowTextW(edit_hwnd, buf, 2048)
    return buf.value


def main():
    global edit_hwnd, win_hwnd
    keep = WNDPROC(proc)
    cls = WNDCLASSW()
    cls.lpfnWndProc = keep
    cls.lpszClassName = "RayclipHarnessAlvo"
    cls.hbrBackground = 6
    user32.RegisterClassW(ctypes.byref(cls))
    win_hwnd = user32.CreateWindowExW(
        0, "RayclipHarnessAlvo", "Rayclip Harness Alvo", 0x00CF0000,
        140, 140, 560, 250, None, None, cls.hInstance, None,
    )
    edit_hwnd = user32.CreateWindowExW(
        0, "EDIT", "", 0x50000044, 10, 10, 530, 200,
        win_hwnd, None, cls.hInstance, None,
    )
    user32.ShowWindow(win_hwnd, 5)
    # pythonw.exe nao tem stderr (sys.stderr is None): escrever sem checar
    # derrubava o helper antes de qualquer medida.
    if sys.stderr is not None:
        sys.stderr.write("alvo pronto hwnd=%s edit=%s\n" % (win_hwnd, edit_hwnd))

    msg = ctypes.create_string_buffer(64)
    last_text = None
    last_cmd = ""
    last_beat = 0.0
    last_stamp = 0.0
    last_beat = 0.0
    last_stamp = 0.0
    with open(STATE, "w", encoding="utf-8") as f:
        f.write("T 0 ''\n")

    while True:
        while user32.PeekMessageW(msg, None, 0, 0, PM_REMOVE):
            user32.TranslateMessage(msg)
            user32.DispatchMessageW(msg)

        try:
            with open(CMD, "r", encoding="utf-8") as f:
                cmd = f.read().strip()
        except OSError:
            cmd = ""
        if cmd and cmd != last_cmd:
            last_cmd = cmd
            if cmd.startswith("focus"):
                user32.ShowWindow(win_hwnd, 5)
                user32.PostMessageW(win_hwnd, WM_APP_FOCUS, 0, 0)
            elif cmd.startswith("clear"):
                user32.SetWindowTextW(edit_hwnd, "")
            # "mark <qpc>" não precisa de ação: o harness só guarda o instante

        cur = text()
        agora = time.perf_counter()
        # Grava quando o texto muda (é o que o harness mede) e a cada 0,5 s de
        # qualquer jeito: o batimento diz ao harness que o helper está vivo.
        if cur != last_text:
            last_text = cur
            last_stamp = agora
            last_beat = agora
            gravar = True
        elif agora - last_beat > 0.5:
            last_beat = agora
            gravar = True
        else:
            gravar = False
        if gravar:
            try:
                with open(STATE, "w", encoding="utf-8") as f:
                    f.write("T %r %r\n" % (last_stamp, cur))
            except OSError:
                pass
        time.sleep(0.002)


if __name__ == "__main__":
    main()
