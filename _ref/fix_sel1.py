"""Regressão do §8.44: clicar numa linha não mudava a seleção.

Causa: `refresh` era `useCallback(..., [selectedId])`, e o efeito de montagem
tinha `[refresh, t]`. Então **toda** troca de seleção recriava `refresh`, o
efeito rodava de novo e chamava `refresh(false)` — que joga a seleção para o
primeiro item. Clicar selecionava e voltava em 24 ms; o log mostrava uma
tempestade de `busca: q="" ...` (duas a cada ~24 ms) porque cada volta também
mudava o estado.

Conserto: `refresh` fica estável (`[]`) lendo a seleção de um ref.
"""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()

old = '''  const consulta = useRef({ query: "", filter: "all" });
  consulta.current = { query, filter: filter as string };'''
new = '''  const consulta = useRef({ query: "", filter: "all" });
  consulta.current = { query, filter: filter as string };

  // A seleção também vem de um ref, e não do fechamento: `refresh` precisa ser
  // **estável** para não recriar o efeito de montagem a cada clique (§8.47).
  const selecionadoRef = useRef<string | null>(null);
  selecionadoRef.current = selectedId;'''
assert old in s
s = s.replace(old, new, 1)

old = '''      const { query: q, filter: f } = consulta.current;
      const sel = selectedId;
      const page = await api.searchEntries(q, f, 0, PAGE);'''
new = '''      const { query: q, filter: f } = consulta.current;
      const sel = selecionadoRef.current;
      const page = await api.searchEntries(q, f, 0, PAGE);'''
assert old in s
s = s.replace(old, new, 1)

old = '''      setSelectedId((cur) =>
        keepSelection && cur && items.some((e) => e.id === cur) ? cur : items[0]?.id ?? null
      );
    },
    [selectedId]
  );'''
new = '''      setSelectedId((cur) =>
        keepSelection && cur && items.some((e) => e.id === cur) ? cur : items[0]?.id ?? null
      );
    },
    // De propósito vazio: a busca e a seleção entram por ref. Com `[selectedId]`
    // cada clique recriava esta função e o efeito de montagem (`[refresh, t]`)
    // rodava `refresh(false)` de novo, devolvendo a seleção ao primeiro item.
    []
  );'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("App.tsx: refresh estavel")
