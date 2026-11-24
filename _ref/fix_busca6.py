"""Depois de mutações, recarrega a página em vez de guardar a lista inteira.

Os comandos de mutação (`toggle_pin`, `delete_*`, `rename_entry`…) devolvem o
histórico inteiro (1,8 MB). Guardar isso em `entries` desfazia a paginação: a
lista voltava a ter 1.683 itens e o `total` ficava mentindo.
"""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()

pares = [
    (
        "        setEntries(await api.updateEntryContent(cur.id, text));",
        "        await api.updateEntryContent(cur.id, text);\n        await refresh();",
    ),
    (
        "        setEntries(await api.renameEntry(id, clean));",
        "        await api.renameEntry(id, clean);\n        await refresh();",
    ),
    (
        "          setEntries(await api.togglePin(selected.id));",
        "          await api.togglePin(selected.id);\n          await refresh();",
    ),
    (
        "            setEntries(await api.movePinned(selected.id, true));",
        "            await api.movePinned(selected.id, true);\n            await refresh();",
    ),
    (
        "            setEntries(await api.movePinned(selected.id, false));",
        "            await api.movePinned(selected.id, false);\n            await refresh();",
    ),
    (
        "        setEntries(await api.movePinned(selected.id, e.key === \"ArrowUp\"));",
        "        await api.movePinned(selected.id, e.key === \"ArrowUp\");\n        await refresh();",
    ),
]

for velho, novo in pares:
    assert velho in s, velho
    s = s.replace(velho, novo, 1)

# `clear_entries` / `delete_recent` / `delete_entries` / `delete_entry`: o
# retorno serve para contar o que saiu; a lista em si vem do `refresh`.
velho = """        const next = await api.deleteRecent(minutes);
        const removed = entries.length - next.length;
        setEntries(next);"""
novo = """        const next = await api.deleteRecent(minutes);
        const removed = entries.length - next.length;
        await refresh();"""
assert velho in s
s = s.replace(velho, novo, 1)

velho = """        const next = await api.deleteEntries(ids);
        setEntries(next);"""
novo = """        await api.deleteEntries(ids);
        await refresh();"""
assert velho in s
s = s.replace(velho, novo, 1)

velho = """        const next = await api.deleteEntry(id);
        setEntries(next);"""
novo = """        await api.deleteEntry(id);
        await refresh();"""
assert velho in s
s = s.replace(velho, novo, 1)

s = s.replace("setEntries(await api.clearEntries(true));", "await api.clearEntries(true);\n          await refresh();")

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("mutações agora recarregam a página")
