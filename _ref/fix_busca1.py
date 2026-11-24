"""Liga a App.tsx na busca paginada do Rust (§8.44)."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()

old = '''/** Janelas de tempo do "Delete Recent Entries…" (mesmas do Raycast). */'''
new = '''/**
 * Entradas por página. A busca e o filtro rodam no Rust (8.44), então a página
 * recebe só isso em vez das 1,8 MB de todo o histórico.
 */
const PAGE = 200;

/** Janelas de tempo do "Delete Recent Entries…" (mesmas do Raycast). */'''
assert old in s
s = s.replace(old, new, 1)

old = '''  const [entries, setEntries] = useState<Entry[]>([]);
  const [query, setQuery] = useState("");'''
new = '''  const [entries, setEntries] = useState<Entry[]>([]);
  /** Quantos itens casam no total (vem do Rust); `entries` é só a página. */
  const [total, setTotal] = useState(0);
  const [query, setQuery] = useState("");'''
assert old in s
s = s.replace(old, new, 1)

old = '''  const refresh = useCallback(async (keepSelection = true) => {
    const next = await api.listEntries();
    setEntries(next);
    setSelectedId((cur) => (keepSelection && cur && next.some((e) => e.id === cur) ? cur : next[0]?.id ?? null));
  }, []);'''
new = '''  // Parâmetros atuais da consulta num ref: `refresh` precisa ser estável (entra
  // em vários `useEffect`) mas tem de enxergar a busca do momento.
  const consulta = useRef({ query: "", filter: "all" });
  consulta.current = { query, filter: filter as string };

  /**
   * Carrega a primeira página da busca atual.
   *
   * Mantém a seleção: se a entrada selecionada não estiver na primeira página
   * (histórico grande e algo antigo selecionado), pede as páginas seguintes até
   * achá-la — perder a seleção ao reabrir a janela estava fora de questão.
   */
  const refresh = useCallback(
    async (keepSelection = true) => {
      const { query: q, filter: f } = consulta.current;
      const sel = selectedId;
      const page = await api.searchEntries(q, f, 0, PAGE);
      let items = page.items;
      if (keepSelection && sel && !items.some((e) => e.id === sel)) {
        for (let i = 1; i < 10 && items.length < page.total; i++) {
          const next = await api.searchEntries(q, f, i * PAGE, PAGE);
          if (next.items.length === 0) break;
          items = [...items, ...next.items];
          if (items.some((e) => e.id === sel)) break;
        }
      }
      setEntries(items);
      setTotal(page.total);
      setSelectedId((cur) =>
        keepSelection && cur && items.some((e) => e.id === cur) ? cur : items[0]?.id ?? null
      );
    },
    [selectedId]
  );

  /** Próxima página, pedida quando a rolagem chega perto do fim. */
  const carregando = useRef(false);
  const loadMore = useCallback(async () => {
    if (carregando.current || entries.length >= total) return;
    carregando.current = true;
    try {
      const { query: q, filter: f } = consulta.current;
      const page = await api.searchEntries(q, f, entries.length, PAGE);
      setEntries((cur) => {
        const tem = new Set(cur.map((e) => e.id));
        return [...cur, ...page.items.filter((e) => !tem.has(e.id))];
      });
      setTotal(page.total);
    } finally {
      carregando.current = false;
    }
  }, [entries.length, total]);'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("App.tsx: refresh paginado")
