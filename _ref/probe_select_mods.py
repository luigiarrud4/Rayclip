"""Ctrl+clique, Shift+clique e setas: a seleção acompanha? (CDP)"""

import json
import time

import probe_select as p

CTRL, SHIFT = 2, 8


def clica_com(c, indice, modifiers=0):
    js = (
        "(() => { const e = [...document.querySelectorAll('[data-id]')][%d];"
        " if (!e) return null; const r = e.getBoundingClientRect();"
        " return JSON.stringify({x: r.x + r.width / 2, y: r.y + r.height / 2,"
        " id: e.dataset.id}); })()" % indice
    )
    alvo = json.loads(c.ev(js))
    for tipo in ("mousePressed", "mouseReleased"):
        c.send(
            "Input.dispatchMouseEvent",
            {
                "type": tipo,
                "x": alvo["x"],
                "y": alvo["y"],
                "button": "left",
                "clickCount": 1,
                "modifiers": modifiers,
            },
        )
    time.sleep(0.6)
    return alvo["id"]


def tecla(c, key, code, key_code):
    for tipo in ("keyDown", "keyUp"):
        c.send(
            "Input.dispatchKeyEvent",
            {"type": tipo, "key": key, "code": code, "windowsVirtualKeyCode": key_code},
        )
    time.sleep(0.4)


def multi(c):
    js = (
        "(() => [...document.querySelectorAll('[data-multi]')]"
        ".map(e => e.dataset.multi + ':' + e.dataset.id.slice(0, 8)))()"
    )
    return c.ev(js)


if __name__ == "__main__":
    c = p.CDP()
    ids = c.ev("(() => [...document.querySelectorAll('[data-id]')].map(e => e.dataset.id))()")
    clica_com(c, 1)
    print("normal  ->", p.estado(c)["sel"][:8], "| esperado", ids[1][:8])
    clica_com(c, 3, CTRL)
    print("ctrl    ->", p.estado(c)["sel"][:8], "| marcados:", multi(c))
    clica_com(c, 5, CTRL)
    print("ctrl    ->", p.estado(c)["sel"][:8], "| marcados:", multi(c))
    clica_com(c, 2, SHIFT)
    print("shift   ->", p.estado(c)["sel"][:8], "| marcados:", multi(c))
    tecla(c, "ArrowDown", "ArrowDown", 40)
    print("seta    ->", p.estado(c)["sel"][:8], "| ids[6] =", ids[6][:8])
    tecla(c, "ArrowUp", "ArrowUp", 38)
    print("seta    ->", p.estado(c)["sel"][:8], "| ids[5] =", ids[5][:8])
