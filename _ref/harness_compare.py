"""Harness de comparação: Rayclip x Raycast, no mesmo Windows, medindo o mesmo.

O que ele mede (nada de achismo, tudo com cronômetro):

  open     atalho global -> janela visível              (IsWindowVisible)
  paste    Enter -> o texto chega na janela alvo        (GetWindowTextW do EDIT)
  capture  cópia -> o item já está no histórico         (abre, Enter, confere o texto)

A janela alvo é criada por este script (janela Win32 com um EDIT): assim o
texto colado é lido com precisão e nenhum app de verdade é sujado com as
strings de teste.

Uso (rodar de /c/Users/Lugia/Projects/rayclip):

    python _ref/harness_compare.py            # tudo, 5 repetições
    python _ref/harness_compare.py open       # só abertura
    python _ref/harness_compare.py paste      # só colagem
    python _ref/harness_compare.py capture    # só captura

Observações importantes:
- O Rayclip precisa estar rodando em build **debug** (asInvoker): o binário de
  release é elevado e o UIPI descarta teclas sintéticas.
- O Raycast precisa estar aberto (ele escuta Ctrl+Shift+V globalmente).
"""

import ast
import ctypes
import ctypes.wintypes as w
import os
import statistics
import subprocess
import sys
import threading
import time

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

# ---------------------------------------------------------------- protótipos
LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, w.HWND, w.UINT, w.WPARAM, w.LPARAM)
WNDENUMPROC = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [
        ("style", w.UINT),
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


user32.CreateWindowExW.restype = w.HWND
user32.CreateWindowExW.argtypes = [
    w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD,
    ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    w.HWND, w.HMENU, w.HINSTANCE, w.LPVOID,
]
user32.RegisterClassW.restype = w.ATOM
user32.RegisterClassW.argtypes = [ctypes.POINTER(WNDCLASSW)]
user32.DefWindowProcW.restype = LRESULT
user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.SetWindowTextW.argtypes = [w.HWND, w.LPCWSTR]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.EnumWindows.argtypes = [WNDENUMPROC, w.LPARAM]
user32.GetWindowThreadProcessId.argtypes = [w.HWND, ctypes.POINTER(w.DWORD)]
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, ctypes.c_int]
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.SetFocus.argtypes = [w.HWND]
user32.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
user32.GetWindowRect.argtypes = [w.HWND, ctypes.c_void_p]
user32.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.GetForegroundWindow.restype = w.HWND
user32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
user32.DispatchMessageW.argtypes = [ctypes.c_void_p]
user32.PeekMessageW.argtypes = [ctypes.c_void_p, w.HWND, w.UINT, w.UINT, w.UINT]
user32.TranslateMessage.argtypes = [ctypes.c_void_p]
user32.SetTimer.argtypes = [w.HWND, ctypes.c_size_t, w.UINT, w.HANDLE]
user32.OpenClipboard.argtypes = [w.HWND]
user32.SetClipboardData.argtypes = [w.UINT, w.HANDLE]

kernel32.OpenProcess.restype = w.HANDLE
kernel32.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
kernel32.QueryFullProcessImageNameW.argtypes = [
    w.HANDLE, w.DWORD, w.LPWSTR, ctypes.POINTER(w.DWORD)
]
kernel32.GlobalAlloc.restype = w.HANDLE
kernel32.GlobalAlloc.argtypes = [w.UINT, ctypes.c_size_t]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalLock.argtypes = [w.HANDLE]
kernel32.GlobalUnlock.argtypes = [w.HANDLE]
kernel32.CloseHandle.argtypes = [w.HANDLE]

WM_APP_FOCUS = 0x8000
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
GMEM_MOVEABLE = 0x0002
CF_UNICODETEXT = 13
PM_REMOVE = 1


def proc_name(pid):
    h = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not h:
        return ""
    try:
        buf = ctypes.create_unicode_buffer(600)
        n = w.DWORD(600)
        if kernel32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)):
            return buf.value.rsplit("\\", 1)[-1].lower()
    finally:
        kernel32.CloseHandle(h)
    return ""


def class_of(hwnd):
    buf = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, buf, 256)
    return buf.value


