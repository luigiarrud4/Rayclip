"""Simula gerenciadores de senha e confere se o Rayclip guarda o conteudo.

Para cada cenario: roda o fake_pm.exe com um nome de processo diferente, espera
o Rayclip processar e verifica se o texto unico entrou no historico.
"""
import io
import json
import os
import shutil
import subprocess
import sys
import time
import uuid

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

TEMP = os.environ["TEMP"]
BASE = os.path.join(TEMP, "pmtest", "base.exe")


def cdp_call(expr, espera=30):
    import urllib.request
    import websocket

    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=5) as r:
        url = [t for t in json.load(r) if t.get("type") == "page"][0]["webSocketDebuggerUrl"]
    ws = websocket.create_connection(url, timeout=espera)
    ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                        "params": {"expression": expr, "awaitPromise": True, "returnByValue": True}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == 1:
            ws.close()
            return m["result"]["result"].get("value")


def tem_entrada(marca):
    js = """
    (async () => {
      const { invoke } = window.__TAURI_INTERNALS__;
      const l = await invoke('list_entries', { offset: 0, limit: 2000 });
      const achou = l.filter(e => (e.text||'').includes(%s) || (e.title||'').includes(%s));
      return JSON.stringify(achou.map(e => ({ id: e.id.slice(0,8), fonte: e.source || '' })));
    })()
    """ % (json.dumps(marca), json.dumps(marca))
    return json.loads(cdp_call(js) or "[]")


def set_capture_sensitive(valor):
    js = """
    (async () => {
      const { invoke } = window.__TAURI_INTERNALS__;
      const s = await invoke('get_settings');
      s.captureSensitive = %s;
      await invoke('save_settings', { settings: s });
      const d = await invoke('get_settings');
      return JSON.stringify({ agora: d.captureSensitive });
    })()
    """ % ("true" if valor else "false")
    return cdp_call(js)


def rodar(nome_exe, modo, texto):
    exe = os.path.join(TEMP, "pmtest", nome_exe)
    shutil.copyfile(BASE, exe)
    subprocess.run([exe, modo, texto], capture_output=True, timeout=30)
    # o listener do Rayclip + OCR/metadata levam um instante
    time.sleep(2.0)
    achou = tem_entrada(texto)
    return achou


CASOS = [
    ("Bitwarden.exe", "text", "senha bitwarden", False),
    ("1Password.exe", "text", "senha 1password", False),
    ("KeePassXC.exe", "text", "senha keepassxc", False),
    ("NordPass.exe", "text", "senha nordpass", False),
    ("WindowsPasswordManager.exe", "text", "senha gerenciador do windows", False),
    ("MeuPasswordVault.exe", "text", "senha vault generico", False),
    ("bloco_de_notas.exe", "text", "texto normal do bloco", True),
    ("bloco_de_notas.exe", "exclude", "texto com ExcludeClipboard", False),
    ("bloco_de_notas.exe", "viewer", "texto com ViewerIgnore", False),
    ("bloco_de_notas.exe", "caninclude0", "texto CanInclude 0", False),
    ("bloco_de_notas.exe", "caninclude1", "texto CanInclude 1", True),
]


def main():
    print("captureSensitive inicial:", set_capture_sensitive(False))
    falhas = []
    for exe, modo, base, esperado in CASOS:
        marca = f"{base} {uuid.uuid4().hex[:6]}"
        achou = rodar(exe, modo, marca)
        entrou = bool(achou)
        ok = entrou == esperado
        print(f"{'OK ' if ok else 'FALHA'} {exe:22} {modo:12} entrou={entrou!s:5} esperado={esperado}")
        if not ok:
            falhas.append((exe, modo, entrou))

    # com captureSensitive ligado, o gerenciador de senha deve ser guardado
    print("ligando captureSensitive:", set_capture_sensitive(True))
    marca = f"senha bitwarden liberada {uuid.uuid4().hex[:6]}"
    achou = rodar("Bitwarden.exe", "text", marca)
    ok = bool(achou)
    print(f"{'OK ' if ok else 'FALHA'} captureSensitive=true + Bitwarden entrou={ok}")
    if not ok:
        falhas.append(("Bitwarden.exe", "text/captureSensitive", ok))
    print("voltando captureSensitive:", set_capture_sensitive(False))
    print("FALHAS:", falhas if falhas else "nenhuma")
    return 0 if not falhas else 1


if __name__ == "__main__":
    sys.exit(main())
