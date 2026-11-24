import io

DOC = "C:/Users/Lugia/Projects/rayclip/DOCUMENTACAO.md"
SECAO = """### 8.44 Busca e filtro no Rust: 1,8 MB por atualização -> 158 KB (v0.1.33)

O log sempre soube o tamanho do problema: `list: 1683 entradas, 1846 KB,
serializado em 33 ms`. A cada atualização (abrir a janela, colar, capturar) a
página recebia **todo** o histórico para filtrar em JS — e o §8.40 já tinha
mostrado que trocar o formato de transporte (`ipc::Response`) não muda nada:
quem pesa é o **tamanho**.

**Agora quem filtra é o Rust.** Comando novo:

```rust
search_entries(query, filter, offset, limit) -> { items, total, ms }
```

- `src-tauri/src/search.rs`: `fold` (minúsculas e sem acento), `hit_rank`
  (exato > início > começo de palavra > meio), `filtrar` (filtro + busca +
  ordenação + página);
- a projeção enxuta é feita **só nos casados**;
- a página carrega **200** itens e a próxima é pedida quando a rolagem chega a
  três telas do fim (`EntryList` -> `onNearEnd`);
- digitar/filtrar pede nova consulta com 90 ms de debounce; a seleção cai no
  primeiro resultado (regra do Raycast: Enter cola o mais provável).

#### O cache que fez a diferença

A primeira versão dobria (minúsculas + sem acento) os campos de 1.683 entradas a
cada consulta: **198 ms por tecla**. Agora o resultado fica num cache em memória,
invalidado por `created_at|hash` da entrada (renomear/editar refaz), com corte de
4 KB por campo — o mesmo corte da projeção antiga, então procurar além disso
nunca funcionou e continua não funcionando. O cache é **aquecido numa thread de
prioridade baixa** ao abrir o histórico (1.683 entradas em 20 ms).

#### Medido (build debug, 1.683 entradas)

| | antes | agora |
|---|---|---|
| payload por atualização | 1.846 KB | **158 KB** (200 de 1683) |
| serialização | 33 ms | **4–5 ms** |
| primeira busca digitada | — | 198 ms -> **8 ms** com o cache |
| aquece o cache | — | 1.683 entradas em **20 ms** |
| página seguinte (rolagem) | — | 200 itens, 166 KB, **5 ms** |

#### Conferido na interface

- `"reno"` -> **RENOMEADO-NO-RELEASE** (título renomeado, prefixo) selecionado e
  acima das imagens que só casaram pelo OCR (selo "Aa");
- `"gith"` -> os links do GitHub primeiro, as imagens com OCR de GitHub depois
  (o que casa em `source` conta como campo visível);
- rolagem -> `offset=200` no log, mais 200 itens.

#### Mudança de comportamento (deliberada)

O rótulo sintético de imagem (`Image (23/11/2026, 10:16:14 PM)`) **saiu da
busca**: ele depende do idioma e da formatação de data do sistema (a UI em PT
mostrava data em formato americano), então casar "image" com toda imagem era
ruído. `matchesQuery` em `format.ts` foi alinhado à mesma regra e ficou só como
referência das regras + selo de OCR.

#### Dívida assumida

Os comandos de mutação (`toggle_pin`, `delete_*`, `rename_entry`, ...) ainda
**devolvem o histórico inteiro** (1,8 MB por ação). A UI passou a ignorar esse
payload e pedir a página de novo, mas o certo é cada um devolver só a entrada
mexida. Fica anotado como próximo passo.

**Testes:** 43 (`cargo test --lib`), 7 novos em `search.rs` (dobra com acento,
filtro por tipo, ordem exato/prefixo/palavra/meio, fixado no mesmo tipo,
paginação com total, selo de OCR).

**Release 0.1.33:** `Rayclip_0.1.33_x64-setup.exe` (3.57 MiB) instalado; inicia na
bandeja; CDP fora (9222 recusada); publicada no GitHub com `rayclip.exe` +
`rayclip.exe.sha256` + instalador.

"""

ALVO = "### 8.43 Renderer do WebView2 morto"
s = io.open(DOC, encoding="utf-8").read()
assert ALVO in s
s = s.replace(ALVO, SECAO + ALVO, 1)
io.open(DOC, "w", encoding="utf-8", newline="").write(s)

README = "C:/Users/Lugia/Projects/rayclip/README.md"
TXT = """### Novidades da v0.1.33 — busca no Rust (1,8 MB -> 158 KB por atualização)

O app recebia **todo** o histórico a cada atualização (1.846 KB, 33 ms de
serialização) e filtrava na página. Agora o filtro, a ordenação e a paginação
acontecem no Rust: a página leva 200 itens (158 KB, 4–5 ms) e pede a próxima
quando a rolagem chega perto do fim.

| | antes | agora |
|---|---|---|
| payload por atualização | 1.846 KB | **158 KB** |
| serialização | 33 ms | **4–5 ms** |
| busca digitada | — | 198 ms -> **8 ms** (com cache aquecido) |

As regras de ordem são as mesmas de antes (texto antes de OCR, fixados primeiro,
exato > início > começo de palavra > meio, recência no empate) e têm teste
próprio. Mudança deliberada: o rótulo sintético `Image (data)` saiu da busca —
casar "image" com toda imagem era ruído.

Dívida anotada: `toggle_pin`, `delete_*` e `rename_entry` ainda devolvem o
histórico inteiro por IPC; a interface ignora o payload e repede a página.

"""
ALVO2 = "### Novidades da v0.1.32"
r = io.open(README, encoding="utf-8").read()
assert ALVO2 in r
r = r.replace(ALVO2, TXT + ALVO2, 1)
io.open(README, "w", encoding="utf-8", newline="").write(r)
print("docs ok")
