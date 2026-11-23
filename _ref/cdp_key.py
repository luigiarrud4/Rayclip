"""Envia teclas de verdade pelo pipeline de input do WebView2 (Ctrl+C etc.)."""

import json
import sys
import urllib.request

from websockets.sync.client import connect


def page():
    return next(
        t for t in json.load(urllib.request.urlopen("http://127.0.0.1:9222/json"))
        if t["type"] == "page"
    )


def key(ws, kind: str, code: str, key_text: str, modifiers: int = 0, vk: int = 0):
    params = {
        "type": kind,
        "code": code,
        "key": key_text,
        "windowsVirtualKeyCode": vk,
        "modifiers": modifiers,
    }
    ws.send(json.dumps({"id": 99, "method": "Input.dispatchKeyEvent", "params": params}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == 99:
            return


def main():
    with connect(page()["webSocketDebuggerUrl"], max_size=8 * 1024 * 1024) as ws:
        # Ctrl+C: modifiers 2 = Ctrl
        key(ws, "rawKeyDown", "KeyC", "c", modifiers=2, vk=0x43)
        key(ws, "keyUp", "KeyC", "c", modifiers=2, vk=0x43)
        print("ctrl+c enviado")


if __name__ == "__main__":
    main()
