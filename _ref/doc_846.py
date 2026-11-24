import io

DOC = "C:/Users/Lugia/Projects/rayclip/DOCUMENTACAO.md"
SECAO = """### 8.46 Mutações devolvem 26 bytes em vez de 1,8 MB (v0.1.34)

Última dívida do §8.44. `toggle_pin`, `move_pinned`, `update_entry_content`,
`rename_entry`, `delete_entry`, `delete_entries`, `delete_recent` e
`clear_entries` devolviam **o histórico inteiro** para a interface se
atualizar. Depois da paginação a interface já pede a página de novo, então esse
payload era só custo — 1,8 MB por clique em "fixar".

Agora devolvem:

```rust
struct Mudanca { removed: usize, total: usize }   // 26 bytes
```

`removed` alimenta o aviso ("3 recent entries deleted") e `total` a contagem da
lista; `delete_recent` deixou de precisar que a interface comparasse tamanhos.

**Medido (build debug, CDP):**

| Comando | resposta | tempo |
|---|---|---|
| `toggle_pin` (fixar) | `{removed:0,total:1683}` | **1,6 ms** |
| `toggle_pin` (desfixar) | idem | 4,4 ms |
| `delete_recent(0)` | `{removed:0,...}` | 1,8 ms |
| `delete_entry` (id inexistente) | `{removed:0,...}` | 3,2 ms |
| `delete_entries` (ids inexistentes) | `{removed:0,...}` | 2,9 ms |

Antes: 1.846 KB, 33 ms só de serialização no log (`list: 1683 entradas...`), e a
partir de ~87 ms de ponta a ponta na medição do §8.40. O `list:` sumiu do log
depois de qualquer mutação.

#### O bug que o teste pegou: **deadlock**

A primeira versão chamava `mudanca(&state, n)` **de dentro** do bloco que
segurava `lock_store(&state.store)` — e `mudanca()` chama `lock_store` de novo.
O mutex do store **não é reentrante**: o app congelou e o `delete_recent` nunca
respondeu (o CDP deu timeout, o log registrou a entrada e nada depois). Corrigido
com `mudanca_com(removed, total)`, que recebe o total que já está na mão dentro
do lock. Fixtures que testam mutação precisam ser rodadas **de verdade**: um
teste que só compara tipos passaria com esse deadlock no lugar.

**Testes:** 43 (`cargo test --lib`).

**Release 0.1.34:** `Rayclip_0.1.34_x64-setup.exe` (3.57 MiB) instalado; inicia na
bandeja; CDP fora (9222 recusada); publicada no GitHub com `rayclip.exe` +
`rayclip.exe.sha256` + instalador.

"""

ALVO = "### 8.45 Auto-updater verificado"
s = io.open(DOC, encoding="utf-8").read()
assert ALVO in s
s = s.replace(ALVO, SECAO + ALVO, 1)
io.open(DOC, "w", encoding="utf-8", newline="").write(s)
print("doc 8.46 ok")

README = "C:/Users/Lugia/Projects/rayclip/README.md"
TXT = """### Novidades da v0.1.34 — fixar/renomear/apagar devolvem 26 bytes

Os comandos de mutação (`toggle_pin`, `move_pinned`, `update_entry_content`,
`rename_entry`, `delete_*`, `clear_entries`) devolviam **o histórico inteiro**
para a interface se atualizar: 1,8 MB por clique em "fixar". Agora devolvem
`{removed, total}` — 26 bytes, 1,6 a 4,4 ms, medido.

O `removed` também deixou o aviso de "Delete Recent Entries…" mais honesto (antes
a interface calculava comparando tamanhos da lista).

De brinde, o teste pegou um **deadlock**: a primeira versão chamava o helper de
resposta dentro do bloco que segurava o mutex do store (não reentrante) e o app
congelava. Corrigido e re-testado nos cinco comandos.

"""
ALVO2 = "### Novidades da v0.1.33"
r = io.open(README, encoding="utf-8").read()
assert ALVO2 in r
r = r.replace(ALVO2, TXT + ALVO2, 1)
io.open(README, "w", encoding="utf-8", newline="").write(r)
print("readme ok")
