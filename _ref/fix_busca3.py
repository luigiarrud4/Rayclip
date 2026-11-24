"""Efeito de nova consulta ao digitar/filtrar, total real e `loadMore` na lista."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/App.tsx"
s = io.open(P, encoding="utf-8").read()

# 1) no máximo 3 páginas de busca para reencontrar a seleção: procurar em 1683
#    entradas traria de volta justamente o 1,8 MB que a mudança tirou.
old = '''        for (let i = 1; i < 10 && items.length < page.total; i++) {'''
new = '''        for (let i = 1; i < 3 && items.length < page.total; i++) {'''
assert old in s
s = s.replace(old, new, 1)

# 2) o `total` do Rust é quantos casam; a lista usa isso para o estado vazio.
old = '''            totalEntries={entries.length}'''
new = '''            totalEntries={total}'''
assert old in s
s = s.replace(old, new, 1)

# 3) rolagem perto do fim pede a próxima página
old = '''            onContextMenu={openActionsFor}'''
new = '''            onNearEnd={() => void loadMore()}
            onContextMenu={openActionsFor}'''
assert old in s
s = s.replace(old, new, 1)

# 4) nova consulta quando muda a busca ou o filtro (debounce 90 ms)
old = '''  useEffect(() => {
    void refresh(false);'''
new = '''  // Digitar/filtrar = nova consulta no Rust. 90 ms de debounce: tecla rápida
  // não vira 10 idas e voltas, e mesmo assim a lista acompanha o texto.
  const refreshAtual = useRef(refresh);
  refreshAtual.current = refresh;
  const primeiraConsulta = useRef(true);
  useEffect(() => {
    if (primeiraConsulta.current) {
      primeiraConsulta.current = false;
      return;
    }
    const id = window.setTimeout(() => {
      // `false`: com busca nova a seleção cai no primeiro resultado, que é o
      // comportamento do Raycast (e dá Enter para colar o mais provável).
      void refreshAtual.current(false);
    }, 90);
    return () => window.clearTimeout(id);
  }, [query, filter]);

  useEffect(() => {
    void refresh(false);'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("App.tsx: consulta ao digitar + loadMore")