def find_window(exe, class_hint=""):
    """Primeira janela visível do processo `exe` (opcionalmente por classe)."""
    found = []

    def cb(hwnd, _l):
        if not user32.IsWindowVisible(hwnd):
            return True
        pid = w.DWORD(0)
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value == 0:
            return True
        if proc_name(pid.value) != exe:
            return True
        if class_hint and class_hint not in class_of(hwnd):
            return True
        r = w.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(r))
        if r.right - r.left < 200 or r.bottom - r.top < 80:
            return True  # splash/bandeja
        found.append(hwnd)
        return False

    user32.EnumWindows(WNDENUMPROC(cb), 0)
    return found[0] if found else None


# --------------------------------------------------------------- clipboard
def set_clipboard_text(text):
    data = (text + "\0").encode("utf-16-le")
    for _ in range(30):
        if user32.OpenClipboard(None):
            break
        time.sleep(0.01)
    else:
        raise RuntimeError("clipboard ocupado")
    try:
        user32.EmptyClipboard()
        h = kernel32.GlobalAlloc(GMEM_MOVEABLE, len(data))
        p = kernel32.GlobalLock(h)
        ctypes.memmove(p, data, len(data))
        kernel32.GlobalUnlock(h)
        user32.SetClipboardData(CF_UNICODETEXT, h)
    finally:
        user32.CloseClipboard()


# ------------------------------------------------------------------- input
class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", w.WORD), ("wScan", w.WORD), ("dwFlags", w.DWORD),
        ("time", w.DWORD), ("dwExtraInfo", ctypes.c_size_t),
    ]


class _INPUTunion(ctypes.Union):
    _fields_ = [("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", w.DWORD), ("u", _INPUTunion)]


user32.SendInput.argtypes = [w.UINT, ctypes.POINTER(INPUT), ctypes.c_int]

VK = {"alt": 0x12, "ctrl": 0x11, "shift": 0x10, "space": 0x20, "enter": 0x0D,
      "esc": 0x1B, "delete": 0x2E, "backspace": 0x08, "tab": 0x09,
      "left": 0x25, "up": 0x26, "right": 0x27, "down": 0x28}


user32.MapVirtualKeyW.restype = ctypes.c_uint


def _key(vk, up):
    # wScan importa: o WPF do Raycast ignorava o Enter quando o scan code era
    # zero (tecla "virtual" sem hardware correspondente).
    scan = user32.MapVirtualKeyW(vk, 0) & 0xFF
    ev = INPUT(type=1)
    ev.u.ki = KEYBDINPUT(wVk=vk, wScan=scan, dwFlags=0x0002 if up else 0, time=0,
                         dwExtraInfo=0)
    return ev


# O SendInput vai para a JANELA EM FOCO. Se o foco estiver no console que hospeda
# o agente, Esc/Alt+Espaço cancelam a operação em curso (foi o que abortava as
# rodadas longas). Só enviamos tecla quando o foco é de quem deveria receber.
FOCOS_PERMITIDOS = {"rayclip.exe", "raycast.exe", "pythonw.exe", "python.exe"}


def pid_em_foco():
    h = user32.GetForegroundWindow()
    if not h:
        return 0
    p = w.DWORD()
    user32.GetWindowThreadProcessId(h, ctypes.byref(p))
    return p.value


def foco_seguro():
    pid = pid_em_foco()
    nome = proc_name(pid) if pid else ""
    return nome in FOCOS_PERMITIDOS, nome


# Janela neutra (alvo do harness). Se o foco estiver em algo estranho (console do
# agente, por exemplo), o foco vai para ela antes de qualquer tecla: e uma janela
# nossa, um "colar"/Esc ali nao faz nada.
ALVO_HWND = None


