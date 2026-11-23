"""Coleta erros de CSP/console da pagina via CDP e confere imgs carregadas."""
import io
import json
import sys
import time
import urllib.request

import websocket  # type: ignore


def alvo():
    for _ in range(40):
        try:
            with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=2) as r:
                for t in json.load(r):
                    if t.get("type") == "page":
                        return t["webSocketDebuggerUrl"]
        except Exception:
            pass
        time.sleep(0.5)
    return None


def main():
    url = alvo()
    if not url:
        print("sem alvo CDP")
        return 1
    ws = websocket.create_connection(url, timeout=20)
    i = 0

    def send(method, params=None):
        nonlocal i
        i += 1
        ws.send(json.dumps({"id": i, "method": method, "params": params or {}}))
        return i

    def wait(mid, limite=15):
        fim = time.time() + limite
        while time.time() < fim:
            msg = json.loads(ws.recv())
            if msg.get("id") == mid:
                return msg
            if msg.get("method") == "Log.entryAdded":
                e = msg["params"]["entry"]
                print("LOG", e.get("source"), e.get("level"), e.get("text")[:300])
        return None

    send("Log.enable")
    send("Runtime.enable")
    wait(i, 5)
    # recarrega a lista para exercitar imagens + IPC sob a CSP nova
    js = """
      (async () => {
        const { invoke } = window.__TAURI_INTERNALS__;
        await invoke('list_entries', { offset: 0, limit: 60 });
        const imgs = [...document.querySelectorAll('img')];
        return JSON.stringify({
          imgs: imgs.length,
          quebradas: imgs.filter(i => i.complete && i.naturalWidth === 0).map(i => i.src),
          fontes: [...new Set(imgs.map(i => i.src.split('/')[2]))],
        });
      })()
    """
    mid = send("Runtime.evaluate", {"expression": js, "awaitPromise": True, "returnByValue": True})
    resp = wait(mid, 20)
    time.sleep(1.2)
    ws.settimeout(1.5)
    csp = []
    try:
        while True:
            msg = json.loads(ws.recv())
            if msg.get("method") == "Log.entryAdded":
                e = msg["params"]["entry"]
                texto = e.get("text", "")
                if "Content Security Policy" in texto or "Refused" in texto:
                    csp.append(texto[:300])
    except Exception:
        pass
    try:
        val = resp["result"]["result"]["value"]
        print("IMGS", val)
    except Exception:
        print("sem resposta:", json.dumps(resp)[:400])
    print("CSP", len(csp))
    for c in csp:
        print("  ", c)
    return 0


if __name__ == "__main__":
    sys.exit(main())
