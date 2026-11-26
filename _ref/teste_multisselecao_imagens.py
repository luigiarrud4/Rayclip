"""Duas imagens diferentes no multi-select: o clipboard recebe dois arquivos?

Antes da correção, o nome do arquivo materializado em `<dados>/drag/` era
`Rayclip image <data-hora até o segundo>`. Duas imagens copiadas no mesmo
segundo davam o **mesmo** nome, e a segunda reusava o arquivo da primeira — o
`CF_HDROP` saía com dois caminhos iguais e o app de destino colava a mesma
imagem duas vezes.

Este teste chama `paste_files` pelo IPC com dois ids de imagem **de conteúdo
diferente** e lê o `CF_HDROP` do clipboard: os dois caminhos têm de ser
diferentes e cada arquivo tem de ter os bytes da sua entrada.

Uso (app aberto com RAYCLIP_CDP=1):
    python _ref/teste_multisselecao_imagens.py
"""
import ctypes
import json
import sys
import time
from ctypes import wintypes as w

sys.path.insert(0, "_ref")
import teste_fundo
import teste_tray_clique as C

user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32
kernel32 = ctypes.windll.kernel32

CF_HDROP = 15
GMEM_MOVEABLE = 0x0002

user32.OpenClipboard.argtypes = [w.HWND]
user32.GetClipboardData.argtypes = [w.UINT]
user32.GetClipboardData.restype = w.HANDLE
shell32.DragQueryFileW.argtypes = [w.HANDLE, w.UINT, w.LPWSTR, w.UINT]
shell32.DragQueryFileW.restype = w.UINT


def ler_hdrop():
    if not user32.OpenClipboard(None):
        return []
    try:
        h = user32.GetClipboardData(CF_HDROP)
        if not h:
            return []
        total = shell32.DragQueryFileW(h, 0xFFFFFFFF, None, 0)
        nomes = []
        for i in range(total):
            tam = shell32.DragQueryFileW(h, i, None, 0)
            buf = ctypes.create_unicode_buffer(tam + 1)
            shell32.DragQueryFileW(h, i, buf, tam + 1)
            nomes.append(buf.value)
        return nomes
    finally:
        user32.CloseClipboard()


def main():
    hw = C.achar()
    if not hw:
        raise SystemExit("rayclip nao esta aberto")
    s = teste_fundo.Sessao()

    # `list_entries` devolve um **array** de entradas.
    entradas = json.loads(
        s.js(
            "(() => window.__TAURI_INTERNALS__.invoke('list_entries', {})"
            ".then(r => JSON.stringify(r.filter(e => e.hasImage).map(e => ({id: e.id, hash: e.hash}))))"
            ".catch(e => JSON.stringify({erro: String(e)})))()"
        )
    )
    if isinstance(entradas, dict):
        raise SystemExit("list_entries falhou: %s" % entradas)
    # two images with different hashes
    vistas, distintas = set(), []
    for e in entradas:
        if e["hash"] in vistas:
            continue
        vistas.add(e["hash"])
        distintas.append(e)
        if len(distintas) == 2:
            break
    if len(distintas) < 2:
        raise SystemExit("preciso de duas imagens diferentes no historico; achei %d" % len(distintas))
    print("imagens escolhidas:", [e["id"][:8] for e in distintas])

    ids = [e["id"] for e in distintas]
    r = s.js(
        "(async () => { try { const n = await window.__TAURI_INTERNALS__.invoke('paste_files', { ids: %s });"
        " return 'ok:' + n } catch (err) { return 'erro:' + err } })()"
        % json.dumps(ids)
    )
    print("paste_files ->", r)
    time.sleep(0.8)

    caminhos = ler_hdrop()
    print("CF_HDROP (%d):" % len(caminhos))
    for p in caminhos:
        print("  ", p)
    if len(caminhos) < 2:
        print("RESULTADO: FALHOU - clipboard sem os dois arquivos")
        return 1
    if len(set(caminhos)) < len(caminhos):
        print("RESULTADO: FALHOU - caminho repetido (a mesma imagem duas vezes)")
        return 1
    print("RESULTADO: OK - dois arquivos diferentes")

    with open(caminhos[0], "rb") as f:
        a = f.read()
    with open(caminhos[1], "rb") as f:
        b = f.read()
    print("bytes: %d e %d | iguais: %s" % (len(a), len(b), a == b))
    if a == b:
        print("RESULTADO: FALHOU - os dois arquivos tem o mesmo conteudo")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
