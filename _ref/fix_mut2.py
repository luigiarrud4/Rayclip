"""api.ts: mutações devolvem `Mudanca` (removed/total), não `Entry[]`."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/api.ts"
s = io.open(P, encoding="utf-8").read()

old = '''export const togglePin = (id: string) => invoke<Entry[]>("toggle_pin", { id });
export const movePinned = (id: string, up: boolean) =>
  invoke<Entry[]>("move_pinned", { id, up });
export const updateEntryContent = (id: string, text: string) =>
  invoke<Entry[]>("update_entry_content", { id, text });'''
new = '''/// Resposta das mutações: antes vinha o histórico inteiro (1,8 MB por clique).
export type Mudanca = { removed: number; total: number };

export const togglePin = (id: string) => invoke<Mudanca>("toggle_pin", { id });
export const movePinned = (id: string, up: boolean) =>
  invoke<Mudanca>("move_pinned", { id, up });
export const updateEntryContent = (id: string, text: string) =>
  invoke<Mudanca>("update_entry_content", { id, text });'''
assert old in s
s = s.replace(old, new, 1)

old = '''export const deleteRecent = (minutes: number) =>
  invoke<Entry[]>("delete_recent", { minutes });
export const renameEntry = (id: string, title: string) =>
  invoke<Entry[]>("rename_entry", { id, title });
export const deleteEntry = (id: string) => invoke<Entry[]>("delete_entry", { id });
export const deleteEntries = (ids: string[]) => invoke<Entry[]>("delete_entries", { ids });
export const clearEntries = (keepPinned = true) =>
  invoke<Entry[]>("clear_entries", { keepPinned });'''
new = '''export const deleteRecent = (minutes: number) =>
  invoke<Mudanca>("delete_recent", { minutes });
export const renameEntry = (id: string, title: string) =>
  invoke<Mudanca>("rename_entry", { id, title });
export const deleteEntry = (id: string) => invoke<Mudanca>("delete_entry", { id });
export const deleteEntries = (ids: string[]) => invoke<Mudanca>("delete_entries", { ids });
export const clearEntries = (keepPinned = true) =>
  invoke<Mudanca>("clear_entries", { keepPinned });'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("api.ts ok")

# App.tsx: contagem do aviso vem do `removed`
P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()
old = '''        const next = await api.deleteRecent(minutes);
        const removed = entries.length - next.length;
        await refresh();
        setSelection((s) => s.filter((id) => next.some((e) => e.id === id)));
        flash(t("{n} recent entries deleted", { n: removed }));'''
new = '''        const res = await api.deleteRecent(minutes);
        await refresh();
        // Sem a lista inteira de volta, a seleção múltipla é limpa: os ids que
        // saíram não podem continuar marcados.
        setSelection([]);
        flash(t("{n} recent entries deleted", { n: res.removed }));'''
assert old in s
s = s.replace(old, new, 1)
io.open(P, "w", encoding="utf-8", newline="").write(s)
print("App.tsx ok")
