"""Remove o filtro em JS do caminho de render (o Rust já entrega pronto)."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()

old = '''  const visible = useMemo(() => {
    const base = entries.filter((e) => matchesFilter(e, filter) && matchesQuery(e, query));
    // Buscando: ordem por relevância (texto antes de OCR, fixado, exato, recência).
    return rankEntries(base, query);
  }, [entries, filter, query]);'''
new = '''  // O Rust já devolveu filtrado e ordenado (§8.44): aqui só agrupamos por data
  // quando não há busca. Filtrar de novo daria resultado diferente — as regras
  // de ordenação são as mesmas, mas o conjunto carregado é só uma página.
  const visible = entries;'''
assert old in s
s = s.replace(old, new, 1)

old2 = '''import { groupEntries, matchesFilter, matchesQuery, matchedOnlyInOcr, mediaUrl, rankEntries } from "./lib/format";'''
new2 = '''import { groupEntries, matchedOnlyInOcr, mediaUrl } from "./lib/format";'''
assert old2 in s
s = s.replace(old2, new2, 1)

old3 = '''      query.trim() ? [{ label: "", entries: visible }] : groupEntries(visible, t),'''
new3 = '''      query.trim() ? [{ label: "", entries }] : groupEntries(entries, t),'''
assert old3 in s
s = s.replace(old3, new3, 1)

old4 = '''    [visible, query, t]
  );'''
new4 = '''    [entries, query, t]
  );'''
assert old4 in s
s = s.replace(old4, new4, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("visible = entries")
