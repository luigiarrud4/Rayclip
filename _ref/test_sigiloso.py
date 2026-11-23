"""Prova que o filtro de conteúdo sigiloso respeita o toggle do Settings.

Escreve no clipboard um texto marcado com
`ExcludeClipboardContentFromMonitorProcessing` (o que navegador em aba anônima
e gerenciador de senha fazem) e vê se o app guarda ou não.
"""
import sys, time, ctypes, ctypes.wintypes as w
sys.path.insert(0, r'C:\Users\Lugia\Projects\rayclip\_ref')
import harness_compare as H
from cdp_wheel import Cdp

u = ctypes.WinDLL('user32', use_last_error=True)
k = ctypes.WinDLL('kernel32', use_last_error=True)
u.RegisterClipboardFormatW.restype = ctypes.c_uint
k.GlobalAlloc.restype = ctypes.c_void_p
k.GlobalAlloc.argtypes = [w.UINT, ctypes.c_size_t]
k.GlobalLock.restype = ctypes.c_void_p
k.GlobalLock.argtypes = [ctypes.c_void_p]
k.GlobalUnlock.argtypes = [ctypes.c_void_p]
u.SetClipboardData.argtypes = [w.UINT, ctypes.c_void_p]
EXCL = u.RegisterClipboardFormatW(ctypes.c_wchar_p('ExcludeClipboardContentFromMonitorProcessing'))


def set_secret(text):
    data = (text + '\0').encode('utf-16-le')
    for _ in range(30):
        if u.OpenClipboard(None):
            break
        time.sleep(0.01)
    u.EmptyClipboard()
    h = k.GlobalAlloc(2, len(data))
    p = k.GlobalLock(h)
    ctypes.memmove(p, data, len(data))
    k.GlobalUnlock(h)
    u.SetClipboardData(13, h)
    # marcador de "não monitore isto" presente no clipboard
    u.SetClipboardData(EXCL, k.GlobalAlloc(2, 1))
    u.CloseClipboard()


def existe(c, texto):
    r = c.js("""(async()=>{const l=await window.__TAURI_INTERNALS__.invoke('list_entries');
        return l.filter(e=>(e.title||'').includes(%r)).length;})()""" % texto)
    return r


c = Cdp()
try:
    # estado inicial: filtro ligado (padrão)
    cfg = c.js("window.__TAURI_INTERNALS__.invoke('get_settings')")
    print('captureSensitive inicial =', cfg.get('captureSensitive'))

    marca1 = 'segredo-bloqueado-%d' % int(time.time())
    set_secret(marca1)
    time.sleep(1.2)
    print('com filtro ligado -> guardou?', existe(c, marca1))

    cfg['captureSensitive'] = True
    c.js("window.__TAURI_INTERNALS__.invoke('save_settings',{settings:%s})" % __import__('json').dumps(cfg))
    time.sleep(0.6)
    print('captureSensitive agora =',
          c.js("window.__TAURI_INTERNALS__.invoke('get_settings')").get('captureSensitive'))

    marca2 = 'segredo-permitido-%d' % int(time.time())
    set_secret(marca2)
    time.sleep(1.2)
    print('com filtro desligado -> guardou?', existe(c, marca2))

    # volta ao padrão
    cfg['captureSensitive'] = False
    c.js("window.__TAURI_INTERNALS__.invoke('save_settings',{settings:%s})" % __import__('json').dumps(cfg))
    print('voltou para', c.js("window.__TAURI_INTERNALS__.invoke('get_settings')").get('captureSensitive'))
finally:
    c.close()