def send_keys(*names, gap=0.012, forcar=False):
    """Combinação inteira num SendInput, com um respiro entre apertar e soltar.

    Sem o respiro o WPF do Raycast às vezes não enxerga o Esc (o app dele fica
    aberto e a próxima medição sairia errada)."""
    ok, nome = foco_seguro()
    if not ok and not forcar:
        if ALVO_HWND:
            import keysafe
            keysafe.focar(ALVO_HWND, espera=0.25)
            ok, nome = foco_seguro()
        if not ok:
            print(f"  aviso: foco em '{nome or '?'}' — teclas {names} NAO enviadas",
                  flush=True)
            return False
    vks = [VK[n.lower()] if n.lower() in VK else ord(n.upper()) for n in names]
    downs = (INPUT * len(vks))(*[_key(v, False) for v in vks])
    ups = (INPUT * len(vks))(*[_key(v, True) for v in reversed(vks)])
    user32.SendInput(len(vks), downs, ctypes.sizeof(INPUT))
    if gap:
        time.sleep(gap)
    user32.SendInput(len(vks), ups, ctypes.sizeof(INPUT))
    return True


def send_down(*names, forcar=False):
    """Manda só o toque das teclas e devolve o instante do SendInput.

    O atalho global dispara no **key down**; medir a partir daqui evita somar o
    respiro do `send_keys` (12 ms) ao número do app.
    """
    ok, nome = foco_seguro()
    if not ok and not forcar:
        if ALVO_HWND:
            import keysafe
            keysafe.focar(ALVO_HWND, espera=0.25)
            ok, nome = foco_seguro()
        if not ok:
            print(f"  aviso: foco em '{nome or '?'}' — teclas {names} NAO enviadas",
                  flush=True)
            return None
    vks = [VK[n.lower()] if n.lower() in VK else ord(n.upper()) for n in names]
    downs = (INPUT * len(vks))(*[_key(v, False) for v in vks])
    user32.SendInput(len(vks), downs, ctypes.sizeof(INPUT))
    return time.perf_counter()


def send_up(*names):
    vks = [VK[n.lower()] if n.lower() in VK else ord(n.upper()) for n in names]
    ups = (INPUT * len(vks))(*[_key(v, True) for v in reversed(vks)])
    user32.SendInput(len(vks), ups, ctypes.sizeof(INPUT))


def click(hwnd):
    """Clique esquerdo de verdade no centro da janela (esconde por perda de foco)."""
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    x = (r.left + r.right) // 2
    y = (r.top + r.bottom) // 2
    user32.SetCursorPos(x, y)
    time.sleep(0.03)
    user32.mouse_event(0x0002, 0, 0, 0, 0)
    time.sleep(0.02)
    user32.mouse_event(0x0004, 0, 0, 0, 0)


# ------------------------------------------------- janela alvo (proxy remoto)
REF = os.path.dirname(os.path.abspath(__file__))
PYW = sys.executable.replace("python.exe", "pythonw.exe")
TASK_NAME = "RayclipHarnessAlvo"


def _task(*args):
    subprocess.run(["schtasks"] + list(args), capture_output=True)


ALVO_CMD = os.path.join(REF, "alvo_cmd.txt")
ALVO_STATE = os.path.join(REF, "alvo_state.txt")


