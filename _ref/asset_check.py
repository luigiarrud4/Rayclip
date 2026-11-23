import io, json, sys, time, urllib.request
import websocket

def alvo():
    with urllib.request.urlopen("http://127.0.0.1:9222/json", timeout=3) as r:
        for t in json.load(r):
            if t.get("type") == "page":
                return t["webSocketDebuggerUrl"]
    return None

ws = websocket.create_connection(alvo(), timeout=25)
js = """
(async () => {
  const { invoke, convertFileSrc } = window.__TAURI_INTERNALS__;
  const lista = await invoke('list_entries', { offset: 0, limit: 200 });
  const comIcone = lista.find(e => e.sourceIcon);
  if (!comIcone) return JSON.stringify({ erro: 'nenhuma entrada com sourceIcon' });
  const src = convertFileSrc(comIcone.sourceIcon.split(String.fromCharCode(92)).join('/'));
  const ok = await new Promise(res => {
    const img = new Image();
    img.onload = () => res(img.naturalWidth > 0);
    img.onerror = () => res(false);
    img.src = src;
  });
  return JSON.stringify({ caminho: comIcone.sourceIcon, url: src, carregou: ok });
})()
"""
ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                    "params": {"expression": js, "awaitPromise": True, "returnByValue": True}}))
while True:
    msg = json.loads(ws.recv())
    if msg.get("id") == 1:
        print(json.dumps(msg["result"], ensure_ascii=False)[:600])
        break
