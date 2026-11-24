import io

DOC = "C:/Users/Lugia/Projects/rayclip/DOCUMENTACAO.md"
SECAO = """### 8.47 Regressão: clicar numa linha não selecionava (v0.1.35)

Sintoma relatado: "o clique de selecionar não funciona mais, ele se recusa".
Real e grave — **qualquer** mudança de seleção (clique, Ctrl+clique, Shift,
setas) era desfeita no mesmo instante, e o app voltava sempre para o primeiro
item. Introduzido no §8.44 (v0.1.33, busca no Rust), não no v0.1.34.

**Causa, em três linhas:**

```ts
const refresh = useCallback(..., [selectedId]);   // §8.44: passou a depender da seleção
useEffect(() => { void refresh(false); ... }, [refresh, t]);   // efeito de montagem
```

`refresh(false)` significa "busca nova, seleção cai no primeiro item". Como a
função mudava de identidade a cada troca de seleção, o efeito de montagem rodava
de novo a cada clique e chamava `refresh(false)` — selecionava e desconceava em
menos de 24 ms.

Efeito colateral medido: **tempestade de buscas**. Cada volta mudava estado e
re-disparava o efeito; o log mostrava pares de `busca: q="" ... -> 200 de 1684
itens, 157 KB` a cada ~24 ms (lista + `ocr_pending`), ou seja, CPU e IPC
constantes enquanto a janela estivesse aberta.

**Conserto:** `refresh` volta a ser estável (`[]`). A seleção chega por
`selecionadoRef` (mesmo padrão já usado para `consulta`), então o efeito de
montagem roda uma vez e os cliques não o recriam.

**Verificação (build debug, CDP, mouse/teclado de verdade — `_ref/probe_select.py`,
`_ref/probe_select_mods.py`):**

| Ação | Antes | Depois |
|---|---|---|
| clique na 3ª linha | seleção ficava na 1ª | seleciona a 3ª |
| clique na 6ª linha | ficava na 1ª | seleciona a 6ª |
| Ctrl+clique em duas linhas | marcava e perdia | `['true:e8ba6e45', 'true:70ae7032']` |
| Shift+clique | intervalo perdido | intervalo de 4 itens |
| seta ↓ / ↑ | não mexia | anda 1 item |
| buscas no log em 8 s | tempestade (~24 ms) | **4** (montagem + páginas) |

**Limite do teste:** a build instalada não pôde ser verificada por clique
sintético — nesta sessão a janela do app não fica na frente (Discord e o console
do agente estão por cima: `WindowFromPoint` no ponto do clique devolveu
`discord.exe`), e o app se esconde ao perder o foco. A prova vale para o bundle
que foi embarcado: `npx tauri build` compilou o mesmo `dist/` verificado no debug.

**Lição:** um `useCallback` que passa a depender da seleção não pode entrar na
lista de dependências de um efeito de montagem. Se a função precisa "ver" o
estado atual, leia de um ref e mantenha as dependências vazias.

"""

ALVO = "### 8.46 Mutações devolvem 26 bytes"
s = io.open(DOC, encoding="utf-8").read()
assert ALVO in s
s = s.replace(ALVO, SECAO + ALVO, 1)
io.open(DOC, "w", encoding="utf-8", newline="").write(s)
print("doc 8.47 ok")

README = "C:/Users/Lugia/Projects/rayclip/README.md"
TXT = """### Novidades da v0.1.35 — clique seleciona de novo

O v0.1.33 (busca no Rust) deixou um `useCallback` dependendo da seleção. Como
esse `refresh` estava nas dependências do efeito de montagem, **cada clique
recriava a função, o efeito rodava de novo e devolvia a seleção para o primeiro
item** — clicar, Ctrl+clique, Shift+clique e as setas não seguravam a seleção.

De brinde, isso também causava uma **tempestade de buscas** (~24 ms entre uma e
outra) enquanto a janela estivesse aberta: 1,8 MB/s de IPC e CPU à toa. Agora
`refresh` é estável e lê a seleção de um ref; o log passou de dezenas de buscas
para 4 em 8 segundos.

"""
ALVO2 = "### Novidades da v0.1.34"
r = io.open(README, encoding="utf-8").read()
assert ALVO2 in r
r = r.replace(ALVO2, TXT + ALVO2, 1)
io.open(README, "w", encoding="utf-8", newline="").write(r)
print("readme ok")