class Alvo:
    """Fala com `alvo_helper.py` (integridade média), que é quem tem a janela.

    O Raycast não consegue digitar numa janela elevada; o helper resolve isso
    sem baixar a integridade do harness (que precisa ser elevado para mexer no
    Rayclip de release).
    """

    elevado = False

    def __init__(self):
        if not self._alive():
            # Tarefa agendada em /RL LIMITED = processo em integridade média
            # sem UAC (o truque do Explorer não funciona a partir de shell
            # elevado nesta máquina). Criada na hora, se ainda não existir.
            _task("/Create", "/TN", TASK_NAME, "/TR",
                  '"%s" "%s"' % (PYW, os.path.join(REF, "alvo_helper.py")),
                  "/SC", "ONCE", "/ST", "23:59", "/RL", "LIMITED", "/F")
            _task("/Run", "/TN", TASK_NAME)
        limite = time.time() + 12
        while time.time() < limite:
            if self._alive():
                return
            time.sleep(0.1)
        # Plano B: sobe o helper na integridade do harness (elevada). Serve
        # para medir o Rayclip; o Raycast não conseguirá colar aqui por UIPI.
        subprocess.Popen([PYW, os.path.join(REF, "alvo_helper.py")])
        limite = time.time() + 12
        while time.time() < limite:
            if self._alive():
                self.elevado = True
                return
            time.sleep(0.1)
        raise RuntimeError("helper da janela alvo nao subiu")

    @staticmethod
    def _alive():
        try:
            return time.time() - os.path.getmtime(ALVO_STATE) < 3.0
        except OSError:
            return False

    def _read_state(self):
        try:
            with open(ALVO_STATE, "r", encoding="utf-8") as f:
                linha = f.read().strip()
        except OSError:
            return None, None
        if not linha.startswith("T "):
            return None, None
        try:
            _, ts, txt = linha.split(" ", 2)
            return float(ts), ast.literal_eval(txt)
        except (ValueError, SyntaxError):
            return None, None

    def _send(self, cmd):
        try:
            with open(ALVO_CMD, "w", encoding="utf-8") as f:
                f.write(cmd + chr(10))
        except OSError:
            pass

    @property
    def hwnd(self):
        # o helper pode ter subido como python.exe ou pythonw.exe (Tarefa
        # Agendada usa pythonw) — tenta as duas formas pela classe da janela
        for exe in ("pythonw.exe", "python.exe"):
            h = find_window(exe, "RayclipHarnessAlvo")
            if h:
                return h
        return find_window("pythonw.exe", "") or find_window("python.exe", "")

    def focus(self):
        self._send("focus")
        time.sleep(0.07)

    def clear(self):
        self._send("clear")
        time.sleep(0.06)

    def text(self):
        return self._read_state()[1] or ""

    def changed_at(self):
        return self._read_state()[0]


# ------------------------------------------------------------------ probes
APPS = {
    "rayclip": {"exe": "rayclip.exe", "hotkey": ("alt", "space")},
    "raycast": {"exe": "raycast.exe", "hotkey": ("ctrl", "shift", "v")},
}


def visible(hwnd):
    return bool(hwnd) and bool(user32.IsWindowVisible(hwnd))


def win_of(name):
    return find_window(APPS[name]["exe"], "Raycast" if name == "raycast" else "")


def type_text(text):
    """Digita um texto curto (letras/números) na busca do app."""
    for ch in text:
        send_keys(ch, gap=0.004)


def make_marker():
    return "hx" + ("%06x" % (int(time.time() * 1000) & 0xFFFFFF))


def paste_via_search(name, alvo, marker, dt_from_enter=True):
    """Abre o app, filtra pelo marcador, dá Enter e confere o que chegou.

    Filtrar é obrigatório: com item fixado (pin) no topo, o "primeiro da
    lista" não é o mais recente — e o teste mediria outra coisa.
    """
    a = alvo
    a.focus()
    a.clear()
    time.sleep(0.10)
    if not close_app(name, alvo):
        return None
    send_keys(*APPS[name]["hotkey"])
    time.sleep(0.45)
    type_text(marker)
    time.sleep(0.25)
    t0 = time.perf_counter()
    send_keys("enter", gap=0.008)
    got = ""
    marco = None
    while (time.perf_counter() - t0) * 1000 < 2500:
        got = a.text()
        if got:
            marco = a.changed_at()
            break
        time.sleep(0.001)
    dt = (time.perf_counter() - t0) * 1000
    if marco:
        # carimbo do helper (QPC global): desconta o polling do harness
        dt = min(dt, (marco - t0) * 1000)
    return (dt, got, marker in got)


def close_app(name, alvo):
    """Fecha o app de verdade: Esc e, se ele insistir, clique fora.

    O Esc tem que ir para a JANELA DO APP: com o foco na janela neutra do
    harness (o normal aqui) o Esc nao fecha nada e todas as medidas seguintes
    mediam um app que ja estava aberto — era o que zerava a coluna do Raycast.
    """
    import keysafe
    for _ in range(3):
        hwnd = win_of(name)
        if hwnd is None:
            return True
        keysafe.focar(hwnd, espera=0.3)
        send_keys("esc")
        time.sleep(0.28)
        if win_of(name) is None:
            return True
        if alvo:
            alvo.focus()
            click(alvo.hwnd)
        time.sleep(0.30)
    return win_of(name) is None


