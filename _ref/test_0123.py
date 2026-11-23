"""Verificacao da v0.1.23: reordenar fixados, editar conteudo, copiar cor,
apagar recentes e grupos "This Year"/"Older".

Roda contra o build DEBUG (CDP na 9222). Nao apaga nada de verdade: o teste de
"apagar recentes" so monta a lista que seria apagada.
"""
import io
import json
import sys
import time

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
sys.path.insert(0, r"C:\Users\Lugia\Projects\rayclip\_ref")
from cdp_wheel import Cdp  # noqa: E402

INV = "window.__TAURI_INTERNALS__.invoke"

c = Cdp()


def js(code):
    return c.js(code)


def entries():
    return js("(async()=>await %s('list_entries'))()" % INV)


def pinned_ids(es):
    # A lista ja vem ordenada pelo backend: fixados primeiro, na ordem do usuario.
    return [e["id"] for e in es if e.get("pinned")]


try:
    es = entries()
    print("entradas:", len(es))

    # ---------------------------------------------------------------- fixados
    textos = [e for e in es if e["kind"] == "text" and len(e.get("text") or "") > 3][:2]
    if len(textos) < 2:
        print("!! poucas entradas de texto para o teste de fixados")
    else:
        a, b = textos[0]["id"], textos[1]["id"]
        js("(async()=>await %s('toggle_pin',{id:%r}))()" % (INV, a))
        js("(async()=>await %s('toggle_pin',{id:%r}))()" % (INV, b))
        ordem = pinned_ids(entries())
        print("depois de fixar b+por ultimo:", ordem[:4])
        assert ordem[0] == b and ordem[1] == a, "pin novo deveria ir para o topo"

        js("(async()=>await %s('move_pinned',{id:%r,up:true}))()" % (INV, a))
        ordem2 = pinned_ids(entries())
        print("a subiu ->", ordem2[:4])
        assert ordem2[0] == a and ordem2[1] == b, "move_pinned up falhou"

        js("(async()=>await %s('move_pinned',{id:%r,up:false}))()" % (INV, a))
        ordem3 = pinned_ids(entries())
        print("a desceu ->", ordem3[:4])
        assert ordem3[0] == b and ordem3[1] == a, "move_pinned down falhou"

        # limpa: desfixa os dois
        js("(async()=>await %s('toggle_pin',{id:%r}))()" % (INV, a))
        js("(async()=>await %s('toggle_pin',{id:%r}))()" % (INV, b))
        print("desfixados ->", pinned_ids(entries())[:4])

    # ------------------------------------------------------------ editar texto
    alvo = next((e for e in entries() if e["kind"] == "text" and (e.get("text") or "")), None)
    if alvo:
        original = js("(async()=>await %s('entry_text',{id:%r}))()" % (INV, alvo["id"]))
        novo = "editado-%d" % int(time.time())
        js("(async()=>await %s('update_entry_content',{id:%r,text:%r}))()" % (INV, alvo["id"], novo))
        depois = js("(async()=>await %s('entry_text',{id:%r}))()" % (INV, alvo["id"]))
        titulo = next(e for e in entries() if e["id"] == alvo["id"])["title"]
        print("conteudo depois de editar:", repr(depois), "| titulo:", repr(titulo))
        assert depois == novo, "conteudo nao foi gravado"
        # volta ao original
        js("(async()=>await %s('update_entry_content',{id:%r,text:%r}))()" % (INV, alvo["id"], original))
        print("restaurado:", js("(async()=>await %s('entry_text',{id:%r}))()" % (INV, alvo["id"])) == original)
    else:
        print("!! nenhuma entrada de texto para editar")

    # ------------------------------------------------------------- copiar cor
    js("(async()=>await %s('copy_text',{text:'#4FA3F8'}))()" % INV)
    print("copy_text ok")

    # ---------------------------------------------------- apagar recentes (dry)
    n = js(
        "(async()=>{const l=await %s('list_entries');"
        "const cut=Date.now()-60000;"
        "return l.filter(e=>!e.pinned&&e.createdAt>=cut).length;})()" % INV
    )
    print("entradas que 'Last 1 Hour'... (60s) apagaria:", n)

    # ----------------------------------------------------------- grupos de data
    labels = js(
        "(()=>[...document.querySelectorAll('.group-label')].map(e=>e.textContent.trim()))()"
    )
    print("grupos na tela:", labels)
finally:
    c.close()
