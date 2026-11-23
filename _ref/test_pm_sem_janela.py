"""Casos sem janela: o Windows nao sabe quem copiou (GetClipboardOwner == NULL).

Mostra o limite conhecido — sem dono identificavel, o nome do processo nao pode
ser checado — e que os marcadores (ExcludeClipboardContentFromMonitorProcessing)
continuam protegendo.
"""
import io
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
import urllib.request

import websocket

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

TEMP = os.environ["TEMP"]
BASE = os.path.join(TEMP, "pmtest", "base.exe")


def cdp(expr):
    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=5) as r:
        url = [t for t in json.load(r) if t.get("type") == "page"][0]["webSocketDebuggerUrl"]
    ws = websocket.create_connection(url, timeout=40)
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
      return JSON.stringify(l.filter(e => (e.text||'').includes(%s)).map(e => e.id.slice(0,8)));
    })()
    """ % json.dumps(marca)
    return json.loads(cdp(js) or "[]")


def rodar(exe_nome, modo, texto):
    exe = os.path.join(TEMP, "pmtest", exe_nome)
    shutil.copyfile(BASE, exe)
    subprocess.run([exe, modo, texto], capture_output=True, timeout=30)
    time.sleep(2.0)
    return tem_entrada(texto)


casos = [
    ("Bitwarden.exe", "nowin_text", "senha copiada sem janela", None),
    ("Bitwarden.exe", "nowin_exclude", "senha sem janela com marcador", False),
    ("bloco_de_notas.exe", "nowin_text", "texto comum sem janela", True),
]
for exe, modo, base, esperado in casos:
    marca = f"{base} {uuid.uuid4().hex[:6]}"
    achou = rodar(exe, modo, marca)
    print(f"{exe:22} {modo:14} entrou={bool(achou)!s:5} esperado={esperado}")