def open_latency(alvo, name, runs=5):
    """alvo pode ser None: a abertura só precisa de foco em alguma janela."""
    cfg = APPS[name]
    out = []
    for _ in range(runs):
        if alvo:
            alvo.focus()
        time.sleep(0.25)
        if not close_app(name, alvo):
            out.append(float("nan"))
            continue
        # o atalho global dispara no key DOWN: medir daqui (antes o respiro de
        # 12 ms do send_keys entrava no número)
        t0 = send_down(*cfg["hotkey"])
        if t0 is None:
            send_up(*cfg["hotkey"])
            out.append(float("nan"))
            continue
        while (time.perf_counter() - t0) * 1000 < 3000:
            if visible(win_of(name)):
                break
            time.sleep(0.0002)
        out.append((time.perf_counter() - t0) * 1000)
        send_up(*cfg["hotkey"])
        time.sleep(0.20)
    close_app(name, alvo)
    return [v for v in out if v == v]


def paste_latency(alvo, name, runs=5):
    out = []
    for _ in range(runs):
        marker = make_marker()
        set_clipboard_text(marker)
        time.sleep(0.45)  # deixa o app capturar
        r = paste_via_search(name, alvo, marker)
        if r is None:
            out.append((float("nan"), False))
            continue
        out.append((r[0], r[2]))
        close_app(name, alvo)
        time.sleep(0.15)
    return out


def capture_latency(alvo, name, waits=(0.03, 0.08, 0.15, 0.30)):
    """Cópia -> o item já está no histórico depois de `wait` ms."""
    out = []
    for wait in waits:
        marker = make_marker()
        set_clipboard_text(marker)
        time.sleep(wait)
        r = paste_via_search(name, alvo, marker)
        out.append((int(wait * 1000), bool(r and r[2])))
        close_app(name, alvo)
        time.sleep(0.15)
    return out


def fmt_ms(vals):
    if not vals:
        return "—"
    return "%.0f / %.0f / %.0f" % (
        statistics.median(vals), min(vals), max(vals)
    )


def main():
    global ALVO_HWND
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    try:
        alvo = Alvo()
    except RuntimeError as e:
        alvo = None
        print("aviso: %s — so a medida de abertura vai rodar" % e)
    if alvo:
        ALVO_HWND = alvo.hwnd  # propriedade, nao metodo
        alvo.focus()
    linhas = []

    def say(s):
        print(s, flush=True)
        linhas.append(s)

    say(f"harness rayclip x raycast — {time.strftime('%d/%m/%Y %H:%M')}")
    for name in APPS:
        hwnd = find_window(APPS[name]["exe"], "Raycast" if name == "raycast" else "")
        say(f"  {name}: exe={APPS[name]['exe']} janela={'sim' if hwnd else 'NAO ENCONTRADA'}")

    if which in ("all", "open"):
        say("\n[open] atalho -> janela visível (ms, mediana/min/max)")
        for name in APPS:
            vals = open_latency(alvo, name)
            say(f"  {name:8s} {fmt_ms(vals)}   {[round(v) for v in vals]}")

    if which in ("all", "paste") and alvo:
        say("\n[paste] Enter -> texto chega no alvo (ms, mediana/min/max)")
        for name in APPS:
            res = paste_latency(alvo, name)
            vals = [r[0] for r in res if r[1]]
            falhas = [r for r in res if not r[1]]
            say(f"  {name:8s} {fmt_ms(vals)}   ok={len(vals)}/{len(res)}"
                + (f"  FALHAS={len(falhas)}" if falhas else ""))

    if which in ("all", "capture") and alvo:
        say("\n[capture] cópia -> item já disponível no histórico")
        for name in APPS:
            res = capture_latency(alvo, name)
            txt = " ".join(f"{w}ms={'ok' if ok else 'NAO'}" for w, ok in res)
            say(f"  {name:8s} {txt}")

    # deixa as duas janelas fechadas: quem roda depois (ou o usuario) nao
    # encontra um app aberto por cima do trabalho
    for name in APPS:
        close_app(name, alvo)

    out = "_ref/compare_resultado.txt"
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(linhas) + "\n")
    print(f"\nsalvo em {out}")


if __name__ == "__main__":
    main()
