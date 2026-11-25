# Rayclip — documentação completa

Rayclip é um **gestor de histórico de área de transferência** para Windows, feito do
zero, que recria **apenas** o recurso *Clipboard History* do Raycast para Windows —
inclusive a aparência. O Raycast original foi desmontado (engenharia reversa) para
saber o que copiar: layout, cores, fontes, tempos e comportamento. Nada de código
dele é usado; só o resultado visual e funcional foi reproduzido.

- Pasta do projeto: `C:\Users\Lugia\Projects\rayclip`
- Stack: **Tauri v2 (Rust) + React + TypeScript + WebView2**
- Versão atual: **0.1.41**
- Binário: `src-tauri/target/release/rayclip.exe` (11.064.320 B, SHA256
  `c13e3d03edfd4a4f9a9f4d5b8ef21a3fc6f5cab105abcd0e935037b1ecf3dd89`)
- Instalador: `src-tauri/target/release/bundle/nsis/Rayclip_0.1.41_x64-setup.exe`
  (3.797.154 B)

Este documento explica **o que o app faz, como é feito por dentro e como cada
problema difícil foi resolvido** (com a medição usada para provar).

---

## 1. Escopo

**Dentro:** histórico do clipboard com busca, prévia, OCR de imagens, prévia de
links, atalhos globais, atalhos internos, ações (colar, copiar, colar sem
formatação, salvar como, abrir link, fixar, apagar), seleção múltipla com colagem
em ordem, bandeja, instância única, início automático elevado, banco cifrado e
importação do histórico do Raycast.

**Fora (de propósito):** comandos/extensões do Raycast, launcher de aplicativos,
snippets, cálculo, janela de "root search". O app é *só* o histórico.

---

## 2. Engenharia reversa do Raycast (2.13.1) — o que descobrimos

| Descoberta | Como foi obtida |
|---|---|
| Casca em **C#/.NET WPF** com **WebView2**; backend Node; módulo nativo Rust (N-API); SQLite | inspeção da pasta de instalação, `.deps.json`, `strings` no binário |
| Histórico em **SQLite cifrado (SQLCipher)** + chave selada com **DPAPI** (`creds\BackendDBKey.dpapi`) | cabeçalho do `.db` não tem `SQLite format 3`; blob DPAPI presente |
| Janela: header 40 px, rodapé 42 px, raio 12, fundo `#000`, texto `#FFF`, acento `#4FA3F8`, vermelho `#FF6363`, verde `#59D499` | leitura dos tokens CSS empacotados (`bundled-raycast-dark`) |
| Fontes **InterVariable** (UI) e **JetBrains Mono** (código) | arquivos de fonte no bundle |
| Captura por **polling** de `GetClipboardSequenceNumber`, não por evento | ausência de `AddClipboardFormatListener` no binário e intervalo constante de consulta |
| Cada cópia vira arquivos soltos em `%LOCALAPPDATA%\Raycast\clipboard` (`png/jpg/gif/html/rtf/txt`) | conteúdo da pasta após copiar texto e imagem |
| Fechar ao trocar de app, ficar por cima, atalho global `Alt+Space` | teste de uso |

Detalhe curioso: o `.db` do Raycast nesta máquina é **ilegível por qualquer
processo** — a chave DPAPI está órfã (o master key do usuário que a cifrou não
existe mais). Não foi possível ler o histórico por dentro; por isso o Rayclip
importa os **arquivos** de `%LOCALAPPDATA%\Raycast\clipboard`.

---

## 3. Arquitetura

```
 Alt+Space / Win+V ─┐
 clique na bandeja ─┼─► show_window() ─► janela sempre-no-topo (Mica/acrílico)
                    │
 GetClipboardSequenceNumber (polling 320 ms)
        │ mudou
        ▼
 capture::build_entry()  ── texto / link / cor / e-mail / imagem / arquivos
        │                      └─ OCR (Windows.Media.Ocr) ─┐
        │                      └─ linkmeta (título, ícone, capa) ─┤
        ▼                                                       │
 store (SQLCipher) ── entries (metadados) + blobs (bytes) ◄──────┘
        │
        ├─► comando list_entries  ─► React (lista virtualizada)
        └─► protocolo http://rayclip.localhost/img/<id>  e  /thumb/<id>
                                   (bytes saem do banco, nunca do disco)
```

**Ciclo de vida da janela** (o ponto mais delicado do app):

- aparece com `Alt+Space` / `Win+V` de forma global (`tauri-plugin-global-shortcut`);
- esconde no **clique fora** (hook global `WH_MOUSE_LL`), na **troca de app**
  (`SetWinEventHook` de `EVENT_SYSTEM_FOREGROUND`) e num watchdog de 150 ms como
  rede de segurança;
- nunca esconde com diálogo nativo aberto, com o próprio menu/popup aberto, nem
  quando um console "pisca" (processo auxiliar) — ver §7.6.

---

## 4. Módulos Rust (`src-tauri/src`)

| Arquivo | Responsabilidade |
|---|---|
| `main.rs` | entrada do processo |
| `lib.rs` | estado do app, comandos `#[tauri::command]`, jobs de OCR/thumb/link, protocolo `rayclip://`, bandeja, atalhos, ciclo mostrar/esconder |
| `capture.rs` | monta uma `Entry` a partir do clipboard; normaliza texto; calcula hash de deduplicação |
| `clipimg.rs` | lê imagem do clipboard (DIB/DIBV5/Bitmap/PNG) e **publica** de volta (colar imagem) |
| `store.rs` | persistência: schema, upsert, blobs, retenção/limite, migração do `store.json` antigo |
| `sqlite.rs` | wrapper FFI mínimo do SQLCipher (`open`/`key`/`exec`/`query`/`Val`) |
| `key.rs` | gera/sela a chave de 32 bytes com **DPAPI** (`rayclip.key`) |
| `ocr.rs` | OCR nativo (`Windows.Media.Ocr`), redimensiona para ≤2400 px, roda em processo separado sem console |
| `linkmeta.rs` | busca título/ícone/capa do link (HTTP), guarda a capa como *data URL* |
| `thumbs.rs` | miniaturas de arquivo (PDF, vídeo, documento) e ícone do app de origem |
| `import.rs` | importa `%LOCALAPPDATA%\Raycast\clipboard` agrupando arquivos por cópia |
| `settings.rs` | `settings.json` (atalhos, limites, preferências) + migração de valores antigos |
| `sysinteg.rs` | autostart elevado (Tarefa Agendada), `Win+V` (DisabledHotkeys), restart do Explorer |
| `win32.rs` | toda a camada Win32: hooks de foco e de mouse, `ShowWindow`, DPAPI/vizinhança de processos, diálogos nativos, Mica/acrílico |

### Comandos expostos ao frontend

`list_entries`, `copy_entry`, `paste_entry`, `paste_many`, `save_entry_as`,
`save_image_as`, `save_image_to`, `save_many`, `save_many_to`, `toggle_pin`,
`delete_entry`, `delete_entries`, `clear_entries`, `hide_window`, `reveal_path`, `open_url`,
`ocr_pending`, `ocr_entry`, `copy_ocr_text`, `link_pending`, `link_entry`,
`thumb_entry`, `get_settings`, `save_settings`, `system_status`,
`set_windows_transparency`, `restart_explorer`, `raycast_import_count`,
`import_raycast`.

### Eventos (Rust → React)

`clipboard:focus`, `clipboard:new`, `clipboard:changed`,
`clipboard:multi-pasted`, `rayclip:settings`, `settings:changed`,
`import:progress`.

---

## 5. Frontend

| Arquivo | Papel |
|---|---|
| `App.tsx` | estado (filtro, seleção, ações), teclado, menus, colagem em lote |
| `components/EntryList.tsx` | lista **virtualizada** + throttle de miniaturas |
| `components/DetailsPane.tsx` | prévia grande, metadados, texto rolável/selecionável |
| `components/ActionPanel.tsx` / `ActionBar.tsx` | painel `Ctrl+K` e barra de ações do rodapé |
| `components/SettingsPage.tsx` | configurações em 7 seções (atalhos, colar, histórico, privacidade, dados/segurança, sistema, sobre) |
| `components/Dropdown.tsx` | dropdown genérico (usado em temas, acentos, formatos) |
| `components/SaveManyDialog.tsx` | "Save N Items As…" com formato por item |
| `lib/keys.ts` | comparação de aceleradores (`Ctrl+Shift+V`) e keycaps |
| `lib/format.ts` | `mediaUrl(kind,id,v)`, `fileUrl(path)`, datas, tamanhos |
| `lib/theme.ts` | temas e cor de acento via variáveis CSS |

Regras de interface copiadas do Raycast: fundo preto translúcido (Mica), header
40 px com busca, rodapé 42 px com ações e keycaps, linha de 44 px, selo `Aa` quando
o resultado veio do OCR da imagem, submenu lateral com hover estável.

**Nada de animação em massa:** só o item selecionado (GIF, prévia grande) é lido
do banco e animado; a lista carrega miniaturas apenas das linhas visíveis
(`IntersectionObserver` + fila com concorrência limitada).

---

## 6. Dados e segurança

### 6.1 Por que banco cifrado

O requisito do usuário: *"o conteúdo do clipboard não pode ser lido por nenhum
outro processo/usuário"*. Um `store.json` + PNGs soltos em `%APPDATA%` não
atendem. Então **todo o conteúdo** (texto, HTML, RTF, bytes de imagem e
miniatura, capa de link, OCR) vive dentro de um **SQLCipher**.

- Chave: 32 bytes aleatórios (`BCryptGenRandom`) selados com **DPAPI**
  (`CryptProtectData`, escopo do usuário) em `rayclip.key`.
- Se o arquivo de chave existir e **não** puder ser aberto (outro usuário,
  backup restaurado), o app **não** gera chave nova: o banco simplesmente não
  abre — evita destruir dados.
- `PRAGMA secure_delete = ON` (conteúdo apagado é zerado no arquivo) e
  `journal_mode = DELETE` (WAL deixava um `-wal` de 14 MB com dados).
- **Nenhum** PNG/JPG de conteúdo é escrito em disco: a interface recebe as
  imagens pelo protocolo `http://rayclip.localhost/img/<id>` (o sufixo
  `.localhost` é exigência do Windows para esquemas customizados).
- `media/` guarda só mídia **derivada** (ícone do app de origem, favicon).

### 6.2 Como o SQLCipher foi compilado sem OpenSSL e sem Perl

`rusqlite` com `bundled-sqlcipher` panica (`Missing environment variable
OPENSSL_DIR`); com `bundled-sqlcipher-vendored-openssl` falha porque o `perl` do
Git Bash é cygwin e gera caminhos que o `Configure` do OpenSSL não entende.

Solução: compilar o SQLCipher **direto da fonte** com o provider **LibTomCrypt**,
que não precisa de OpenSSL:

- build-deps: `cc = "1"` e `sqlcipher-src = "=420.0.0"`;
- `build.rs` → `build_sqlcipher()`:
  1. compila `sqlcipher_src::LIBTOMCRYPT_SOURCES` (com `LTC_SOURCE`,
     `LTC_NO_PROTOTYPES`, warnings desligados) em `tomcrypt`;
  2. compila a amalgamação `SOURCE_FILE` com `SQLITE_HAS_CODEC=1`,
     `SQLCIPHER_CRYPTO_LIBTOMCRYPT=1`, `SQLITE_EXTRA_INIT=sqlcipher_extra_init`,
     `SQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown`, `SQLITE_TEMP_STORE=2`,
     `SQLITE_THREADSAFE=1`, `SQLITE_ENABLE_FTS5/STAT4/RTREE/COLUMN_METADATA`,
     `SQLITE_USE_URI`, `HAVE_USLEEP`.
- `src-tauri/src/sqlite.rs` é um wrapper FFI enxuto (`sqlite3_open_v2`,
  `sqlite3_key`, `prepare/step/column`, `busy_timeout`), com teste que prova que
  a chave errada recebe `error decrypting page 1`.

### 6.3 Modelo de dados

```sql
entries(id TEXT PK, kind, title, text, paths JSON, has_image, image_w, image_h,
        source, source_icon, ocr_text, has_thumb, gif, link JSON, html, rtf,
        size_bytes, created_at, pinned, hash)
blobs(id, kind, bytes BLOB, PRIMARY KEY(id, kind))   -- kind = image | thumb
meta(k TEXT PRIMARY KEY, v TEXT)
```

Índices em `created_at` e `hash` (deduplicação). Limite de histórico
configurável (100…5000, padrão 500) e retenção opcional por dias: ao passar do
limite, o app remove as mais antigas **não fixadas** e apaga os blobs junto.

---

## 7. Funcionalidades e como cada uma foi feita

### 7.1 Captura de texto
Polling de `GetClipboardSequenceNumber()` a cada 320 ms (o contador muda mesmo
quando o app dono do clipboard morre; não exige abrir o clipboard). Ao detectar
mudança, lê os formatos disponíveis na ordem: `CF_HTML`, `CF_RTF`,
`CF_UNICODETEXT`. Texto é normalizado (fim de linha, espaços à direita) e
classificado:

- **e-mail** se for um endereço válido;
- **cor** se for `#RRGGBB` / `rgb()` / nome CSS conhecido (mostra amostra);
- **link** se começar com `http(s)://` e tiver TLD plausível;
- **caminho de arquivo(s)** se todos os caminhos existirem;
- senão **texto**.

Deduplicação por hash do conteúdo: copiar de novo o mesmo item **move** a
entrada existente para o topo em vez de criar duplicata. O hash cobre só o
conteúdo (texto/bytes), não a formatação, e o **id é preservado** no bump — ver
§8.22.

### 7.2 Imagens
Leitura por `CF_DIBV5` → `CF_DIB` → `CF_BITMAP` → PNG registrado. Os bytes
originais (PNG/GIF/WebP) são gravados como *blob*; GIFs guardam também um
"pôster" (primeiro quadro em PNG) para a lista não animar 1.400 GIFs de uma vez.

### 7.3 Colar imagem de volta
`copy_entry` publica de uma vez `CF_DIBV5`, `CF_DIB`, `CF_BITMAP` e PNG (além de
todos os formatos de texto quando existirem). Publicar os quatro evita o caso
"o app de destino só entende um deles".

### 7.4 OCR de imagens
- Motor: `Windows.Media.Ocr` (nativo, sem download de modelo).
- Imagens acima de 2400 px são reduzidas antes do reconhecimento; a imagem é
  salva num PNG temporário (nome com PID + timestamp) apenas durante a
  chamada e apagado em seguida.
- Roda **fora** da thread principal e em processo separado, com
  `CREATE_NO_WINDOW` + `Stdio::null()` — nenhuma janela de console pisca.
- O texto reconhecido vai para `ocr_text`, entra na busca (com selo `Aa`) e alimenta
  o atalho "Copy text found in image".

### 7.5 Prévia de links
`linkmeta` busca o HTML (ureq + TLS + gzip, limite de tamanho e timeout), extrai
`og:title`/`<title>`, `og:image`/`icon`, e grava a capa como **data URL** dentro
do banco (para não virar arquivo em disco). Com **Offline & private** ligado,
nada é buscado e a prévia fica desligada.

### 7.6 Mostrar e esconder (o coração do app)
Ver §8.1–8.4: é onde estavam os problemas difíceis.

### 7.7 Ações
Colar (`Enter`), colar sem formatação (`Ctrl+Shift+Enter`), colar mantendo aberto
(`Ctrl+Enter`), copiar texto/OCR (`Ctrl+T`), abrir link (`Ctrl+O`), fixar
(`Ctrl+P`), apagar (`Delete`), limpar histórico mantendo fixados
(`Ctrl+Shift+Delete`), painel de ações (`Ctrl+K`), configurações (`Ctrl+,`).

**Renomear (`Ctrl+E`):** o título da linha selecionada vira um campo de texto
inline (fundo do accent, texto todo selecionado). `Enter` grava, `Esc` cancela,
clicar fora grava; nome vazio cancela. Vale para **qualquer tipo** de entrada
(inclusive imagens — o nome não é sobrescrito pelo OCR/miniatura) e o novo nome
entra na busca. Também está no painel de ações (`Ctrl+K` → "Rename Entry").

**Salvar como:** funciona para **todo** tipo de entrada — imagem
(PNG/JPEG/WebP/BMP/GIF), texto (TXT/MD/HTML), arquivos de arquivos — com diálogo
nativo, formato por item e extensão livre. Selecionando vários itens, "Save N
Items As…" salva todos numa pasta escolhida, cada um no seu formato.

### 7.8 Seleção múltipla e ordem
`Ctrl`/`Shift` + clique (ou `Shift`+setas) marcam itens; `Enter` cola **na ordem
em que foram marcados** (`paste_many`), com o contador `2/3` na barra.

### 7.9 Origem ("Source")
Detectada por **ancestralidade de processo**: `GetClipboardOwner()` → PID → nome
do processo → cadeia de processos pai. Se quem escreveu foi o próprio Rayclip (ou
seu processo filho `msedgewebview2`), a origem aparece como **Rayclip** em vez de
`msedgewebview2`. O ícone é extraído com `SHGetFileInfo` para `media/src-<nome>.png`.

### 7.10 Integração com o Windows
- **Atalho global** `Alt+Space` (configurável) e **alternativo** `Win+V`.
- **Takeover do Win+V** via `HKCU\...\Explorer\Advanced\DisabledHotkeys` +
  restart do Explorer com confirmação.
- **Iniciar com o Windows**: Tarefa Agendada `\Rayclip` com `/RL HIGHEST`
  (o `HKCU\...\Run` não inicia elevado; a tarefa inicia).
- **Instância única**: evento nomeado `Local\RayclipActivateV1`; a segunda
  instância avisa a primeira e sai sem abrir console.
- **Bandeja**: ícone com "Mostrar", "Limpar histórico", "Configurações", "Sair";
  clique esquerdo mostra a janela.
- **Janela**: sempre no topo, sem borda padrão, Mica quando os efeitos de
  transparência estão ligados (e acrílico como reforço), com degradação
  automática para fundo sólido quando o usuário desliga a transparência no
  Windows (a opção mostra um aviso e um botão que liga com `reg`).

---

## 8. Como cada problema difícil foi resolvido

Formato: **sintoma → diagnóstico → solução → verificação**.

### 8.1 A janela demorava a sair da tela (3 rodadas)

**Rodada 1 — não existia hook, só polling.**
Sintoma: ~600 ms para esconder ao clicar em outro app.
Diagnóstico: o watchdog consultava o primeiro plano a cada 150 ms e exigia
duas leituras seguidas.
Solução: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT)` numa
thread com *message loop*; a decisão passa a rodar **no próprio callback**.
Verificação: 2–3 ms até esconder.

**Rodada 2 — proteção de cursor e console.**
Sintoma: Alt+Tab deixava a janela na tela até o mouse sair de cima dela; clicar
num terminal nunca escondia.
Diagnóstico: `cursor_over_window()` bloqueava o hide; `hwnd_is_transient()`
tratava `ConsoleWindowClass` como "flash de processo auxiliar" e o app nunca
escondia naquele terminal; o watchdog tinha carência de 900 ms.
Solução: o cursor não bloqueia mais (se o primeiro plano é outro app, o usuário
saiu); console é **confirmado** 140 ms depois (se continuar na frente, esconde);
carência do watchdog 900 → 300 ms.
Verificação: clique no Discord ~7 ms; desktop ~54 ms; terminal ~141 ms.

**Rodada 3 — a causa real: o caminho do hide.**
Sintoma: mesmo com o hook, "demora um pouco" quando o app é aberto e o usuário
clica em outro app logo em seguida.
Diagnóstico (log com **timestamp em ms**): `desde_show = 758 ms` para um clique
400 ms após abrir. `WebviewWindow::hide()` e `is_visible()` **despacham para a
thread principal do Tauri**, que fica ~600 ms ocupada pintando o WebView2 logo
após `show()`; todo pedido de esconder entrava na fila.
Solução:
- `win32::hide_now()` → `ShowWindow(SW_HIDE)` no HWND (~40 µs); o `hide()` do
  Tauri roda numa thread separada só para sincronizar o estado interno;
- visibilidade lida de `IsWindowVisible`, sem round-trip;
- pedidos que chegam nos primeiros 150 ms não são mais descartados — são
  **confirmados ao vivo** (`recheck`) em 160 ms;
- novo **hook global de mouse** (`WH_MOUSE_LL`): esconde no instante do botão
  pressionado em outra janela, sem depender de o outro app processar a ativação
  (o Discord demora). O callback só repassa o HWND por canal — nenhuma chamada
  ao Tauri dentro do hook, senão o Windows descarta o hook.
Verificação: abrir e clicar em outro app **32 ms depois** → escondida em **~10 ms**
(antes ~500–600 ms).

### 8.2 O hook de clique fechava a janela ao clicar dentro dela

Sintoma (v0.1.6): clicar numa linha não selecionava nem colava — a janela fechava.
Diagnóstico: o hook comparava o **PID** da janela sob o cursor com o nosso. Dentro
do app o alvo do clique é uma janela filha do **WebView2** (processo
`msedgewebview2`), então todo clique interno era lido como "clique fora".
Solução: comparar a **janela raiz** (`GetAncestor(hwnd, GA_ROOT)`) com a nossa;
PID só é usado para menus/diálogos nativos. Barra de tarefas/bandeja até 450 ms
após abrir é ignorada (senão o duplo-clique no ícone da bandeja fechava o que
acabara de abrir).
Verificação: clique numa linha → janela aberta, linha destacada, painel de
detalhes atualizado; duplo clique → cola e fecha.

### 8.3 Backspace apagava entrada do histórico

Sintoma: `Ctrl+A` + `Backspace` no campo de busca apagava uma entrada.
Diagnóstico: o atalho de apagar era `Ctrl+Backspace` e o handler global de
teclado não verificava o foco.
Solução: apagar virou `Delete` / `Ctrl+Shift+Delete` (valores antigos são
migrados no load), e `Backspace`/`Delete` **nunca** disparam ação de histórico
quando o foco está em input/textarea/contenteditable.
Verificação: digitar `abc`, `Ctrl+A`, `Backspace`, `Delete` → **0** chamadas de
`delete_entry`/`clear_entries` no log; três testes unitários cobrem defaults e
migração.

---

### 8.4 Colar demorava ~400 ms

Sintoma: duplo clique demorava para colar.
Diagnóstico: (a) `copy_entry` chamava `store.save()`, que reescrevia as ~1.450
entradas no banco a cada cópia; (b) `paste_entry` tinha `sleep` fixo de 220 ms e
`paste_into` outros 80 ms.
Solução: `save()` → `touch()` (uma linha, `INSERT ... ON CONFLICT`); o hide é
imediato e a thread de colagem **espera o alvo voltar ao primeiro plano** (poll
de 5 ms, teto de 250 ms) em vez de dormir.
Verificação: Enter → `Ctrl+V` entregue no app anterior em **29–48 ms**, com
cópia de texto em ~4 ms (antes ~50 ms).

### 8.5 Consoles piscando

Sintoma: processos auxiliares (`reg`, `schtasks`, PowerShell) faziam janelas de
console aparecerem por um instante.
Solução: **todo** processo filho é criado com `CREATE_NO_WINDOW` e
`Stdio::null()`; além disso, o hide trata console como caso ambíguo (confirmar
140 ms depois — se o terminal continuar na frente, esconde).

### 8.6 Seleção de texto sem arrastar a janela

Sintoma: selecionar texto no painel de detalhes arrastava a janela (região de
arraste nativa).
Solução: a região de arraste é apenas a **faixa do cabeçalho** (drag strip) e o
resto é conteúdo normal; `user-select: text` no painel de detalhes com
rolagem própria.

### 8.7 Miniaturas quebradas ao rolar

Sintoma: ao rolar rápido, algumas linhas apareciam sem imagem.
Diagnóstico: todas as imagens eram pedidas de uma vez, e o React reutilizava
nós `<img>` trocando o `src`.
Solução: lista virtualizada + `IntersectionObserver` + fila com concorrência
limitada e `key` estável por entrada; `mediaUrl(kind,id,v)` carrega um `v` na
URL para não servir cache velho (`Cache-Control: no-store`).
Verificação: 22/22 linhas visíveis com `naturalWidth > 0` e 0 quebradas,
inclusive com `scrollTop = 44922`.

### 8.8 Submenu "Save as…" fechando ao mover o mouse

Sintoma: ao descer para o submenu, ele voltava para "Paste/Copy…".
Solução: zona de tolerância (o submenu só fecha quando o ponteiro sai da
interseção dos dois painéis por mais de ~120 ms) e foco controlado pelo teclado.

### 8.9 Autostart elevado

Sintoma: `HKCU\...\Run` iniciava sem privilégios e o `Win+V` não era tomado.
Solução: Tarefa Agendada `\Rayclip` com `/RL HIGHEST` e `/SC ONLOGON`;
desligar remove a tarefa e a chave `Run`/`.lnk` para não duplicar.

### 8.10 Migração para o banco cifrado

Sintoma: dados antigos em `store.json` + PNGs em `images/`.
Solução: `store.migrate_legacy()` roda na primeira abertura dentro de
`BEGIN IMMEDIATE`: lê o JSON, cria as entradas, converte imagens/miniaturas em
blobs, move ícones/favicons para `media/` e **apaga** `store.json` e `images/`.
Verificação: 80 entradas migradas (34 imagens), arquivos antigos removidos,
`rayclip.db` com **zero** ocorrências de `SQLite format 3`, `\x89PNG`, `IHDR`,
`http` e `pichau`.

### 8.11 Importação do histórico do Raycast

Sintoma: precisávamos trazer o histórico real do Raycast, que grava **um arquivo
por formato** de cada cópia.
Solução: `import.rs` agrupa arquivos pela janela de mtime (`/2000*2000`, imagens
pela própria mtime), extrai texto de `CF_HTML` (`<!--StartFragment-->` e offsets
`StartFragment`), usa a mtime original como `created_at`, marca
`source = "Raycast"` e insere em transações de 200, emitindo `import:progress`.
O limite de histórico é **aumentado automaticamente** (100→…→5000) para não
cortar a importação, e duplicatas são ignoradas.
Verificação: **1.381 itens em 10,6 s** (1.272 imagens + 105 textos, 1 duplicata
ignorada), total 1.457 entradas, limite auto-ajustado para 2.000, todas as
miniaturas carregando do banco e "Save as…" devolvendo os bytes originais.

---

### 8.12 Edição inline no meio de uma lista virtualizada e cheia de atalhos

**Sintoma esperado** (ainda não aconteceu, mas o código convidava): após
`Ctrl+E`, uma trava global de teclado desativava a janela inteira se o campo
inline sumisse da tela (busca, rolagem, exclusão).

**Diagnóstico:** a primeira versão engolia **todas** as teclas sempre que estava
"renomeando". Se a linha em edição saísse da lista virtualizada, o estado ficava
preso e `Enter`/`Esc`/setas morriam com ela.

**Solução em camadas:**
1. O campo inline chama `stopPropagation()` para **todas** as teclas — `Enter`
   grava, `Esc` cancela, nada vaza para o app (setas continuam no campo).
2. A trava global só vale quando o **foco** está de fato no campo
   (`document.activeElement` tem a classe `rename-input`).
3. Um `useEffect` limpa o estado de renomear se o campo sumir do DOM (500 ms).

**Verificação:** renomear texto e imagem persiste e entra na busca; `Esc`
cancela sem gravar; `Backspace` dentro do campo **não** apaga entrada (0
`delete_entry`/`clear_entries` no log); `Esc` fora do renomear continua a
esconder a janela; painel `Ctrl+K` mostra "Rename Entry — Ctrl E".

### 8.13 A lista carregava a imagem inteira (o "travando" da rolagem)

**Sintoma:** rolar o histórico com muitas capturas de tela "engasgava"; abrir a
janela parecia lento com o banco grande.

**Medições (1.573 entradas):** 1.305 imagens, **348 MB** no banco, **nenhuma
miniatura** gerada — e a linha pedia a imagem inteira em `/img/<id>`, com
`Cache-Control: no-store`, então cada rolagem **decriptografava e transferia**
megabytes por linha. Além disso `entries_now` clonava `html`/`rtf` (34 MB)
segurando o mutex, e a capa do link (`og:image`) viajava como data URL de até
900 KB dentro do JSON da lista.

**Solução:**
- A linha sempre usa `/thumb/<id>`; o backend gera a miniatura de 64 px na
  primeira requisição e grava em `thumb_bytes`. Rolagem de 30 páginas passou a
  fazer **5 requisições** no total e o heap do renderer ficou em 17 MB.
- `/img`, `/thumb`, `/fav` e `/cover` respondem `Cache-Control: immutable`
  (a URL já carrega `?v=`), então nada é re-baixado.
- `entries_now` virou projeção explícita (só campos da UI; `html`/`rtf` fora).
- **Favicon e capa de link agora são arquivos** (`media/fav/<host>`,
  `media/covers/<hash>`) servidos por URL local. Uma migração no start converteu
  **24** entradas antigas: a listagem caiu de **3.014 KB para 1.727 KB** e o
  `list_entries` de **148 ms para 83 ms** (debug).
- Texto da lista: 16 KB → **4 KB** por entrada; o painel de detalhes busca o
  texto completo por `entry_text` (verificado: 4.096 na lista, **40.375** na
  prévia).

**Verificação:** seleção e rolagem sobrevivem a `Esc` + reabrir; 0 long tasks ao
rolar; todas as linhas visíveis com favicon carregado.

### 8.14 Ícone do site nos links (o link do Discord sem logo)

**Sintoma:** `https://cdn.discordapp.com/attachments/...` aparecia com o clipe
genérico; a captura do usuário mostrava `cdn.discordapp.com` sem ícone.

**Diagnóstico:** a busca de metadados baixava a URL como se fosse HTML (no CDN
era um `.mp4`), não achava `<meta>` nem `<link rel=icon>` e caía em
`cdn.discordapp.com/favicon.ico`, que responde **403**. E o ícone ia como data
URL **em cada entrada**.

**Solução:**
- `HEAD` antes do corpo: se o `Content-Type` não é HTML, nada de baixar 700 KB —
  a entrada recebe só host + favicon.
- Favicon por **host** com cache em disco e cadeia de candidatos:
  `host` → domínio registrável → `www.registrável`, em `/favicon.ico`,
  `/favicon.png` e `/apple-touch-icon.png` (sondagem por `HEAD`, `GET` só do que
  existe); se nada existir, lê a **página inicial** do domínio e usa o
  `<link rel="icon">` declarado — foi o que resolveu o Discord
  (`discord.com/assets/favicon.ico`, 256×256).
- Página inacessível (403, timeout) gera metadados **mínimos** (host + ícone) e
  a entrada deixa de ser re-tentada em toda abertura; `link_pending` tem duas
  filas (ícones primeiro, metadados depois).
- `/fav/<host>` se **auto-conserta**: se o arquivo sumiu, baixa uma vez na hora.
- UI: componente `SiteIcon` com `onError` → tile com a inicial do domínio, para
  nunca aparecer ícone quebrado nem clipe genérico.

**Verificação:** log `link: icone de https://cdn.discordapp.com/...`; arquivos
`media/fav/{cdn.discordapp.com,discord.gg,...}` (ICO completo, 285 KB); DOM com
`img.favicon` 256×256 carregado; captura da janela mostra o logo do Discord na
linha **e** no cartão de detalhes.

### 8.15 Como o Raycast cola "na hora" (e o que mudamos)

Investigação no binário instalado (2.13.1.0) — strings de `Raycast.dll`,
`native.dll` e `Raycast.UIAccess.exe`:

| Peça do Raycast | O que faz | Nós tínhamos |
|---|---|---|
| `Raycast.UIAccess.exe` + named pipe `RaycastUIAccessRPC` | **helper elevado com UIAccess** que executa `SendInput` — cola até em janela elevada, sem esbarrar em UIPI | nenhum helper: `keybd_event` do processo normal |
| `rpc_simulate_paste` (`native.dll`, `kb_input_simulation.rs`) | o processo principal **pede a colagem** ao helper e espera a confirmação | `paste_into` local |
| `AllowSetForegroundWindow` + `force_activate_window` + `foreground cloaked retry worker` | garante que o alvo **é** o primeiro plano (com retry para janela "cloaked" do DWM, típico de apps da Store) | `SetForegroundWindow` + sondagem de 5 ms até 280 ms |
| `SendInput` | 4 eventos em **uma chamada atômica** | 4 × `keybd_event` (outra tecla podia se intrometer) |
| `set_clipboard_to` + `set and verify` + `Held the clipboard open for …` | escreve, **confere** e mantém o clipboard aberto até o alvo ler | escreve e segue |
| `ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory` | marca o conteúdo para o histórico do Windows não duplicar | não marcamos |
| `_textPasteLock`, `_textPasteRunning`, `RunCoalescingPasteLoop` | **serializa** e junta pedidos de colagem; `waitForPaste`, `PasteDelay`, `MaxPasteAttempts` | uma thread por colagem, sem retry |

**O que mudou na v0.1.10:**
1. `win32::paste_into` → um único `SendInput` (`send_ctrl_v`).
2. Caminho rápido sem espera: se o alvo já está em primeiro plano, **0 ms** de
   sondagem (antes eram 8 ms fixos + até 280 ms em passos de 5 ms).
3. `AttachThreadInput` imediato quando precisa puxar o foco + sondagem de 1 ms
   (até 30 ms) + **nova tentativa** se o primeiro `SendInput` não sair com o alvo
   na frente.
4. `allow_foreground_any()` (`AllowSetForegroundWindow(ASFW_ANY)`) quando a
   janela aparece — devolve o foco ao app anterior sem cair na foreground lock.
5. `PASTE_LOCK`: uma colagem por vez (`paste_many` e `paste_entry` não se
   cruzam mais).
6. `paste_many` sem as esperas fixas de 220 ms + 60 ms: esconde na hora
   (`hide_now`) e começa a colar; intervalo entre itens 340 → 300 ms.

**Medido (build debug, alvo = Bloco de Notas):** escrita no clipboard **3,9 ms**;
primeiro `SendInput` **+19 ms** depois da chamada; `espera=0ms`; segundo item com
**638 µs** de injeção; texto chegou no Notepad (conferido por captura). Antes, o
`paste_many` de 2 itens levava ~700 ms só de esperas.

**Pendente de propósito:** helper elevado/UIAccess (colar em janelas elevadas),
marcar o clipboard para o Windows não duplicar, e juntar (coalescer) pedidos
repetidos.

### 8.16 O app roda elevado por padrão (e por que isso resolve a colagem)

Colar em janela elevada (Gerenciador de Tarefas, console de administrador,
editor aberto como admin) é bloqueado pelo **UIPI**: um processo de integridade
média não consegue injetar `SendInput` numa janela de integridade alta. O Raycast
contorna isso com um helper `UIAccess` separado. A escolha aqui é mais simples:

**o Rayclip inteiro é elevado.**

1. **Manifesto** — `build.rs` gera o manifesto do binário com
   `<requestedExecutionLevel level="requireAdministrator" />` **só no profile
   release**; o build de debug continua `asInvoker`, para os testes automatizados
   (um processo não elevado não consegue mandar teclado para um elevado).
   Conferido no binário: release tem `requireAdministrator`, debug não tem.
2. **Nunca rodar rebaixado** — `run()` começa com uma rede de segurança: se o
   processo subiu **sem** elevação e a tarefa existe, ele se reabre por ela e
   sai. Assim, mesmo um build sem manifesto nunca fica sem elevação.
3. **Abrir sem UAC** — `sysinteg::ensure_launch_task()` registra a tarefa
   `RayclipElevated` (sem gatilho, `/RL HIGHEST`) apontando para o exe atual.
   `schtasks /Run /TN RayclipElevated` abre o app **já elevado, sem prompt**.
   Se o caminho da tarefa não for o do exe em uso (build novo, versão instalada),
   ela é recriada — senão o atalho abriria a versão antiga.
4. **Autostart** continua na tarefa `Rayclip` (`ONLOGON` + `/RL HIGHEST`):
   elevado desde o login, sem UAC.

**Medido** (build release elevado, alvo = Bloco de Notas **aberto como
administrador**): `SystemStatus.elevated=true`, colagem com
`espere=0ms total=2,2 ms` e o texto chegou no editor — o caso que antes falhava
em silêncio.

**Custo aceito:** UAC no início manual (autostart e a tarefa acima não pedem),
e arrastar arquivos do Explorer para dentro do app fica bloqueado pelo UIPI
(a direção contrária, arrastar para fora, também).

### 8.17 Prévia no diálogo "Save N items" (coluna fixa + Quick Look)

O diálogo mostrava só um ícone genérico por linha: com três imagens selecionadas
não dava para saber qual era qual.

**Primeira tentativa (descartada):** um tooltip que seguia o ponteiro. Ficou
errado na prática — como o diálogo ocupa quase toda a janela, o tooltip caía
sobre o cabeçalho/linhas e mudava de lugar a cada movimento do mouse (flagrado
na `Screenshot_24.png`).

**Como é agora, imitando o Quick Look do macOS** (pesquisado na documentação da
Apple: `Espaço` abre a prévia grande, `Espaço`/`Esc` fecha, setas trocam o item
**com a prévia aberta**, e ela fica no mesmo lugar, sem seguir o cursor):

1. **Coluna de prévia fixa** (272 px) à direita da lista, separada por divisória:
   título da entrada, palco preto com a imagem (`object-fit: contain`) ou o texto
   monoespaçado com fade, e rodapé com tipo/contagem + dica "Space to enlarge".
   Passar o mouse numa linha **atualiza a coluna** — nada flutua, nada cobre o
   cabeçalho, e a linha em prévia ganha contorno (`data-previewed`).
2. **Ampliação estilo Quick Look**: `Espaço`, duplo clique na linha ou clique no
   ícone abrem um card **sempre no centro do diálogo** (600×420 máx) com fundo
   escurecido. `Espaço`/`Esc` fecham; `↑`/`↓` trocam a entrada com o card aberto;
   clicar fora fecha. `Espaço` é ignorado enquanto o foco está num campo de
   extensão (senão digitava espaço em vez de abrir).
3. **`Esc` em camadas**: com o card aberto, o primeiro `Esc` fecha o card; o
   segundo fecha o diálogo.

**Medido (debug, via CDP):** diálogo com 3 linhas → coluna fixa presente e 2
miniaturas reais nas linhas; `Espaço` → card **600×373 em x=150, y=58** (mesmo
lugar sempre), imagem de verdade (`naturalWidth` 773×425); na linha de texto o
card mostra `<pre>` com o caminho e o título; `↑` troca de item com o card
aberto; `Esc` fecha o card **e mantém o diálogo**.

## 9. Build, release e depuração

### Comandos

```bash
npm install
npx tauri build --debug --no-bundle   # binário debug (~55 s)
npx tauri build                       # release + instalador NSIS (~150–240 s)
npx tsc --noEmit                      # checagem de tipos do frontend
cargo test --lib                      # testes do Rust (sqlite, settings, update)
```

Publicar uma versão (gera release no GitHub, de onde o app se atualiza):

```bat
deploy.bat                 :: 0.1.24 -> 0.1.25, build, hash, push, release
deploy.bat 0.2.0           :: versão explícita
deploy.bat --build-only    :: só compila e escreve rayclip.exe.sha256
```

O `deploy.bat` usa `_ref/get_version.ps1` e `_ref/bump_version.ps1` (UTF-8 sem
BOM — o serde do build recusa BOM no `tauri.conf.json`). Assets publicados:
`rayclip.exe` (é o que o updater baixa), `rayclip.exe.sha256` e
`Rayclip_X.Y.Z_x64-setup.exe`.

**Regra combinada com o dono do projeto:** depois de **todo** update (build,
correção, ajuste), **fechar e reabrir o app** — nunca deixar o binário antigo
rodando. Sequência padrão:

```bash
powershell -NoProfile -Command "Stop-Process -Name rayclip -Force -ErrorAction SilentlyContinue"
rm -f "$TEMP/rayclip.log"
powershell -NoProfile -Command "Start-Process 'C:/Users/Lugia/Projects/rayclip/src-tauri/target/debug/rayclip.exe'"
```

Armadilhas conhecidas:

- **Trava do `rayclip.exe` em uso**: `Stop-Process -Name rayclip -Force`
  primeiro (senão `os error 5` no link).
- `npx tauri dev` serve `http://localhost:1420`; se o servidor não subiu, a
  janela fica em branco — em máquina real use sempre `--debug --no-bundle`.
- O `build.rs` chama `build_sqlcipher()` **antes** de `tauri_build::try_build`.
- Pode sobrar `src-tauri/.cargo/config.toml` com `PERL5LIB` da tentativa antiga
  de OpenSSL — inofensivo, não é usado.
- Debug remoto (verificação): colocar `"additionalBrowserArgs":
  "--remote-debugging-port=9222"` na config da janela **apenas** para testar e
  remover antes do release (`grep -c remote-debugging` deve retornar 0).

### Log de depuração

`%TEMP%\rayclip.log` só existe em build debug, com **timestamp em ms** (é assim
que se mede latência). Entradas típicas:

- `show_window hwnd=…`, `hook: instalado ok=true`, `mouse-hook: instalado ok=…`
- `click-hook: hide (pid=… desde_show=…)`, `foreground-hook: hide […]`,
  `recheck: hide […]`, `hook: console … confirmando em 140 ms`
- `paste_entry: copiado em …`, `paste_into: alvo=… fg_ok=…`
- `db: N entradas (cifrado=true) …`, `import: X de Y itens`, `key: …`
- `source: <exe> (self=…)`, `clipimg: 976x721 dib=… v5 ok bitmap ok`

### Receita de inspeção do banco

O conteúdo não pode aparecer em disco; para conferir:

```python
open(db,'rb').read()  # e contar ocorrências de
# b"SQLite format 3", b"\x89PNG", b"IHDR", b"IDAT", b"GIF8", b"http" — tudo 0
```

### Medições automatizadas (scripts em `_ref/` e `/tmp`)

- `measure_hide*.ps1` — mostra a janela e mede ms até `!IsWindowVisible` após
  clicar num alvo.
- `measure_fast2.ps1 <x> <y> <delay>` — mostra e clica `<delay>` ms depois;
  compara com o `desde_show` do log do app.
- `pastelat.ps1` — formulário WinForms alvo: Enter no app → mede o texto chegar.
- `uia_*.ps1` — inspeção do WebView via UI Automation (houve timeouts: o UIA do
  WebView2 é instável; prefira screenshot + `look_at_image`).

---

## 10. Limitações conhecidas

- Imagens lidas por `CF_BITMAP`; alguns apps só expõem `CF_DIB`/PNG (fallback:
  aparece sem prévia ou como texto).
- OCR depende dos pacotes de idioma do Windows e reduz imagens acima de 2400 px.
- O banco cifrado (v0.1.4+) não é lido por versões antigas; `store.json` antigo
  é migrado automaticamente na primeira abertura.
- Fundo translúcido depende dos efeitos de transparência do Windows; com eles
  desligados o app degrada para sólido e oferece um clique para ligar.
- 1–2 px de franja de cor na borda da janela em screenshots de captura de tela
  (artefato do seletor de janela, não um defeito de renderização).
- UIPI: sintetizar clique/tecla a partir de processo não elevado **não** alcança
  o binário release quando ele roda elevado (autostart) — testes de input usam o
  build debug.

---

## 11. Histórico de versões

| Versão | O que entrou |
|---|---|
| 0.1.0 | base funcional: captura, lista, busca, ações, janela, hooks |
| … | OCR, prévia de links, miniaturas, virtualização, bandeja, autostart, temas |
| 0.1.4 | SQLCipher (conteúdo cifrado), chave DPAPI, importação do Raycast |
| 0.1.5 | Backspace nunca apaga; hide rápido (hook); paste rápido (`touch`) |
| 0.1.6 | hide direto no Win32; hook global de clique; recheck em vez de descartar |
| 0.1.7 | fix do clique dentro da janela (comparação pela janela raiz) |
| 0.1.9 | renomear entrada (`Ctrl+E`) com edição inline, atalho configurável e ação no painel |
| 0.1.11 | prévia das linhas no "Save N items" (miniatura na linha; primeira versão com tooltip) |
| 0.1.28 | senhas: simulador de gerenciador de senha (`_ref/fake_pm.rs`), 13 cenários verificados, nomes genéricos no filtro (`is_ignored_app`) e o limite documentado (cópia sem janela não tem dono identificável) |
| 0.1.27 | auditoria de colar/apagar/exportar: comandos de teste desligados em release, pasta de destino validada, o plano B por `curl` não grava mais arquivo temporário e a limpeza cobre os três restos |
| 0.1.26 | auditoria de segurança: chave fail-closed (sem CSPRNG/DPAPI o banco não abre), OCR sem arquivo temporário, anti-SSRF na prévia de links, CSP fixa e asset protocol desligado |
| 0.1.24 | atualização automática pelas releases do GitHub (`Settings → Updates`): confere o SHA-256 publicado, troca o exe por rename (o que roda nunca é sobrescrito), reabre passando o PID antigo, rollback se o novo não subir; `deploy.bat` publica a release com `rayclip.exe` + `.sha256` + instalador |
| 0.1.23 | paridade com o Raycast no painel: reordenar fixados, editar conteúdo (`Ctrl+Shift+E`), copiar cor em HEX/RGB/HSL/HSV, apagar entradas recentes (5 min … 24 h) com confirmação e grupos de data "This Year"/"Older" |
| 0.1.22 | idioma da interface trocável (`Settings → Preferences → Language`): inglês (padrão) e português do Brasil, com ~270 textos traduzidos e fallback para o inglês |
| 0.1.21 | opção "Also capture passwords & protected content" (desliga o filtro de conteúdo sigiloso) |
| 0.1.20 | captura por listener do Windows (sem perder cópia em rajada), atalhos registrados um a um, harness de comparação com o Raycast |
| 0.1.19 | prévia do link com capa de verdade (og:image + miniatura do YouTube, bloco de site quando não há capa, re-tentativa de capa por execução) |
| 0.1.18 | buscador de metadados confiável: plano B por `curl.exe`, oEmbed do YouTube, `fetch_v`, filtro de página de bloqueio |
| 0.1.17 | apagar várias seleções com confirmação (lote via comando `delete_entries`); ação "Delete N Entries" no painel |
| 0.1.16 | dedup mantendo o id e hash só do conteúdo; busca ordenada por relevância (texto antes de OCR, fixado, exato/prefixo, recência); 1º resultado sempre selecionado |
| 0.1.15 | prévia e card grande com enquadramento correto (altura 100% resolvida em flex), cursor `grabbing` no arrasto |
| 0.1.14 | zoom só depois de **clicar** na imagem, 25%–800%, wheel não-passivo (não rola mais a coluna), card grande no botão ⤢ |
| 0.1.13 | zoom na imagem (scroll, arrasto) no painel e visualizador grande redimensionável (`Espaço`), reaproveitado no "Save N items" |
| 0.1.12 | prévia do "Save N items" em **lugar fixo**: coluna à direita + ampliação estilo Quick Look (`Espaço`/duplo clique, setas, `Esc`) |
| 0.1.10 | **app elevado por padrão** (`requireAdministrator` no release + tarefa `RayclipElevated` sem UAC + rede de segurança), colagem instantânea (`SendInput` atômico, 0 ms de espera no caso comum, `AllowSetForegroundWindow`, trava de colagem), lista deixa de carregar imagem inteira (miniatura sob demanda + cache imutável), favicon do site por host em arquivo (Discord corrigido), capa de link em arquivo, `system_status` com cache de 30 s (125 ms → 1 ms), seleção e rolagem preservadas ao reabrir |
| 0.1.8 | auditoria de riscos + performance do histórico com 1.457 itens: projeção magra (~34 MB → 1,9 MB por listagem), `entry_text` sob demanda, recarga com debounce e só com a janela visível, crash de stack overflow corrigido, banco à prova de apagão em leitura (modo degradado), backfill de OCR sequencial/retomável |

---

## 12A. Auditoria de riscos (v0.1.8)

### O que foi encontrado e corrigido

1. **Payload gigante em cada listagem.** `list_entries` mandava `html`/`rtf`
   (usados só dentro do Rust) + até 30 MB de `text` por item: **34 MB / 1,4 s**
   por invocação. Agora a lista manda no máximo **16 KB de texto por item**
   (1,9 MB / 100 ms no total) e o painel busca o texto integral sob demanda
   (`entry_text`), por seleção.
2. **Crash (stack overflow, `0xc00000fd`).** Uma substituição automática virou
   `notify_changed` em recursão infinita quando a janela estava visível ⇒ o app
   morria sozinho durante o backfill de OCR. Corrigido e verificado: sobrevive
   a minutos de indexação.
3. **`IsWindowVisible` mentia.** O Tauri mapeia a janela da criação (mesmo com
   `visible: false`), então "está visível?" respondia `true` sempre. O app agora
   rastreia a própria visibilidade (`SHOWN`), o que também destrava o filtro do
   item 4.
4. **Notificação de mudança a cada cópia, mesmo escondido.** Copiar com a janela
   fechada recarregava 1 MB+ de lista à toa (e, no estado anterior, recursava).
   Emissões agora só acontecem com a janela na tela; a UI junta os 2–3 eventos
   da mesma mudança (novo item/OCR/link) numa única recarga (debounce 140 ms).
5. **Apagão total por reação**: se a leitura do banco falhasse (corrupção, vírus,
   disco), `read_all` devolvia lista vazia e o próximo `save()` **apagaria todo
   o histórico**. Agora leitura com erro = modo **degradado**: `save()` só grava
   as linhas conhecidas, nunca remove nada.
6. **OCR é desnecessariamente repetido.** 702 imagens importadas sem texto eram
   re-indexadas a cada inicialização: milhares de threads disputando 2 vagas e
   desistindo em 20 s (o índice nunca terminava). Agora: tentativa falha é
   marcada com texto vazio (não re-tenta), e o backfill é **uma thread
   sequencial** que pausa enquanto a janela estiver aberta — termina de vez.
7. **`open_url` executava comandos.** Usava `cmd /C start` com o conteúdo do
   clipboard; um texto contendo `&`, `|` ou `%VAR%` virava execução. Agora só
   `http/https/mailto` via `ShellExecuteW`.
8. **Falhas de mutex envenenado** (panic numa thread segurando o lock) agora
   se recuperam (`into_inner`) em vez de travar o app para sempre.
9. **Temp de OCR (PNGs) limpos no início** — não acumula lixo após morte súbita.

### Verificado e sem risco

- Nomes de arquivo no "Save as…": caracteres `\ / : * ? " < > |` e de controle
  são trocados; não há subida de diretório (`..`).
- SQL sempre parametrizado (nenhum `format!` com valores em SQL).
- Sem `innerHTML`/`eval`/`dangerouslySetInnerHTML` no frontend; imagens e
  miniaturas só pelo `rayclip.localhost` (BLOBs do banco).
- Conteúdo continua só no banco cifrado; `media/` guarda apenas ícones/capas.

### Performance do histórico real (1.457 entradas, 391 MB, debug build)

| Medida | antes | depois |
|---|---|---|
| invocação `list_entries` | 1.436 ms | **101 ms** |
| JSON de resposta | 34.114 KB | **1.911 KB** |
| serialização (Rust) | 583 ms | **36 ms** |
| filtro de busca na UI | — | **2,1 ms** |
| texto enviado/entrada | até 13,6 M chars | ≤ 16 KB (integral sob demanda) |
| RSS do processo | 95 MB | 91 MB |

A lista continua com os campos que a UI pinta (título, prévia curta, OCR,
link, origem, tamanho); o que era gigante (`html`, `rtf`, `text` completo)
ficou fora do IPC.

---

## 12. Testes principais (resumo do README)

Captura de 7 tipos de conteúdo; OCR com busca por texto dentro da imagem e
persistência; busca vazia; hover no submenu entrega formato certo no diálogo;
troca de app → janela escondida em ~7–10 ms; bandeja e instância única; tarefa
agendada elevada; importação de 1.381 itens em 10,6 s; banco sem conteúdo
legível em disco; clique seleciona / duplo clique cola; `Backspace` na busca não
apaga nada.

Tabela completa de mais de 30 testes com resultados: ver `README.md`.
### 8.56 Espaço com a prévia aberta, hover desligado, vídeo e GIF de link (v0.1.41)

Quatro pedidos que vieram do uso.

#### 1. Com a prévia (Quick Look) na tela, a busca não digita mais

Era o campo de busca que continuava com o foco por baixo da imagem: as letras
(em especial o **Espaço**) entravam no filtro, e a lista mudava atrás da prévia.
Duas travas juntas:

- `NavBar` recebe `locked` e o `<input>` fica `readOnly` enquanto a prévia está
  aberta (corta a digitação, sem perder o texto que já estava lá);
- ao abrir (`Espaço`, `⤢` ou clique no painel) o app faz `inputRef.blur()`.

O `Espaço`/`Esc` continuam fechando a prévia: o handler do `ImageViewer` já roda
em fase de captura com `stopImmediatePropagation`, então não chega ao app.
Medido: com a prévia aberta, `a b c` não altera o filtro ('' e `readOnly=true`) e
`Espaço` fecha.

#### 2. "Preview on hover" passou a nascer desligado

Padrão trocado para `false` e, para quem já tem `settings.json` gravado, uma
migração de esquema desliga **uma vez** (`schema: 1`); depois disso o valor salvo
manda — quem gosta liga nas Preferências e fica ligado.

Motivo de existir `schema`: com `#[serde(default)]`, mudar o `Default` do struct
não muda o arquivo de quem já instalou. Medido: mover o mouse por três linhas
diferentes não muda mais a linha selecionada.

#### 3. Vídeo (e áudio) toca dentro do app

- **Rota `rayclip://video/<id>`** no protocolo próprio, com **Range**: o
  elemento de mídia do Chromium pede pedaços (`bytes=0-`, `bytes=1000-`, sufixo
  `bytes=-N`) para tocar, buscar posição e saber a duração. `resposta_midia()`
  responde `206` + `Content-Range`; sem `Range` devolve o começo como parcial
  (arquivo grande não vira 300 MB de memória). Range impossível → `416` com o
  tamanho real; extensão que não é mídia → `415`.
- **O caminho vem do banco**, nunca da URL: a rota só serve o `paths[0]` da
  entrada pedida, então a interface não ganha acesso a arquivo nenhum do disco.
- **Volume**: `muted` de propósito — abrir uma entrada não é pedido para tocar
  som; a barra do player tem o botão de volume, com a dica "Muted by default".
- `preload="metadata"` + poster da miniatura: mostra o primeiro quadro e a
  duração sem baixar o arquivo todo.
- CSP ganhou `media-src 'self' blob: http://rayclip.localhost` (sem isso o
  `<video>` cai no `default-src` e não carrega nada).

Medido com um `.mp4` de 240x480, 6,62 s: player presente, `muted=true`,
`currentTime` avançou para 1,15 s tocando de verdade.

#### 4. Link que **é** um GIF aparece animado (tipo Discord)

Antes, link direto de mídia (`https://media.giphy.com/....gif`) caía no ramo
"não é HTML" do `linkmeta` e saía só com host + favicon. Agora:

- `extensao_de_midia()` reconhece `.gif`/`.webp`/`.png`/`.jpg`/`.avif` (e
  `.mp4`/`.webm`/`.mov`, ainda sem player remoto) na URL;
- o `Content-Type` confirma e os bytes são baixados com o mesmo cliente
  anti-SSRF (teto de **8 MB**, `MIDIA_MAX`);
- os bytes vão **crus** para `media/covers/<hash>` e são servidos por
  `rayclip://cover/<hash>` — é por não reencodar que o GIF continua animado;
- capas de `og:image` passam pelo mesmo `guarda_capa()`, então GIF de página
  também anima.

Medido: link do giphy → capa de 715.492 B, magic `GIF8`, 480x270 na tela.

#### Testes

`cargo test --lib` → **59/59** (6 novos: mime por extensão, range normal,
sufixo, sem range, 416, 415; mais `extensao_de_midia`). `cargo clippy
--all-targets` → 0 avisos. E2E `_ref/teste_midia.py` no build **instalado**
0.1.41 → `RESULTADO: OK`.

#### De quebra: o app não subia

Durante os testes o app passou a "achar" que já havia outra instância e morria
sem janela. Causa: `CreateMutexW` **só seta** o último erro quando o mutex já
existe; quando cria um novo, o erro anterior continua lá — e `create_dir_all`
de uma pasta existente deixa `ERROR_ALREADY_EXISTS` (183). `already_running()`
agora faz `SetLastError(0)` antes. Sem isso, a gravação de configuração (nova,
por causa do `schema`) podia fazer o app não abrir.

### 8.54 Colar vários arquivos de uma vez (v0.1.40)

Multi-seleção já existia, mas só para **texto**: `paste_many` copia um item,
cola, copia o próximo, cola… Em três arquivos isso vira três colagens, e o
clipboard termina com o último. Não é o que o Explorer entende por "cole estes
três arquivos aqui".

Agora, quando **todas** as entradas marcadas têm arquivo (`paths` no disco ou
imagem guardada no banco), o comando novo `paste_files` faz o que o Windows
espera: **uma** `CF_HDROP` com todos os caminhos e **uma** colagem.

| Seleção | Caminho usado |
| --- | --- |
| tudo arquivo/imagem | `paste_files` — uma `CF_HDROP`, uma colagem |
| misto, texto, link, cor | `paste_many` — em sequência, como antes |
| "Paste as plain text" | sempre `paste_many` (arquivo não tem versão texto) |

O `FileList` do `clipboard-win` já era usado para colar **uma** entrada de
arquivo; era só agregar. Imagem que só existe no banco cifrado é materializada
em `<dados>/drag/<id>.<ext>` pelo mesmo caminho do arrasto — daí a extração do
`arquivos_da_entrada()`, agora usado pelo arrasto **e** pela colagem.

Sequência de eventos: escreve a lista → registra a sequência para o watcher não
re-capturar nossa própria escrita → sobe as entradas para o topo (ordem da
seleção) → esconde a janela (ou mantém, com `keep_open`) → **uma** colagem sob
`PASTE_LOCK` → evento `clipboard:multi-pasted`.

#### O defeito que a revisão pegou junto

`drag_files`/`paste_files` liam `entry.paths.first()`. Um item que representa
**três** arquivos (copiar três no Explorer cria um item só, `kind = "files"`)
arrastava e colava **um** dos três, em silêncio. Agora `arquivos_da_entrada()`
devolve todos os caminhos, e a colagem junta os de todas as entradas marcadas na
ordem da seleção.

#### Medição

- `_ref/teste_colar_arquivos.py`: três arquivos copiados separadamente (três
  itens do histórico) → Ctrl+clique nos três → `Enter` com o Explorer na frente
  → **um** `paste_files n=3` no log, uma colagem, os três arquivos na pasta com
  bytes idênticos. `RESULTADO: OK` no build instalado 0.1.40.
- Armadilha do harness que vale registrar: `Ctrl` apertado **antes** do primeiro
  clique vai para o app que tinha o foco (o Explorer), e o WebView recebe o
  clique com `ctrlKey=false` — a seleção é substituída em vez de somada. O Ctrl
  tem que descer depois que o app já está em foco. E `data-selected` marca só a
  linha primária: quem conta seleção múltipla é `data-multi`.
- `cargo test --lib`: **52/52**; `cargo clippy --all-targets`: **0 avisos**.

### 8.55 Revisão do código inteiro: quatro defeitos e o que ficou de fora

Passada de revisão sobre `src-tauri/src` e `src` (17 mil linhas).

1. **Atualizador podia pendurar para sempre.** O teste
   `update::tests::exe_que_nao_sobe_volta_para_o_antigo` ficou mais de 60 s
   "rodando" — o `CreateProcess` de um arquivo que não é PE às vezes abre caixa
   de erro do Windows em vez de devolver erro, e o app esperava. Agora
   `parece_exe()` confere `MZ` + assinatura `PE\0\0` (2 bytes e 4 no offset
   0x3C) **antes** de lançar; arquivo truncado ou em quarentena vira a mensagem
   "the downloaded file is not a valid program" e o rollback do `.old` acontece
   na hora. Era bug de verdade, escondido atrás de um teste que travava.
2. **Entrada de vários arquivos entregava um só** (ver §8.54).
3. **Código morto perigoso.** `strip_caption()` — o caminho `WS_POPUP` que
   **quebra** `WindowFromPoint` (todo clique escondia a janela) — continuava no
   arquivo com um comentário "não usar". Foi apagado junto das constantes
   `WS_CAPTION`/`WS_THICKFRAME`/`WS_POPUP`/`WS_SYSMENU`; o escolhido,
   `strip_caption_sem_quebrar()` (DWM), ficou. Saíram também `cursor_point()` e
   `window_rect()` (diagnóstico sem uso).
4. **Mensagens de erro em dois idiomas.** Strings de retorno iam direto ao toast
   (`"arquivo sumiu do disco"`) no meio de uma UI em inglês. Padronizadas em
   inglês, com o teste de caminho de rede acompanhando.

Também: `matches_query`/`matched_only_in_ocr` só existem para os testes →
`#[cfg(test)]`; quatro `return` finais viraram expressão de cauda; teste do
`Store` usa `..Default::default()`.

**O que ficou de fora, de propósito:** `lib.rs` tem ~3.800 linhas e mistura
comandos de colagem, de salvamento, de OCR, de link e a montagem da janela. A
divisão em módulos é mecânica mas mexe em ~40 comandos; fazer isso junto com
funcionalidade nova aumenta o risco sem ganho visível. Fica como tarefa própria,
com o harness atual como rede de proteção.

### 8.53 Arrasto grande fechado, verificado e publicado (v0.1.39)

Fecho do ciclo do §8.50. A solução que ficou **não** é OLE à mão: é o crate
`tauri-plugin-drag` (`@crabnebula/tauri-plugin-drag` no frontend +
`tauri-plugin-drag` no Rust, capability `drag:default`).

#### Como o arrasto escolhe o caminho

| Entrada | Caminho do arrasto |
| --- | --- |
| arquivo no disco (`paths` não vazio) | **OLE/`CF_HDROP`** com o caminho original |
| texto que **é** um caminho de arquivo | o arquivo (caminho), não o texto |
| imagem/GIF que só existe no banco cifrado | materializada uma vez em `<dados>/drag/<id>.<ext>` e arrastada por caminho |
| texto, link, cor | `DownloadURL` do Chromium + `text/plain` (`blob:`, nunca `data:`) |

Decisões que custaram medição e não devem ser desfeitas:

- a linha que é **arquivo** não recebe o `draggable` do Chromium: dois arrastos
  ao mesmo tempo (o do navegador e o do sistema) travavam a janela;
- o gesto do sistema começa no `pointermove` com limiar de **4 px** (clique que
  treme continua clique) e é decidido pelo **tipo da entrada**, de forma
  síncrona — depender de o caminho já ter vindo no hover fazia o gesto rápido
  cair no caminho do Chromium, que é o que falha acima de ~3 MB;
- caminho de rede (`\\servidor\...`) é recusado: é o bug aberto drag-rs #72 e
  derruba o processo;
- `_arrastando`/`ARRASTANDO` impedem a janela de esconder e a lista de
  re-renderizar durante o gesto, com rede de segurança de 60 s (`fim()`), senão a
  janela pararia de esconder para sempre.

#### Medições desta máquina (24/11, v0.1.39)

- `.exe` de **25.752.442 B** arrastado para uma pasta do Explorer: arquivo criado
  com nome e extensão certos, **25.752.442 B** (`_ref/teste_arrasto.py` →
  `RESULTADO: OK`). Na VM do teste, medido até **120 MB**.
- `_ref/teste_pos_arrasto.py` (regressões depois do gesto): janela continua
  visível, o clique seguinte ainda seleciona (id esperado = id selecionado) e
  `Alt+Space` volta a esconder/mostrar — `RESULTADO: OK`.
- `cargo test --lib`: **47/47** (4 testes novos: caminho local, arquivo sumido,
  recusa de rede, e o payload de texto).
- `npx tsc --noEmit`: 0 erros.
- Release instalada em silêncio (`/S`): `FileVersion` **0.1.39**, porta **9222
  fechada** com o app aberto.
- Confirmado pelo usuário em uso real: arrasto de arquivo grande, GIF, imagem e
  texto, temas nos 5 esquemas e Preferências — "tudo funciona".

#### DevTools sob demanda (§8.51) muda a rotina de teste

`RAYCLIP_CDP=1` (ou o número da porta) liga o CDP **só naquela execução**, pela
configuração em memória, e o `tauri.conf.json` fica sem `additionalBrowserArgs`.
Consequência prática: **não existe mais** o passo "tirar a flag antes do release";
o build de teste e o de release são o mesmo arquivo. `_ref/cdp_env.py` monta esse
ambiente e `_ref/reiniciar.ps1 -Cdp` já sobe o app do jeito certo.

#### Lixo de teste que ficou no disco

- `C:\Users\Lugia\Downloads\AstralRinth App_0.22.0_x64-setup (1).txt` — 25 MB,
  é o `.exe` com nome errado, criado pelo bug do v0.1.37 (pode apagar).
- entradas de teste no histórico (`teste_3mb.zip`, `teste_25mb.zip`,
  `TESTE ARRASTO TEXTO 987654`) e arquivos do `%TEMP%` (`teste_*mb.zip`,
  `alvo_arrasto/`) — apagar com `Delete` na lista.

### 8.52 Dois defeitos que o print mostrou: hora do nome e OCR (v0.1.39)

O print da janela tinha duas informações que não fechavam: a imagem estava listada
como 21:11 e o arquivo arrastado se chamava `Rayclip image 2026-11-25 00-12-26`
(dia seguinte, três horas no futuro); e o painel dizia "No text detected" numa
imagem cheia de texto.

#### O nome do arquivo saía em UTC

`chrono_label()` fazia a conversão civil a partir do epoch **UTC** — o comentário
do código até admitia. Numa máquina em UTC-3, capturar às 21:12 gerava nome de
00:12 do dia seguinte. Agora usa `GetLocalTime` (horário do sistema, já com
horário de verão resolvido): o mesmo gesto gera `Rayclip image 2026-11-24
21-31-44.png`.

#### O OCR nunca rodava, por dois motivos

1. **Perfil de idiomas vazio.** O código usava só
   `OcrEngine::TryCreateFromUserProfileLanguages()`. Nesta máquina
   `Get-WinUserLanguageList` está vazia, a chamada devolve erro e **toda** imagem
   virava "sem texto" — com o pacote `Language.OCR~~~pt-BR` instalado do lado.
   Agora há plano B: se o perfil falhar, usa o primeiro idioma de
   `AvailableRecognizerLanguages()` (preferindo `pt`).
2. **O componente de OCR não existe neste Windows.** Mesmo com motor escolhido,
   o WinRT responde `Class not registered` (`0x80040154`):
   `C:\Windows\System32\Windows.Media.Ocr.dll` **não existe** (está só em
   `WinSxS`, sem link), e a classe `Windows.Media.Ocr.OcrEngine` não está em
   `HKLM\SOFTWARE\Microsoft\WindowsRuntime\ActivatableClassId`. Isso é estado
   da máquina, não do app: só conserta com reparo do Windows
   (`DISM /Online /Cleanup-Image /RestoreHealth`, ou `Add-WindowsCapability`).

O que o app faz agora, com razão: `ocr::disponivel()` testa o motor uma vez e o
comando `ocr_disponivel` deixa a interface dizer **"OCR indisponível neste
Windows"** em vez de "Nenhum texto detectado" — que era uma mentira confortável.
Além disso, imagem nenhuma é marcada como "já tentada" quando o motor não existe:
se o componente for instalado depois, o índice se completa no próximo início, em
vez de ficar vazio para sempre.

Ainda não medido: OCR funcionando de verdade (precisa de Windows com o
componente). O teste para isso é arrastar/copiar uma captura com texto e conferir
que a linha aparece na busca pelo texto de dentro da imagem.

### 8.51 DevTools sob demanda, sem flag no build (v0.1.39)

Até aqui, medir qualquer coisa pela interface exigia colocar
`"additionalBrowserArgs": "--remote-debugging-port=9222"` no `tauri.conf.json`
e **lembrar de tirar** antes do release (era até uma regra do plano). O build de
teste e o publicado eram arquivos diferentes por causa de uma flag.

Agora a porta existe só quando alguém pede:

```powershell
$env:RAYCLIP_CDP = "1"          # ou o número da porta
$env:RAYCLIP_CDP = "1"; src-tauri\target\debug\rayclip.exe --open
```

- `aplicar_cdp()` no `lib.rs` mexe na configuração **em memória**, antes de o
  `run` criar as janelas; sem a variável, nada muda e o binário é o de release.
- Sempre em loopback (`--remote-debugging-address=127.0.0.1`): não fica um canal
  de controle da interface aberto para a rede.
- Os argumentos passados incluem os padrões do wry
  (`--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`,
  `--autoplay-policy=no-user-gesture-required`): `additional_browser_args`
  **substitui** o padrão do wry em vez de acrescentar.

O caminho “óbvio” não funciona e ficou registrado para não ser tentado de novo: a
variável `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` é **ignorada**, porque o wry
sempre define os argumentos por conta própria (medido: porta não abre, e o
`--remote-debugging-port` não aparece na linha de comando do `msedgewebview2.exe`).

Medido: sem a variável, `http://127.0.0.1:9222/json/list` recusa conexão; com
`RAYCLIP_CDP=1`, responde 200 com 1 alvo, e a linha de comando do WebView2 ganha
`--remote-debugging-address=127.0.0.1 --remote-debugging-port=9222`.

Os testes acompanharam: `_ref/cdp_env.py` (`env_cdp()`) passa a variável para o
app que eles mesmos sobem, e `_ref/reiniciar.ps1 -Cdp` faz o mesmo no teste
manual. Nenhuma edição de configuração durante a medição.

### 8.50 Arrastar arquivo grande para fora: o teto era o Chromium (v0.1.39)

O relato era curto: "pegar um `.zip`/`.exe` de 25 MB e arrastar para uma pasta não
faz nada". Até 3 MB funcionava; acima disso, nada era criado — e sem nenhum erro.

#### A causa

O arrasto saía pelo `DownloadURL` do Chromium, que é a receita que vira
`CF_HDROP` no Explorer — mas ele carrega os **bytes** do conteúdo. Entre 3 e
25 MB o Chromium simplesmente desiste. O caminho certo para arquivo que já está
no disco é OLE (`DoDragDrop` + `CF_HDROP` apontando para o **caminho**): não copia
bytes, não tem teto e não passa pela memória do app.

Tentar isso à mão já tinha travado cinco vezes (§2 do `PLANO_VM.md`):
`SHDoDragDrop` nunca voltava e o `QueryContinueDrag` nunca era chamado. A leitura
mais provável, que as tentativas não mediram: `SetCapture` feito de uma thread
que **não** é a dona da janela sob o cursor (a do WebView2) não captura nada, e
sem captura não chega mensagem de mouse — o laço espera para sempre.

#### O que resolveu

Não foi reescrever OLE à mão: foi o **`tauri-plugin-drag` 2.1.1** (CrabNebula),
que já faz exatamente esse desenho em produção (Rust + `@crabnebula/tauri-plugin-drag`
no frontend, permissão `drag:default`).

- **Entrada que é arquivo** (`paths`, ou tipo `file/files/pdf/video/audio/archive`):
  o comando novo `drag_files` devolve o **caminho** e a linha chama `startDrag`.
  Nenhum byte é lido: o log, que antes mostrava `drag payload: … 26214400 bytes`,
  agora mostra só `drag: comecou`.
- **Imagem/GIF** (que só existe no banco cifrado): materializada uma vez em
  `%APPDATA%\com.rayclip.app\clipboard\drag\<nome>.png` e o arrasto leva o
  caminho. Custo aceito: um arquivo a mais no disco do app.
- **Texto/link/cor**: continuam no `DownloadURL` do Chromium (funcionam, e o
  `text/plain` é o que faz o Notepad/Word receberem texto em vez de arquivo).
- **Caminho de rede** (`\\servidor\...`) é recusado: o plugin derruba o app com
  UNC (bug aberto do drag-rs #72). Melhor não arrastar do que fechar sozinho.

O caminho do sistema começa no **pointermove** (limiar de 4 px, como nos
gerenciadores de arquivo), não no `dragstart`: começar de dentro do gesto do
Chromium é justamente a situação que travava. Por isso a linha que é arquivo sai
com `draggable={false}` — dois arrastos ao mesmo tempo (o do navegador e o do
sistema) travam a janela.

A escolha é feita pelo **tipo da entrada**, de forma síncrona. A primeira versão
decidia pelo caminho já ter chegado (prefetch do hover); num "pegar e arrastar"
rápido o caminho ainda não tinha chegado e a linha caía de novo no Chromium — o
bug voltava para quem arrasta rápido, que é o caso normal.

#### Medido (build debug, Explorer à direita do app)

| Entrada | Antes | Depois |
| --- | --- | --- |
| `.zip` 3 MB → pasta | OK | OK, 3.145.728 B, `sha256 ff5a7aa5…` |
| `.zip` 25 MB → pasta | nada criado | OK, 26.214.400 B, `sha256 995f4dfe…` |
| `.exe` 25 MB → pasta | nada criado | OK, 26.214.400 B, nome e extensão certos |
| imagem do banco → pasta | OK (497 KB) | OK, PNG de 2.399 B (materializado em `drag/`) |
| texto → Notepad | OK | OK (inalterado) |
| arquivos escritos em `%TEMP%` | 0 | 0 (só o arquivo de staging das imagens, no disco do app) |
| janela durante o arrasto | visível | visível (`IsWindowVisible` = true) |
| clique depois do arrasto | seleciona | seleciona |
| `Alt+Space` depois do arrasto | funciona | funciona |

O gesto apressado (`SEM_HOVER=1` no `_ref/teste_arrasto_ole.py`, sem os 250 ms de
hover) foi medido à parte: 3 MB e 25 MB passam igual.

#### Erros do harness (de novo meus, não do app)

1. Esperar a janela por 4 s: nesta máquina o WebView2 leva ~6 s para abrir — o
teste dizia "app nao abriu" sem motivo. Agora são 30 s.
2. Medir a lista **antes** do primeiro `search_entries` devolvia "0 linhas" e
   parecia que o filtro não funcionava.
3. Com o Explorer em primeiro plano, o app abria e escondia na hora (é o
   comportamento normal: perdeu o foco). O teste tem que trazer a janela para
   frente antes de medir, não depois.

#### Limpeza que entrou junto

O log de diagnóstico `hook diag: pt=(…)` do hook de clique saiu. Não era só log:
os argumentos do `format!` chamavam `cursor_point` (2×), `class_of_hwnd`,
`foreground_hwnd` e `window_rect` a **cada clique em qualquer janela**, e em
release também — o `log` descarta a mensagem depois, mas os argumentos já foram
calculados.

### 8.49 Arrastar para fora, temas quebrados e moldura nativa (v0.1.37/0.1.38)

Três pedidos do usuário, cada um com uma causa diferente — e, de novo, quase
todo o tempo gasto foi no **harness**, não no app.

#### (a) Arrastar uma entrada para fora do app

O Explorer só aceita arquivo (`CF_HDROP`). A primeira tentativa foi `DoDragDrop`
por OLE com `SHCreateDataObject`, que **nunca voltava**: o laço modal precisa da
fila de mensagens da thread que *possui* a janela, e a janela aqui é do WebView2.
Rodar na thread principal também travou, e durante o arrasto a janela aparecia
como invisível (`IsWindowVisible` = `false`) sem que nada no código a escondesse.

O caminho que funciona é o **arrasto do próprio Chromium** (`DownloadURL`):

```ts
e.dataTransfer.setData("DownloadURL", `${mime}:${nome}:${url}`);
```

- `url` é um **`blob:`** criado de base64, não um `data:` — um `data:` de 500 KB
  fazia o Chromium cancelar o arrasto no mesmo instante (`dragstart` seguido de
  `dragend`, arquivo nenhum criado);
- o payload vem do Rust (`drag_payload`) e é **adiantado no hover** (250 ms
  parado) porque o `dragstart` precisa dele síncrono; sem payload pronto o
  `onDragStart` chama `preventDefault()`;
- o cache é um `Map` por id, fora do componente: durante o arrasto o mouse
  atravessa outras linhas e cada `mouseenter` pediria outro payload;
- `text/plain` (e `text/uri-list` para links) vai **junto** do `DownloadURL`,
  então soltar no Notepad/Word/Discord cola o texto, e soltar numa pasta cria o
  arquivo;
- `ARRASTANDO` (flag no Rust) e `arrastandoAgora()` (no frontend) impedem que a
  janela esconda e que a lista se re-renderize no meio do gesto — re-renderizar
  troca a linha sob o cursor e cancela o arrasto.

Medidos: imagem de 500 KB -> arquivo de **497.252 bytes** idêntico na pasta do
Explorer; texto -> Notepad com o texto exato (Ctrl+A, Ctrl+C, `Get-Clipboard`);
`drag payload` de 18 a 40 ms (antes, reencodando PNG, 170 ms).

O travamento relatado ("não respondendo" ao clicar num `.zip`) era o payload
sendo lido **síncrono no hover**: `fs::read` do zip inteiro + base64 na hora.
Agora `drag_payload` é `async` e roda em `spawn_blocking`, com teto de
**128 MB** (`MAX_ARRASTO`; acima disso o arrasto é recusado e fica só uma linha
no log). Medido
com um zip de 120 MB: hover e clique com `SendMessageTimeout` + `IsHungAppWindow`
respondendo sempre.

Uma entrada de texto que **é** um caminho de arquivo (`"C:\...\foto.png"`) agora
arrasta o arquivo de verdade: antes saía `... .txt` com o caminho dentro. A
extensão, o nome e o tipo vêm do arquivo (`caminho_do_texto`), e o mesmo vale
para entradas com `paths`.

#### (b) Texto ilegível nos temas claros

`_ref/audit_temas.py` percorre cada nó de texto nos 5 temas, descobre a cor de
fundo realmente pintada subindo o DOM e calcula contraste WCAG. Causa: superfícies
fixas em tema escuro (`.panel` `rgb(24 24 26 / .97)`, `.filter-menu`,
`.dropdown-menu`, `.settings-toast`). Corrigido com os tokens
`--surface-raised`, `--surface-menu`, `--surface-toast`, `--window-bg` e cores
semânticas próprias do tema claro (`--red: #c4281f`, `--yellow: #7a5a00`...).
Resultado: **0 textos abaixo de 3.0** em lista, painel de ações (`Ctrl+K`), menu
de tipo e as 3 abas das Preferências, nos 5 temas.

Armadilha do próprio auditor: as transições de cor de 130 ms fazem o contraste
ser medido no meio da animação (17 "falhas" falsas); o script injeta
`*{transition:none!important;animation:none!important}` antes de medir.

#### (c) Botões de minimizar/maximizar/fechar no PC do amigo

`decorations: false` não basta em todo Windows: a moldura pode voltar. A
tentativa com `SetWindowLong(GWL_STYLE)` (`WS_POPUP | WS_THICKFRAME`) resolveu a
moldura e **quebrou o teste de acerto do mouse** — `WindowFromPoint` passou a
devolver a barra de tarefas e o hook de clique passou a esconder a janela a cada
clique. A versão que ficou não mexe no estilo:
`DwmSetWindowAttribute(DWMWA_NCRENDERING_POLICY, DWMNCRP_DISABLED)`
(`strip_caption_sem_quebrar`, §8.49c), que desliga a moldura desenhada pelo DWM
sem alterar o `WS_*` que o Windows usa para decidir onde o clique cai.

#### Os erros do harness (de novo, e todos meus)

1. `GetSystemMetrics(76/77)` é a **origem** da área virtual (0,0), não o tamanho
   — o cálculo virava divisão por −1 e **todo** clique sintético ia para
   `(0, 1079)`, a barra de tarefas. Daí a impressão de "o app esconde quando
   clico" e o clique que caiu no Discord. O certo é 78/79 para tamanho e 76/77
   para origem, com `MOUSEEVENTF_VIRTUALDESK`.
2. `getBoundingClientRect` é da **página**; faltava somar a origem da janela.
3. `keysafe.focar` sem conferir `GetForegroundWindow` de novo: o console do
   agente roubava o foco entre a medição e o clique.
4. O `Popen` de uma segunda instância **reinicia** o app (instância única): as
   coordenadas medidas na página antiga deixavam de valer.

#### (d) Aviso de atalho em conflito

`apply_shortcuts` devolvia o erro cru do plugin
(`HotKey { mods: Modifiers(ALT), key: Space, id: 65598 }`) na UI. Agora devolve
códigos estáveis — `hotkey-conflict|<campo>|<tentado>|<mantido>` e
`hotkey-failed|<campo>|<tecla>` — e o atalho em conflito **volta** para o último
que funcionava (`ATALHOS_ATIVOS`), com tradução em EN/pt-BR.

### 8.47 Regressão: clicar numa linha não selecionava (v0.1.35)

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

### 8.48 O clique sempre funcionou: `SendInput` mudo é que enganava o teste

Depois do §8.47 a janela continuava "se escondendo" em todo clique sintético, e
o log dizia `click-hook: hide (pid=… class=Chrome_RenderWidgetHostHWND)` — o que
dava a entender que o app se fechava ao clicar dentro dele. A causa não estava
no app: **nenhum clique ou tecla era entregue**.

As `structs` de `INPUT`/`MOUSEINPUT`/`KEYBDINPUT` do harness eram escritas à mão
sem a **união** que existe no C. Resultado: `sizeof(INPUT)` = **32** em vez de
**40** bytes. `SendInput` valida o tamanho, recusa a chamada e devolve **0** — e
ninguém olhava o retorno. O app "se escondia" porque o foco ia para o console do
agente e o clique nunca acontecia.

**Conserto (`_ref/entrada.py`):** união explícita
(`_UNIAO = MOUSEINPUT | KEYBDINPUT | HARDWAREINPUT`), com
`assert sizeof(INPUT) == 40` e `assert sizeof(MOUSEINPUT) == 32`, e `_enviar()`
que **levanta erro** quando `SendInput != 1`. Expõe `clicar(x, y)` e `tecla(vk)`;
todo teste passa a usar isso em vez de structs inline.

**Prova com entrada de verdade (cliques e teclas do Windows, build debug):**

| Ação | Resultado |
|---|---|
| seta ↓ / ↓ / ↑ | seleção `e9c16c56` → `78d27958` → `39e66175` → `78d27958` |
| digitar `x` | campo de busca `''` → `'x'` |
| clique nas linhas 2, 4 e 6 | seleciona a linha clicada (CDP, `data-selected`) |

Ou seja: clique, Ctrl+clique, Shift+clique e setas funcionam — o defeito real era
só o do §8.47, e o v0.1.35 publicado já o corrige.

**Lição:** struct de `ctypes` para API C com união **tem** que declarar a união, e
toda chamada `SendInput` precisa conferir o retorno. Um teste que não falha alto
quando a entrada não é entregue produz diagnósticos falsos — e foi o que
aconteceu aqui por horas.

### 8.46 Mutações devolvem 26 bytes em vez de 1,8 MB (v0.1.34)

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

### 8.45 Auto-updater verificado ponta a ponta (0.0.9 -> 0.1.33)

A pergunta era "o auto-updater está funcionando?". Estava, mas nunca tinha sido
testado com uma atualização de verdade — só com testes de unidade das peças.

**Como testar sem estragar a instalação:** um build de debug declarando
**0.0.9** (bump temporário em `Cargo.toml`) + CDP ligado, rodando de
`src-tauri/target/debug/`. O swap acontece na pasta do próprio exe, então a
instalação em `AppData\Local\Rayclip` nem foi tocada.

```
invoke('update_check', {force: true})
invoke('update_install')
```

**Resultados:**

| Etapa | O que saiu |
|---|---|
| check | `{current: "0.0.9", version: "0.1.33", tag: "v0.1.33", size: 10871296, hasAsset: true}` |
| install | retornou `"0.1.33"`; log `update: v0.1.33 disponivel` |
| exe antes | 25.950.720 B (debug), sha `cf08df98e6f01bb5…` |
| exe depois | **10.871.296 B**, sha `3a4ad814c5e7550c…` — **igual ao asset publicado** |
| backup | `rayclip.exe.old` = o exe de debug, exatamente como o desenho prevê |
| processo novo | versão **0.1.33** rodando do mesmo caminho |
| abertura seguinte | `rayclip.exe.old` **apagado** (a regra "sobrevive à primeira execução") |

O sha256 confere porque o updater é *fail-closed*: sem checksum ou com hash
diferente ele não instala (testes `download_sem_checksum_e_recusado`,
`download_de_outro_host_e_recusado`, `versao_maior_e_mais_nova`).

**Depois do teste:** `Cargo.toml` e `tauri.conf.json` restaurados (`git diff`
vazio, `grep -c remote-debugging` = 0) e o build de debug refeito com log.

**Observação:** o caminho "já estou na última versão" não baixa nada — é o
estado normal da máquina (0.1.33 instalada = 0.1.33 publicada).

### 8.44 Busca e filtro no Rust: 1,8 MB por atualização -> 158 KB (v0.1.33)

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

### 8.43 Renderer do WebView2 morto: recarrega e, no pior caso, reinicia (v0.1.32)

Última pendência da lista (item F). Se o processo do renderer do WebView2 morre
(crash, driver de vídeo, memória), a janela fica **em branco para sempre** — o
Rust continua vivo, o atalho abre, e não tem interface. Raycast 2.8 recarrega
nesse caso.

**Como se detecta** (não existe evento de morte na API do Tauri 2):

- a página manda `heartbeat` a cada 3 s (`App.tsx` → `api.heartbeat()`);
- `ULTIMO_BATIMENTO` guarda o instante da última batida;
- uma thread no `setup` acorda a cada 5 s e só age se a janela está **na tela**
  (`SHOWN`), o app tem mais de **20 s** de vida e a última batida tem mais de
  **15 s**.

Usar `SHOWN` de propósito: com a janela na bandeja o WebView2 estrangula timers
e uma batida atrasada não significa renderer morto.

**Escada de recuperação:**

1. `reload()` (até 2 tentativas, 15 s de folga entre elas);
2. se nem o reload trouxer sinal, o **ambiente inteiro** do WebView2 morreu —
   `reload()` não recria o processo do navegador. Aí o app sobe uma cópia de si
   mesmo com `--open` e faz `exit(0)`. Uma vez só, senão vira laço.

**Medido (build debug, com log):**

| Cenário | Log | Resultado |
|---|---|---|
| só o renderer morto | `renderer morto: sem heartbeat ha 16758 ms -> reload (1)` e **268 ms depois** `renderer voltou a responder heartbeat` | janela igual a antes (media 20.8, desvio 34.1) |
| ambiente WebView2 inteiro morto | `reload (1)` 01:34:03, `reload (2)` 01:34:18, `renderer nao voltou apos 2 reloads (15002 ms) -> reiniciando o app` 01:34:33 | pid novo, janela volta com a interface |

No build de release (sem log) o mesmo teste de matar o renderer deu **mesmo pid**
e a janela de volta ao normal — a recuperação não depende do log.

#### `_ref/shot_px.py` — medir “branco” por pixel

`PrintWindow` com `PW_RENDERFULLCONTENT` + média/desvio do cinza:

| Estado | media | desvio |
|---|---|---|
| interface de verdade | ~20.8 | ~34 |
| janela morta / em branco | **0.0** | **0.0** |

Isso provou duas coisas que o olho não provaria: a janela realmente ficou
uniforme quando matei o renderer, e voltou idêntica depois do reload.

#### Duas pegadinhas

- **`cargo build` não embute o frontend.** O primeiro teste acusou falso
  positivo (`renderer morto: sem heartbeat ha 20001 ms`) porque o exe de debug
  servia o bundle **antigo**, sem o `heartbeat`. Para build de debug com log e
  frontend novo: `npx tauri build --debug --no-bundle`.
- Logo depois do restart automático a primeira captura deu `media 0.0` — era o
  WebView2 ainda subindo, não falha. Medindo 5 s depois: 20.8.

**Release 0.1.32:** `Rayclip_0.1.32_x64-setup.exe` (3.56 MiB) instalado; CDP fora
(9222 recusada); inicia na bandeja; `cargo test --lib` 36/36. Publicada no GitHub
com `rayclip.exe` + `rayclip.exe.sha256` + o instalador (é o par que o auto-updater
consome).

**Arquivos:** `src-tauri/src/lib.rs` (`heartbeat`, vigia, `TENTATIVAS_RELOAD`),
`src/api.ts`, `src/App.tsx`, `_ref/shot_px.py`.

### 8.42 Os "14 ms" eram do harness: medindo do key down, 2 ms x 22–24 ms

A medida de abertura estava inflada **para os dois apps** pelo próprio harness:
`send_keys` dorme 12 ms entre apertar e soltar a tecla, e o cronômetro começava
antes desse respiro. O atalho global dispara no **key down**, então esses 12 ms
não pertencem a ninguém.

Agora existem `send_down` (devolve o instante do `SendInput`) e `send_up`:

```python
t0 = send_down(*cfg["hotkey"])   # mede daqui
...                              # espera a janela ficar visível (poll 0,2 ms)
send_up(*cfg["hotkey"])
```

| App | antes (com o respiro) | agora (do key down) |
|---|---|---|
| Rayclip | 14 ms | **2 ms** (2,2,2,2,2 — repetido em duas rodadas) |
| Raycast | 21–23 ms | 22–24 ms |

A conclusão anterior ("Rayclip ~10 ms mais rápido") estava subestimada: partindo
do repouso na bandeja, a janela aparece em **2 ms**, contra 22–24 ms do Raycast —
uma ordem de grandeza.

#### A tentativa de otimizar o app: medida e descartada

Hipótese: `win.show()` do Tauri despacha para a thread principal e o
`ShowWindow` direto seria mais rápido (é o truque que o `hide_fast` já usa).
Implementei `RAYCLIP_SHOW_LENTO` como chave de comparação e rodei os dois no
mesmo build, 5 rodadas cada:

| Caminho | mediana |
|---|---|
| `win.show()` (original) | **2 ms** (7,2,2,2,2) |
| `ShowWindow` + `force_foreground` direto | **2 ms** (6,2,2,2,2) |

Empate — e o mesmo vale para tirar o `set_always_on_top(true)` da abertura. As
duas “otimizações” foram **revertidas** (mesma lição do §8.40): sem ganho medido,
não fica. O `show_now` e a flag de A/B saíram do código.

Os dois picos de 121/127 ms vistos na primeira rodada não reapareceram em
nenhuma das quatro rodadas seguintes; parecem ser do aquecimento do WebView2
logo depois de subir, não da abertura.

**Sem release:** só o harness (`_ref/harness_compare.py`) e documentação
mudaram; a v0.1.31 instalada é a mesma e já mede 2 ms.

### 8.41 Iniciar na bandeja e `--open` para abrir na tela (v0.1.31)

Antes, `setup()` **sempre** fazia `win.show()` + `set_focus()`: quem abre o app no
login via Tarefa Agendada levava a janela na cara, no meio do trabalho.

Agora:

- sem argumento → nasce **na bandeja** (`inicio: janela na bandeja (use --open para
  abrir na tela)` no log);
- com `--open` → mostra a janela já no início, pelo mesmo caminho do atalho
  (`win.show()` + `set_focus()` + `set_shown(true)`, senão o estado de
  visibilidade ficaria mentindo para o vigia de foco).

A flag é lida em `run()` (para o log) e **de novo dentro da closure do `setup`**:
capturar o `bool` de fora obrigava a closure a ser `move`, e ela sobrevive à
função — a primeira tentativa nem compilava (`E0373 closure may outlive`).

**Medido, na instância instalada (0.1.31):**

| Como inicia | Janela visível depois de 6 s |
|---|---|
| sem argumento | **não** (`find_window` = None) |
| `--open` | sim (título "Rayclip") |
| sem argumento + `Alt+Space` | sim |
| sem argumento + abrir o exe de novo | sim (segunda instância manda a primeira mostrar) |

`_ref/reiniciar.ps1` ganhou `-Hidden`; sem ele passa `--open` (é o que os testes
em `_ref/` precisam para achar a janela). Detalhe que custou uma rodada: com
**duas instâncias** vivas o `Alt+Space` não abre nada — a instância que ficou
com o registro do atalho não era a que respondia. Matando todas e subindo uma, o
atalho volta a funcionar.

**Release 0.1.31:** `Rayclip_0.1.31_x64-setup.exe` (3.56 MiB) instalado; CDP
fora do build (9222 recusada); o app está rodando **na bandeja**, como você pediu.

**Arquivos:** `src-tauri/src/lib.rs` (`abrir_no_inicio`), `_ref/reiniciar.ps1`
(`-Hidden`).

### 8.40 Colagem 3×5, "Compactar banco" e OCR em prioridade baixa (v0.1.30)

#### Colagem: a diferença de 2 ms se mantém (`_ref/colagem_3x.py`)

Três rodadas de 5 por app, 15 amostras cada:

| App | mediana | min–max |
|---|---|---|
| Rayclip | 20,1 ms | 18–42 |
| Raycast | **17,7 ms** | 17–21 |

O Raycast é ~2,4 ms mais rápido **de forma consistente** (as três rodadas deram
22,4/20,0/20,1 contra 17,8/17,7/17,9). Não dá para chamar de empate, mas 2,4 ms
são um sétimo de um quadro a 60 Hz — invisível. O caminho já é o mais curto
possível: `copy_entry` → `emit` → `hide_fast` → thread → `SendInput` atômico com
0 ms de espera e sondagem de foco de 1 ms. O log confirma `espera=0ms` na
maioria das colagens, então não há sono fixo a remover.

#### `ipc::Response` para a lista: **medido e descartado**

Hipótese: devolver a lista como JSON já serializado (bytes) evitaria uma segunda
serialização no Tauri. Medido no debug, 5 rodadas de cada, mesma janela, 1,8 MB:

| Comando | mediana |
|---|---|
| `list_entries` (Vec<Entry>) | 87,3 ms (84–87,4) |
| `list_entries_raw` (`ipc::Response` + `JSON.parse`) | 86,9 ms (85,3–87,1) |

**Empate.** O Tauri 2 já manda a string JSON pronta pelo `Channel` (não há
re-serialização para eliminar), então a mudança só acrescentaria superfície.
Revertida — o comando e a função voltaram a ser o `Vec<Entry>` de antes. Fica
registrado: **não repetir essa “otimização”**. Os 87 ms são dominados pelo
tamanho do que trafega (1,8 MB) e pelo `JSON.parse` na página; o ganho real
estaria em mandar menos bytes ou filtrar no Rust, não no formato.

#### "Compactar banco" (VACUUM) no Settings

Novo comando `compact_db` e campo em **Data & Security** (EN/pt-BR, com estado
"Compactando…" e aviso do resultado). Medido nesta máquina, 1680 entradas:

| | bytes |
|---|---|
| antes | 455.913.472 (434,8 MB) |
| depois | 444.649.472 (424,1 MB) |

**−11,3 MB (−2,5%)**, e a lista continuou carregando normalmente depois
(1680 entradas). O texto da tela deixa claro que isso devolve **espaço**, não
prova que os bytes antigos saíram do SSD.

#### OCR em prioridade baixa

O backfill (`run_ocr_for`) chama `win32::thread_below_normal()`
(`SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)`) antes de indexar: indexar
imagem é trabalho de fundo e não pode competir com a janela que o usuário acabou
de abrir. Em máquina com um núcleo livre o efeito é nulo; com a CPU cheia, quem
está na frente ganha a vez.

**Release 0.1.30:** `Rayclip_0.1.30_x64-setup.exe` (3.56 MiB) instalado e
reaberto; CDP fora do build (porta 9222 recusada na instância instalada);
histórico **1680** entradas; `cargo test --lib` **36/36**; i18n 329 chaves PT, 0
sem tradução.

**Arquivos:** `src-tauri/src/lib.rs` (`compact_db`, prioridade no OCR),
`src-tauri/src/store.rs` (`db_file`), `src-tauri/src/win32.rs`
(`thread_below_normal`), `src/api.ts`, `src/components/SettingsPage.tsx`,
`src/lib/i18n.ts`, `_ref/colagem_3x.py`.

### 8.39 Harness completo numa rodada (e o que impedia o Raycast de ser medido)

O `_ref/harness_compare.py all` agora roda **as três seções numa tacada só**, sem
abortar o console e sem medir a coisa errada:

| Medida | Rayclip | Raycast |
|---|---|---|
| Atalho → janela visível | **14 ms** (14–14) | 21 ms (14–22) |
| Enter → texto chega no alvo | 21 ms (20–25), 5/5 | **19 ms** (17–22), 5/5 |
| Cópia → item no histórico | ok em 30 / 80 / 150 / 300 ms | ok em 30 / 80 / 150 / 300 ms |

Abertura ~7 ms na nossa frente, colagem empatada (a diferença de 2 ms anda de
lado entre rodadas: já deu 23 x 23). O que valia mesmo era o harness voltar a
medir os dois de ponta a ponta.

**Três defeitos do harness, todos do mesmo tipo — tecla indo para o lugar errado:**

1. `send_keys` mandava a combinação para **a janela em foco**, qualquer que
   fosse. Sem trava, um `Esc` podia cair no console do agente e cancelar a
   operação. Agora existe `foco_seguro()`: só envia se o foco for
   `rayclip.exe`, `raycast.exe`, `pythonw.exe`/`python.exe` (o helper). Se não
   for, o foco vai antes para a **janela neutra** do harness (`ALVO_HWND` +
   `keysafe.focar`), e se ainda assim não der, a tecla **não** sai e fica
   registrada no log.
2. `Alvo.hwnd` procurava a janela do helper só como `python.exe`, mas a Tarefa
   Agendada sobe `pythonw.exe` — devolvia `None`, e a “focalização” não
   acontecia. Agora tenta as duas formas, primeiro pela classe
   `RayclipHarnessAlvo`.
3. `close_app` mandava `Esc` com o foco na janela neutra, então **não fechava
   nada**: o Raycast já estava aberto e toda medida dele virava tempo esgotado
   (3.001 ms de abertura, 0/5 colagens). Agora foca **a janela do app** antes do
   `Esc`.

Junto: o harness fecha as duas janelas no fim (não deixa app aberto em cima do
trabalho) e as **72 entradas** de marcador (`hxXXXXXX`) que sobraram das rodadas
foram apagadas — histórico de volta a **1680**.

**Sem release:** só ferramenta de medição e documentação; a v0.1.29 instalada é
a mesma.

**Arquivos:** `_ref/harness_compare.py` (`foco_seguro`, `pid_em_foco`,
`ALVO_HWND`, `Alvo.hwnd`, `close_app`, limpeza final).

### 8.38 Latência de imagem, GIF animado e por que o console abortava a operação

Três resultados e um conserto de app (v0.1.29).

#### Latência de captura de imagem (`_ref/test_img.py`)

Um PNG novo por rodada (conteúdo idêntico é deduplicado pelos dois apps),
colado no formato que o navegador usa (**CF_DIB** montado à mão a partir de
`BITMAPINFOHEADER` + BGR de baixo para cima), 5 rodadas cada:

| App | Cópia → item no histórico (mediana) |
|---|---|
| **Rayclip** | **57 ms** (56–59) |
| Raycast | 223 ms (213–228) |

O Raycast sequer vê o DIB direto: ele grava `file-*.png` em
`%LOCALAPPDATA%\Raycast\clipboard` (1294 arquivos, 375 MB) e indexa depois. Nós
resolvemos a imagem em memória e gravamos uma vez no banco.

#### GIF animado no painel de detalhes

Método: dois prints da janela **400 ms** de distância, contando pixels com
diferença forte (>90 somando RGB) e a caixa onde eles estão — se aparecerem
milhares de pixels numa caixa quadrada, é o GIF trocando de quadro; se aparecer
um risco fino no campo de busca, é só o cursor piscando.

| Caso | Rayclip | Raycast |
|---|---|---|
| GIF copiado do navegador (formato cru `GIF`) | **anima** (54.061 px, caixa 240×240) | não medido com seleção confiável |
| GIF copiado do Explorador (arquivo, `CF_HDROP`) | **anima** (55.557 px, caixa 240×240) — *depois do conserto* | **anima** (38.518 px, caixa 200×200) |

**O que estava errado:** GIF copiado como **arquivo** entrava pelo caminho de
"lista de arquivos", virava `kind: image` com `gif: false` e a prévia mostrava
um quadro estático (o thumbnail do Windows). O Raycast anima nesse caso.

**Conserto** (`capture.rs`): `gif_de_arquivo()` reconhece `*.gif` de até 20 MB
que comece com `GIF8` e chama `monta_gif()`, o mesmo construtor do GIF que vem
pelo clipboard — bytes crus no BLOB, poster do 1º quadro para a lista (GIF fora
da seleção **não** anima), `hash` do conteúdo (o mesmo GIF vindo do navegador e
do Explorador continua sendo **uma** entrada) e `paths` preservado, então
"Revelar na pasta" segue funcionando. O antigo braço `Captured::Gif` passou a
usar o mesmo construtor, sem duplicação.

#### Por que o console abortava a operação (causa, não sintoma)

`SendInput` manda a tecla para a **janela em foco**, seja ela qual for. Vários
roteiros de teste mandavam `Esc`, `Alt+Espaço`, setas e texto **sem conferir o
foco**: quando o app de destino não abria (ou já estava fechado), as teclas
caíam no console que hospeda o agente — e `Esc`/`Alt+Espaço` ali cancelam a
operação em curso. Era isso o "Command aborted"/"This operation was aborted"
que parecia aleatório. Efeito colateral visível: o Raycast guardou no histórico
dele o texto do próprio console ("Error: This operation was aborted…"), o que
confirmou o diagnóstico.

**Regra nova, em código (`_ref/keysafe.py`):**

- `focar(hwnd)` usa `AttachThreadInput` + `SetForegroundWindow` + `SetFocus` e
  **confirma** que o foco ficou na janela (o Windows bloqueia troca de foco
  "roubada" quando o processo chamador não é o dono do foco).
- `teclas_seguro(pid, …)` e `texto_seguro(pid, …)` só enviam se
  `GetForegroundWindow` pertencer ao processo esperado; senão levantam erro e
  **nada** é enviado.
- Atalho global (o `Alt+Espaço`/`Ctrl+Shift+V` do Rayclip/Raycast) é enviado com
  o foco numa **janela neutra do harness** (`RayclipHarnessAlvo`): ali um
  `Ctrl+Shift+V` é um colar inofensivo e nunca chega no console.
- Scripts que só precisam clicar usam CDP (`Input.dispatchMouseEvent`) — zero
  teclado.

Duas armadilhas menores do mesmo dia: `websocket.create_connection` precisa de
`suppress_origin=True` neste WebView2 (senão o handshake é recusado com "Origin
header is not a valid URL") e `cargo build --release` **não** embute o frontend —
sobra `chrome-error://chromewebdata/` e todo `invoke` falha; para medir é preciso
`npx tauri build --no-bundle`.

**Release 0.1.29:** instalador `Rayclip_0.1.29_x64-setup.exe` (3.56 MiB),
instalado e reaberto; CDP **fora** do build final (`grep -c remote-debugging` =
0 e a porta 9222 responde "recusado" na instância instalada). Histórico de volta
a **1680 entradas**, sem sobras de teste na tela; `cargo test --lib` **36/36**.

**Arquivos:** `src-tauri/src/capture.rs` (`gif_de_arquivo`, `monta_gif`),
`_ref/keysafe.py`, `_ref/test_gif4.py`, `_ref/test_gif5.py`,
`_ref/test_gif_file.py`, `_ref/test_img.py`, `_ref/test_gif2.py` (marca
aleatória por rodada, para o dedup não mascarar a medida).

### 8.37 Velocidade e ordem da busca: Rayclip x Raycast medidos no mesmo Windows

Duas medições que faltavam, agora com número.

#### Velocidade (`_ref/harness_compare.py`, mediana de 5 rodadas)

| Medida | Rayclip | Raycast |
|---|---|---|
| Atalho → janela visível | **15 ms** (14–17) | 25 ms (22–26) |
| Enter → texto chega no alvo | **23 ms** (21–29) | **23 ms** (21–25) |
| Cópia → item já no histórico | ok em 30 / 80 / 150 / 300 ms | ok em 30 / 80 / 150 / 300 ms |

Colagem empatada, captura empatada, abertura ~10 ms mais rápida (o Raycast é
WPF, nós somos WebView2 já quente).

**Para o harness voltar a medir colagem foi preciso consertar duas coisas:**

1. `_ref/alvo_helper.py` quebrava sempre que rodava em `pythonw.exe`:
   `sys.stderr` é `None` ali e o `sys.stderr.write(...)` do "alvo pronto"
   derrubava o processo antes de qualquer medida. Por isso as seções `[paste]` e
   `[capture]` nunca tinham número — só `[open]` aparecia.
2. `schtasks /RL LIMITED` (o caminho antigo para a janela alvo em integridade
   média) falha nesta máquina com "último resultado: 1". Substituto:
   `_ref/alvo_medium.ps1`, que duplica o token do `explorer.exe` e cria o
   processo com `CreateProcessWithTokenW` (integridade média de verdade, sem UAC).
   Dois tropeços no caminho, ambos do Windows/PowerShell: `CreateProcessWithTokenW`
   mora em **advapi32**, não em user32; e `\"` não é escape em PowerShell
   (o nome da aplicação chegava com barras invertidas → erro 123).

#### Ordem da busca (`_ref/test_ranking.py`)

Cinco textos com a mesma palavra rara em posições diferentes, copiados do **menos**
relevante para o mais relevante — assim relevância e recência apontam para lados
opostos e dá para ver qual regra cada app usa. Duas rodadas, com a ordem de cópia
invertida na segunda.

Rodada 1 (copiado: exato → prefixo → começo de palavra → dentro de palavra → meio):

| # | Texto | Rayclip | Raycast |
|---|---|---|---|
| 1 | `qk4562` (exato) | **1º** | 5º |
| 2 | `qk4562 beta` (prefixo) | **2º** | 4º |
| 3 | `beta qk4562` (começo de palavra) | **4º** | 3º |
| 4 | `beta qk4562bum` (dentro de palavra) | **3º** | 2º |
| 5 | `xqk4562x` (meio, copiado por último) | 5º | **1º** |

Rodada 2 (ordem de cópia invertida: o exato foi o último):

| # | Texto | Rayclip | Raycast |
|---|---|---|---|
| 1 | `qk5780` (exato) | **1º** | **1º** |
| 2 | `qk5780 beta` | **2º** | 2º |
| 3 | `beta qk5780` | **3º** | 3º |
| 4 | `beta qk5780bum` | **4º** | 4º |
| 5 | `xqk5780x` | 5º | 5º |

**Conclusão:** o Raycast para Windows **ordena só por recência** — a busca filtra,
não classifica (na rodada 1 o "exato" ficou em último; na rodada 2, em primeiro,
sempre acompanhando a data). O Rayclip classifica: exato → prefixo → começo de
palavra → meio, com recência só como desempate (na rodada 1, `beta qk4562bum`
veio antes de `beta qk4562` por ser mais novo, ambos no mesmo degrau).

É uma divergência **mantida de propósito**: quem digita a string inteira espera
ela em primeiro. Fica registrado para não ser "corrigido" por engano no futuro.

**Sem release:** nada do app mudou, só ferramentas de medição e documentação.
Vale para a v0.1.28 instalada.

**Arquivos:** `_ref/alvo_medium.ps1` e `_ref/alvo_run.py` (novos),
`_ref/alvo_helper.py` (conserto do `stderr`), `_ref/test_ranking.py` (novo),
`_ref/test_pm*.py` (o `sys.stdout` saiu do nível do módulo, para importar sem
derrubar o stdout de quem importa).

### 8.36 Comparação com o Raycast no mesmo Windows (senhas)

O mesmo simulador (`_ref/fake_pm.rs`) rodou contra o Raycast instalado
(`Raycast.exe`, 2.13.1.0), com o histórico de clipboard dele aberto por
`Ctrl+Shift+V`. Como o banco do Raycast é SQLCipher sem chave acessível, a
leitura foi **pela interface**: cada cópia recebe um sufixo único com prefixo
comum (`rcsenhaXXXX-`), uma única busca pelo prefixo mostra exatamente quais
cópias foram guardadas.

Detalhe do método: o print é tirado **dentro do processo Python**
(`PIL.ImageGrab` sobre o retângulo da maior janela visível), porque lançar um
`powershell` depois do atalho roubava o foco e o Raycast escondia a janela.

**Conjunto 1 — seis cópias**

| Cópia | Marcador | Raycast guardou? |
|---|---|---|
| `Bitwarden.exe`, texto puro | — | **sim** |
| `bloco_de_notas.exe`, texto puro | — | sim |
| `bloco_de_notas.exe` | `ExcludeClipboardContentFromMonitorProcessing` | **não** |
| `bloco_de_notas.exe` | `CanIncludeInClipboardHistory = 0` | **não** |
| `bloco_de_notas.exe` | `CanIncludeInClipboardHistory = 1` | sim |
| `1Password.exe`, texto puro | — | **sim** |

**Conjunto 2 — o caso do mundo real (gerenciador que publica o marcador)**

| Cópia | Marcador | Raycast guardou? |
|---|---|---|
| `Bitwarden.exe` | `ExcludeClipboardContentFromMonitorProcessing` | **não** |
| `Bitwarden.exe` | `CanIncludeInClipboardHistory = 0` | **não** |
| `Bitwarden.exe`, sem janela | — | sim |
| `1Password.exe` | `ExcludeClipboardContentFromMonitorProcessing` | **não** |

**Comparação final**

| Cenário | Rayclip | Raycast (Windows) |
|---|---|---|
| Gerenciador de senha, texto puro | **não guarda** (nome do processo) | **guarda** |
| Qualquer app + `ExcludeClipboard…` | não guarda | não guarda |
| Qualquer app + `CanIncludeInClipboardHistory = 0` | não guarda | não guarda |
| `CanIncludeInClipboardHistory = 1` | guarda | guarda |
| App comum | guarda | guarda |
| Sem janela (dono desconhecido) + marcador | não guarda | não guarda |
| Sem janela, sem marcador | guarda | guarda |

**Conclusão:** nas duas camadas que o Windows oferece por *marcador*, o Rayclip
e o Raycast se comportam igual. Na camada de **nome do processo**, o Rayclip é
mais restritivo: o Raycast para Windows não olha quem copiou — se o gerenciador
de senha não publicar o marcador, o conteúdo entra no histórico dele. Isso não é
falha nossa; é uma decisão de privacidade mantida de propósito e visível em
Settings → Privacy ("Also capture passwords & protected content"), que desliga o
filtro quando o usuário quiser. Na prática os gerenciadores grandes publicam o
marcador, então o resultado é o mesmo — a diferença aparece só no caso ruim.

**Sem release:** não houve mudança de código, só medição e documentação. Vale
para a v0.1.28 já instalada.

**Arquivos:** `_ref/test_pm_raycast.py` (novo), `_ref/shot_win.ps1` (novo,
captura a maior janela visível de um processo WPF sem `MainWindowHandle`).

### 8.35 v0.1.28 — senhas: simulação de gerenciador de senha (o teste que faltava)

O filtro de conteúdo sigiloso nunca tinha sido exercitado de verdade: nesta
máquina não existe gerenciador de senha instalado. Foi criado um **simulador**
(`_ref/fake_pm.rs`, ~180 linhas, sem dependências, compilado com `rustc`) que
cria uma janela oculta — para ser o **dono** do clipboard, como um app real —,
escreve um texto e publica os marcadores que os apps usam.

**Como o filtro funciona (para referência)**

1. **Marcadores nativos** (`capture::is_sensitive`): `ExcludeClipboardContentFromMonitorProcessing`,
   `Clipboard Viewer Ignore` (basta o formato existir) e
   `CanIncludeInClipboardHistory = 0` (DWORD lido do clipboard).
2. **Nome do processo dono** (`win32::clipboard_owner_process` → `GetClipboardOwner`
   → `QueryFullProcessImageNameW`), comparado por substring com `IGNORED_APPS`.
3. `capture_sensitive` (Settings → Privacy, desligado por padrão) desliga **os dois**.

**Resultado da simulação (build debug, com inspeção por CDP)**

| Executável | O que fez | Entrou no histórico? | Esperado |
|---|---|---|---|
| `Bitwarden.exe` | texto puro | **não** | não |
| `1Password.exe` | texto puro | **não** | não |
| `KeePassXC.exe` | texto puro | **não** | não |
| `NordPass.exe` | texto puro | **não** | não |
| `WindowsPasswordManager.exe` | texto puro | **não** | não |
| `MeuPasswordVault.exe` | texto puro | **não** | não |
| `bloco_de_notas.exe` | texto puro | sim | sim |
| `bloco_de_notas.exe` | + `ExcludeClipboardContentFromMonitorProcessing` | **não** | não |
| `bloco_de_notas.exe` | + `Clipboard Viewer Ignore` | **não** | não |
| `bloco_de_notas.exe` | + `CanIncludeInClipboardHistory = 0` | **não** | não |
| `bloco_de_notas.exe` | + `CanIncludeInClipboardHistory = 1` | sim | sim |
| `Bitwarden.exe` | texto, com `capture_sensitive` ligado | sim | sim |

13 de 13 cenários certos. Antes de mexer no código, o teste mostrou dois
"fracassos" que eram **do simulador**: `SetClipboardData(fmt, NULL)` sozinho é
"rendering atrasado" e falha, então o formato nunca ficava disponível — do jeito
que apps reais fazem (um DWORD 1), os dois marcadores passaram a ser respeitados
sem nenhuma mudança no Rayclip. Vale registrar: era o harness errado, não o app.

**O que a simulação revelou e foi ajustado**

`IGNORED_APPS` ganhou dois nomes genéricos (`"password"` e `"vaultwarden"`), que
pegam `WindowsPasswordManager.exe`, `MeuPasswordVault.exe` e afins sem pegar app
comum. A comparação saiu para `capture::is_ignored_app(&str)` (testável) com o
teste `reconhece_gerenciador_de_senha`: 10 nomes de gerenciador reconhecidos,
`notepad.exe`, `chrome.exe`, `Code.exe`, `explorer.exe`, `Discord.exe`,
`WindowsTerminal.exe` **não**.

**Limite real, medido (não teórico)**

Quando o processo copia **sem janela nenhuma** (`OpenClipboard(NULL)` sem HWND),
o `GetClipboardOwner` devolve `NULL` e o Windows **não informa** quem escreveu —
logo o nome não pode ser checado. Medido com `nowin_text`:

| Executável | Modo | Entrou? |
|---|---|---|
| `Bitwarden.exe` | sem janela, texto puro | **sim** (não há como saber) |
| `Bitwarden.exe` | sem janela + `ExcludeClipboard…` | **não** |
| `bloco_de_notas.exe` | sem janela, texto puro | sim |

Ou seja: a proteção por marcador vale sempre; a proteção por nome exige um dono
identificável. Na prática os dois grandes (1Password e Bitwarden) publicam o
marcador **e** têm janela, então caem nas duas camadas.

**Verificação**

| Verificação | Resultado |
|---|---|
| `cargo test --lib` | **36 passam** (novo: `reconhece_gerenciador_de_senha`) |
| Simulação com 13 cenários | 13/13 como esperado |
| Entradas de teste | 14 apagadas depois; histórico de volta a 1.680 |
| Release | `Rayclip_0.1.28_x64-setup.exe` |

**Arquivos tocados:** `src-tauri/src/capture.rs` (`is_ignored_app`, lista,
teste), `_ref/fake_pm.rs` (novo simulador), `_ref/test_pm.py` e
`_ref/test_pm_sem_janela.py` (novos roteiros).

### 8.34 v0.1.27 — auditoria dos caminhos de colar, apagar e exportar

Segunda passada da revisão externa: `capture.rs`, `store.rs` e os comandos IPC de
colagem, exclusão e exportação. O que estava certo já estava certo; o que faltava
está aqui.

**O que foi verificado e já estava correto**

| Item | Situação |
|---|---|
| SQL em todo o `store`/`sqlite` | **nenhuma** query montada com `format!`: tudo por `?1`, `?2` (`db.run`/`db.query` com `&[Val]`). Sem injeção por título ou texto |
| `open_url` | já aceitava só `http`/`https`/`mailto` e usa `shell_open` (sem `cmd /C start`, que interpretava `&`, `\|`, `%VAR%`) |
| `reveal_path` | exige que o caminho **exista** antes de chamar o `explorer` e passa o argumento por `Command::arg` (sem shell) |
| `file_stem` (nome do arquivo salvo) | tira `/\:*?"<>|` e caracteres de controle, corta em 45 bytes **em limite de UTF-8** (não entra em panic), remove `.`/`-`/espaço nas pontas (então `..` vira vazio) e escapa nomes reservados do Win32 (`CON`, `NUL`, `COM1`…). Sem traversal |
| `write_entry_as` | só grava no caminho que o **diálogo do Windows** devolveu, ou no `unique_path()` (nunca sobrescreve: `nome 2.ext`) |
| `paste_many` | guarda **8** sequências em `ignore_sequences`, então as cópias intermediárias da colagem múltipla não voltam como entradas novas; a colagem é serializada por `lock_paste()` |
| Log (`%TEMP%\rayclip.log`) | não registra **conteúdo**, só ids, contagens, caminhos salvos e URLs |
| Interface | nenhum `dangerouslySetInnerHTML` / `innerHTML`: todo texto do clipboard é renderizado como texto pelo React |

**O que foi corrigido agora**

1. **`save_image_to` e `save_many_to` são comandos de teste** — escrevem em
   qualquer caminho recebido, sem diálogo. Em **release** agora recusam
   (`"… is a development-only command"`); só a build de desenvolvimento aceita.
   É defesa em profundidade: a interface nunca os chamou.
2. **`write_items` exige que a pasta de destino exista e seja pasta** (antes um
   `dir` inválido criava parte do caminho ou falhava item a item).
3. **`curl_get` não grava mais arquivo temporário.** O plano B baixava a página
   em `%TEMP%\rayclip-fetch-<pid>-<n>.out` — a página copiada (com o título e o
   texto dela) ficava em claro no disco. Agora o corpo vem pelo **stdout** e o
   `Content-Type` por uma marca final (`-w "\n__RAYCLIP_CT__%{content_type}"`),
   separada pela última ocorrência (o HTML pode conter a marca). Teste
   `separa_corpo_do_content_type` cobre exatamente esse caso.
4. **`cleanup_temp()` cobre os três restos**: `rayclip-ocr-*` (PNG do OCR),
   `rayclip-fetch-*.out` (página baixada) e `rayclip-thumb-*` (miniatura de
   arquivo). Um crash no meio de qualquer um deles não deixa mais lixo para
   sempre no `%TEMP%`.

**Verificação (build 0.1.27)**

| Verificação | Resultado |
|---|---|
| `cargo test --lib` | **35 passam** |
| `save_many_to` em pasta real | 3 arquivos gravados, nomes saneados (`C Users Lugia KitLugia.txt`), sem sobrescrever |
| `save_many_to` em pasta inexistente | `recusado: the destination folder does not exist` |
| `save_image_to` em entrada sem imagem | recusa limpa (e em release recusa por ser comando de desenvolvimento) |
| `%TEMP%` | **0** arquivos `rayclip-ocr-*`, `rayclip-fetch-*`, `rayclip-thumb-*` |
| Release | `Rayclip_0.1.27_x64-setup.exe` |

**O que continua sendo limite conhecido (e aceito):** "Salvar como…", "Copiar",
"Salvar N itens" e a colagem existem exatamente para **tirar** o conteúdo do
banco cifrado — por escolha do usuário, para um caminho escolhido no diálogo do
Windows. Não há como isso ser secreto: o dado sai por definição. O que a auditoria
garante é que ele só sai por ali, para onde o usuário apontou.

### 8.33 v0.1.26 — auditoria de segurança: falhar fechado

Uma revisão externa do código (feita depois que o projeto foi para o GitHub)
listou cinco pontos. Todos os quatro que dependiam de código foram corrigidos
aqui; o quinto (o app inteiro rodar elevado) ficou documentado como decisão
consciente, com o caminho de saída anotado.

| Achado | Antes | Depois |
|---|---|---|
| **1. Chave "de reserva"** (`key.rs`) | se `BCryptGenRandom` falhasse, a chave saía de tempo + endereço de memória (o próprio comentário admitia: não é CSPRNG) | `random_bytes()` devolve `Option`; sem CSPRNG **não existe chave** e o banco não abre. Uma chave previsível é pior que a ausência de histórico |
| **2. Chave em claro se o DPAPI falhasse** (`key.rs`) | `protect().unwrap_or_else(...)` gravava os 32 bytes crus em `rayclip.key` | sem `CryptProtectData` não se grava nada: `Err(motivo)`, banco fechado. Chave **antiga** crua (de build pré-0.1.26) é aceita e **re-protegida na hora** — não se joga fora o histórico de ninguém |
| **3. OCR gravava PNG em `%TEMP%`** (`ocr.rs`) | a imagem do clipboard ia em claro para `rayclip-ocr-<pid>-<ts>.png`, era lida e apagada (e ficava lá para sempre se o app morresse no meio) | `InMemoryRandomAccessStream` + `BitmapDecoder`: a imagem reduzida é codificada em PNG **na memória** e decodificada de lá. Nenhum arquivo. `cleanup_temp()` ficou só para limpar restos das builds antigas |
| **4. SSRF na prévia de links** (`linkmeta.rs`) | `ureq` seguia redirecionamentos sozinho e nada impedia `http://192.168.0.1/admin`, `http://localhost:8080` ou `http://169.254.169.254/` | `url_permitida()` + `ip_publico()`: só http/https, host precisa resolver **só** para IP público (loopback, 10/8, 172.16/12, 192.168/16, 169.254/16, CGNAT 100.64/10, `fc00::/7`, `fe80::/10`, multicast, 0/8, 240/4 fora), nome de máquina sem domínio barrado. `agent().redirects(0)` e `pedir()` seguem até 5 saltos **revalidando cada `Location`**. O plano B por `curl.exe` não segue redirecionamento (`--max-redirs 0 --proto =http,https`) |
| **5. App inteiro elevado** | `requireAdministrator` no manifesto de release | mantido — é o que permite colar em janelas elevadas (o Raycast resolve isso com um helper `UIAccess`). O caminho de saída, se um dia for necessário, é separar um serviço auxiliar elevado; está anotado no README |
| **6. CSP `null` e asset protocol `**`** (`tauri.conf.json`) | a interface podia carregar qualquer coisa e ler qualquer arquivo do disco | CSP fixa (`script-src 'self'`, `img-src` só o próprio app + `data:`/`blob:`, `object-src 'none'`, `frame-src 'none'`) e **asset protocol desligado**: o ícone do app de origem agora vem de `http://rayclip.localhost/src/<nome>` (nome validado, só a pasta `media/`, sem `..`). `connect-src` limitado ao IPC |

**Detalhes que valem registro**

* `Store` ganhou `key_error: Option<String>` e o app **continua abrindo** em modo
  somente-memória quando a chave não sai; `system_status` publica `keyError` e
  `Settings → Data & Security` mostra o motivo em vermelho. Antes o app abria
  vazio, sem explicar nada.
* O `id` do protocolo chega sem o `.png` (o handler tira o sufixo de todas as
  rotas): a rota `/src/` remonta o nome e só aceita `src-*`.
* A CSP foi verificada com o app rodando: **0 violações**, 11 imagens carregadas,
  nenhuma quebrada, e `/src/rayclip.db` → 404 (traversal barrado).
* O `secure_delete` continua ligado, mas a promessa no README foi corrigida:
  em SSD, com *wear leveling*, **nenhum software** garante apagar o setor físico.

**Verificação (build 0.1.26)**

| Verificação | Resultado |
|---|---|
| `cargo test --lib` | **34 passam** (inclui 4 de chave, 3 de SSRF, e os de store/settings/update) |
| Chave crua antiga → re-protegida | `chave_antiga_sem_dpapi_e_reprotegida`: arquivo deixa de ter os 32 bytes, DPAPI abre a nova, chave devolvida é a mesma (histórico preservado) |
| Chave ilegível (outro usuário) | `chave_ilegivel_nao_e_sobrescrita`: `Err`, arquivo intacto |
| SSRF | `bloqueia_destinos_internos` (27 URLs: loopback, privados, link-local, metadata da nuvem, IPv6 local, nome sem domínio, `user@host`, `file:`/`javascript:`) e `aceita_destinos_publicos` |
| Ao vivo: copiar `http://127.0.0.1:9222/json` | log `link: destino nao publico bloqueado (127.0.0.1:9222 salto 0)`; nenhuma requisição saiu |
| Ao vivo: copiar `https://github.com/luigiarrud4/Rayclip` | metadados normais (`site: GitHub` + ícone) — a guarda não bloqueia o caso comum |
| OCR em memória | 3 imagens reindexadas (`ocr: indexed …`), 1.249 entradas com texto preservadas, **0** arquivos `rayclip-ocr-*.png` em `%TEMP%` |
| CSP + `/src/` | 0 violações; `src-opera.png` 32×32 carrega; `rayclip.db` e `../rayclip.db` → falha |
| Release | `Rayclip_0.1.26_x64-setup.exe`, 3.729.413 B, SHA256 `4e4bb5d63f143c234fa91ffa1a857247b3edee1efbf942cc1f8a98505b668f9` |

**Arquivos tocados:** `src-tauri/src/key.rs` (reescrito), `src-tauri/src/ocr.rs`,
`src-tauri/src/linkmeta.rs`, `src-tauri/src/store.rs`, `src-tauri/src/lib.rs`
(`system_status` + rota `/src/`), `src-tauri/tauri.conf.json`, `src/lib/format.ts`,
`src/components/SettingsPage.tsx`, `src/styles.css`, `src/lib/i18n.ts`.

**O que continua valendo como limite conhecido:** enquanto o Rayclip está aberto,
qualquer processo **do mesmo usuário** pode ler o histórico pela memória ou pedir
as imagens ao `rayclip.localhost` — DPAPI protege contra *outro usuário* e contra
o arquivo copiado para outra máquina, não contra o mesmo login. Isso é inerente a
qualquer gerenciador de clipboard (o Raycast tem o mesmo limite) e está escrito no
README.

### 8.32 v0.1.24 — atualização automática (GitHub releases)

Pedido: *"seria bom um sistema de auto updater porque o rayclip vai para o
GitHub… use [o updater do robloxmanager] como base para pesquisa"*.

**O que foi olhado antes de decidir.** Três referências na máquina:

| projeto | como atualiza | o que aproveitamos |
|---|---|---|
| KitLugia (`deploy.bat`, `GitHubUpdater.cs`, `KitLugia.Updater`) | baixa um ZIP da release, confere o `.sha256` publicado, gera um `.cmd` que espera o app fechar e faz `xcopy` de tudo | a ideia do **deploy script** (`gh release create`) e o **fail-closed** do checksum (sem `.sha256`, não instala) |
| RobloxManager (`ram_core/src/update.rs`) | baixa o **exe**, confere SHA-256, troca os nomes no lugar, reabre passando o PID antigo | praticamente tudo do mecanismo: swap por rename, rollback, `--after-update <pid>`, `.old` sobrevivendo à primeira execução |
| Tauri (`tauri-plugin-updater`) | baixa e roda o instalador NSIS em silêncio | **não usado** (ver abaixo) |

**Por que não o plugin oficial do Tauri.** Ele exige um par de chaves de
assinatura (minisign) e um `latest.json` publicado, e instala rodando o
instalador NSIS — que recria atalhos, mexe no registro e custa ~3,6 MB por
atualização. O Rayclip é **um único exe** e já roda elevado; trocar o arquivo
tem menos partes móveis, não pede UAC (o processo novo herda a elevação) e não
toca em nada do sistema. O KitLugia usa ZIP + `.cmd` porque é um app .NET com
dezenas de arquivos; aqui isso seria complexidade sem função.

**Como funciona (`src-tauri/src/update.rs`).**

1. `check(force)` → `GET https://api.github.com/repos/<repo>/releases/latest`
   (o repo vem de `RAYCLIP_UPDATE_REPO` ou da constante `luigiarrud4/Rayclip`).
   Compara a versão por **número** (`0.1.100` > `0.1.99`); tag estranha
   (`tauri-v2.9.3`) nunca vira "atualização". Cache em memória de 6 h para não
   gastar o limite de 60 req/h da API anônima; o botão "Check now" ignora o
   cache.
2. `instalar()` — baixa o asset `rayclip.exe` para `rayclip.exe.update`
   calculando o SHA-256 **enquanto grava**, compara com o checksum da release
   (campo `digest` da API ou o asset `rayclip.exe.sha256`), lê o arquivo de
   volta do disco (é aí que o antivírus dá o veredito) e exige `MZ`. Sem
   checksum, **não instala**.
3. Swap: `rayclip.exe` → `rayclip.exe.old`, `rayclip.exe.update` → `rayclip.exe`
   (dois renames no mesmo volume; se o segundo falhar o primeiro é desfeito).
4. `relaunch()` sobe o novo com `--after-update <pid>` e observa 2 s; se ele
   morrer, o `.old` volta a ser `rayclip.exe` e a mensagem aparece na tela.
5. No `run()`, **antes** da checagem de instância única, o processo novo espera
   o PID antigo sair (até 30 s) — senão os dois abririam o SQLite ao mesmo
   tempo, que é justamente o que estraga o histórico. Sem o argumento, a
   abertura normal limpa sobras (`.update`, `.old`).

**Interface.** `Settings → Updates`: versão instalada, "Find updates
automatically" (padrão ligado; só avisa, nunca instala sozinho), "Check now",
cartão da release com notas e os botões **Install update** / **Release page**,
e uma barra de progresso (`update:progress`). Quando uma versão nova aparece na
verificação de fundo (15 s após abrir e a cada 6 h), o rodapé ganha uma pílula
discreta `Update 0.1.25` que abre essa seção. Em **Offline & private** nada é
consultado (o comando devolve erro e a UI explica).

**Publicar (`deploy.bat`).** Modelado no `deploy.bat` do KitLugia, mas em
versão única:

```bat
deploy.bat                 :: sobe o patch (0.1.24 -> 0.1.25) e publica
deploy.bat 0.2.0           :: publica essa versão
deploy.bat --build-only    :: compila e calcula o hash, sem publicar
```

Ele grava a versão nos três arquivos que precisam concordar (`package.json`,
`tauri.conf.json`, `Cargo.toml` — via `_ref/bump_version.ps1`, em UTF-8 **sem
BOM**, senão o serde do build recusa o `tauri.conf.json`), roda
`npx tauri build`, calcula o SHA-256 com `certutil`, comita, faz push e cria a
release no GitHub com **três** assets: `rayclip.exe` (o que o updater baixa),
`rayclip.exe.sha256` e `Rayclip_X.Y.Z_x64-setup.exe` (instalação manual).

**Verificação**

| passo | resultado |
|---|---|
| `cargo test --lib update` | **10/10** (versão numérica, versão igual/mais velha, tag inválida, checksum ausente, host não permitido, swap com o exe aberto, sobras, exe que não sobe → rollback, PID inexistente) |
| `update_check` ao vivo (`RAYCLIP_UPDATE_REPO=BurntSushi/ripgrep`) | `{"current":"0.1.24","version":"16.0.0", …}` com as notas da release lidas |
| tag fora do padrão (`tauri-apps/tauri`, tag `tauri-v2.9.3`) | devolve `null` sem erro (não inventa atualização) |
| release sem o asset | botão **Install update** desabilitado; o comando responde "release v16.0.0 has no rayclip.exe asset to download" |
| troca de verdade (duas cópias no `%TEMP%`): `rayclip.exe` → `.old`, novo no lugar, segundo processo com `--after-update <pid>`, antigo morto | o novo **continua vivo** (não morre como "instância duplicada") e o `.old` fica ao lado |
| abertura normal seguinte | `.old` apagado, só `rayclip.exe` na pasta |
| `deploy.bat --build-only` | build + `Rayclip_0.1.24_x64-setup.exe` + `rayclip.exe.sha256` com o hash correto |
| **atualização real, de ponta a ponta**: instalado pelo NSIS em `%LOCALAPPDATA%\Rayclip` (v0.1.24), publicado v0.1.25 com `deploy.bat 0.1.25`, "Check now" → "Install update" na UI | baixou, conferiu o SHA-256, trocou o exe, reabriu: `%LOCALAPPDATA%\Rayclip\rayclip.exe` ficou **v0.1.25** com o **mesmo SHA-256 do asset da release** (`26033c40…`), o antigo virou `rayclip.exe.old` (v0.1.24), sobrou **um** processo só |
| abertura normal seguinte (regra de reiniciar) | `.old` apagado; sobrou só `rayclip.exe` + `uninstall.exe` |
| UI em inglês e pt-BR | "Installed version / Check for updates / Install update / Release page" e "Versão instalada / Procurar atualizações / Instalar atualização / Página da release"; `check_i18n.py` → 321 chaves, 286 usos, 0 sem tradução |

**Repositório e releases.** O código está em
<https://github.com/luigiarrud4/Rayclip> (público — o updater lê a API do
GitHub sem token) e as releases em
<https://github.com/luigiarrud4/Rayclip/releases>. Para uma versão nova, basta:

```bat
deploy.bat
```

Se o repositório mudar de nome, troque a constante `REPO` no `deploy.bat` e
`REPO_PADRAO` em `src-tauri/src/update.rs` (ou compile com
`RAYCLIP_UPDATE_REPO` no ambiente).

**Pegadinha encontrada no caminho:** o `gh release create` com quebra de linha
por `^` no meio dos argumentos mandava um `tag_name` inválido (HTTP 422) — a
linha ficou inteira, sem continuação.

### 8.31 v0.1.23 — o que ainda faltava em relação ao Raycast (e como foi achado)

Pedido: *"falta mais alguma coisa? olhe novamente o Raycast e o nosso, se quiser
use o IDA Pro com o modo strings"*.

**Como foi investigado.** Não há IDA Pro nesta máquina, então o papel do "modo
strings" foi feito por `_ref/strings_dump.py` (extrai ASCII e UTF-16LE de um
binário, igual ao strings do IDA). O que rendeu de verdade, porém, foram os
arquivos do próprio Raycast:

| fonte | o que saiu |
|---|---|
| `Raycast.dll` (9,7 MB, extração de strings) | nomes de IPC (`IPCTypesClipboardDecodeQrCodeInput`, `HostClipboardEntry`), funções uniffi (`uiaccess_set_clipboard_listener_enabled`), preferências de organização (`organization_clipboard_history_retention_days`, `_disabled_applications`) e mensagens de log do backend (`Backend refused the clipboard entry over database contention, retrying in…`) |
| `frontend/main-window-*.js` (5,8 MB) | a **lista exata** de ações do histórico e o mapa de RPC do backend (`clipboard.deleteAllAfterDate`, `clipboard.movePinnedUp/Down`, `clipboard.updateContent`, `clipboard.getQrCodeFromImage`…) |
| `frontend/clipboard-history-filters-view-*.js` | o que são os "Filters": regras por **aplicativo de origem** que aplicam etiquetas automaticamente (`{sourceUrl, tags[], enabled}`) |

Lendo `id:`…`, title:`…`` em volta de `pasteAsPlainText`, apareceu a lista de
ações do Raycast. Comparando com o nosso painel, faltavam cinco coisas
(excluindo o que é de outro produto: Snippets, Quicklinks, AI Chat):

| Raycast | nós |
|---|---|
| `Paste`, `Paste as Plain Text`, `Paste and Keep Window Open` | já tínhamos |
| `Copy to Clipboard`, `Save as File`, `Reveal` | já tínhamos |
| `Pin/Unpin Entry`, **`Move Up/Down in Pinned`** | **faltava a reordenação** |
| `Rename Entry…`, **`Edit`** (`/edit-clipboard-entry-content`, campo "Content") | **faltava editar o conteúdo** |
| **`Copy Color as…`** (`color.getFormatted` → HEX/RGB/HSL…) | **faltava** |
| `Delete Entry`, **`Delete Recent Entries…`** (5/15/30 min, 1 h, 24 h) | **faltava** |
| `Copy Text from Image` (OCR) | já tínhamos |
| **`Copy Text from QR Code`** (`clipboard.getQrCodeFromImage`) | **não entrou** (precisaria de um decodificador de QR novo; ver §10) |
| grupos `Pinned…This Month`, **`This Year`**, **`Older`** | tínhamos `Earlier` no lugar dos dois últimos |
| `Filters` (etiqueta automática por app) | **não entrou** (é um recurso à parte, com tela própria de configuração) |

**O que foi implementado**

1. **Ordem dos fixados** (`Move Up/Down in Pinned`, `Ctrl+Shift+↑/↓`).
   Coluna nova `pin_order` (com `ALTER TABLE … DEFAULT 0` para bancos antigos),
   `Store::toggle_pin` coloca o recém-fixado no topo e `renumber_pins`
   reescreve 0..n-1; `move_pinned(id, up)` troca duas posições e grava só as
   linhas que mudaram. `entries_now` passou a ordenar **fixados por
   `pin_order`** e o resto por recência. Teste de unidade novo:
   `cargo test --lib store` → `fixados_entram_no_topo_e_reordenam` (passa).
2. **Editar conteúdo** (`Edit Content…`, `Ctrl+Shift+E`) — sem mexer no título,
   que continua sendo o `Ctrl+E`. O texto **inteiro** é carregado por
   `entry_text` antes de abrir (a lista só traz os primeiros KB; sem isso salvar
   cortava a entrada — o `EditContentDialog` sincroniza o campo **uma vez**,
   quando o texto chega). `update_entry_content` regrava texto, tamanho e o hash
   do conteúdo; o título só acompanha quando ainda era derivado do texto.
3. **Copiar cor como…** para entradas do tipo `color`, com HEX, HEX + alpha,
   RGB, RGBA, HSL e HSV. O parse e a conversão ficam em `src/lib/color.ts`
   (aceita `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()` e `hsl()/hsla()`), e a
   cópia usa um comando novo `copy_text` (texto puro, entra na lista de
   sequências ignoradas para não virar item do histórico).
4. **Apagar entradas recentes** (`Delete Recent Entries…`) — submenu com
   `Last 5 Minutes`, `15`, `30`, `Last Hour`, `Last 24 Hours`. O comando
   `delete_recent(minutes)` apaga **só o que não está fixado**, com a mesma
   confirmação do apagar em lote (foco começa em **Cancelar** e a lista mostra
   o que vai embora).
5. **Grupos de data iguais aos do Raycast**: `… This Month`, **`This Year`**,
   **`Older`** (`Earlier` saiu). Conferido com um teste isolado do
   `groupEntries()` (esbuild + node) porque a lista é virtualizada e o rótulo
   só existe na fronteira de cada grupo.

**Verificação (build debug + CDP, depois release)**

| passo | resultado |
|---|---|
| fixar duas entradas | a última fixada vira a primeira do grupo (`pin_order` 0 e 1) |
| `Move Up in Pinned` / `Move Down in Pinned` | ordem troca e é persistida; no topo, "subir" não faz nada |
| painel de uma entrada fixada | `Unpin Entry` · **`Move Up in Pinned`** · **`Move Down in Pinned`** |
| `Ctrl+Shift+E` numa entrada de texto | editor abre com **4.055 caracteres** (texto inteiro), dica "Esc to cancel · Ctrl+Enter to save", botões Cancel/Save |
| editar e salvar | `entry_text` devolve o texto novo, título acompanha, e restaurar volta ao original |
| `Copy Color as…` em `#59D499` | HEX `#59D499` · RGB `rgb(89, 212, 153)` · HSL `hsl(151, 59%, 59%)` · HSV `hsv(151, 58%, 83%)` |
| escolher RGB | clipboard recebe `rgb(89, 212, 153)` **e nenhum item novo entra no histórico** |
| `Delete Recent Entries…` | submenu 5/15/30 min, 1 h, 24 h; confirmação "Delete recent entries? … Pinned entries are kept." com "Delete 109", foco em **Cancel** |
| grupos de data | `Pinned, Today, Yesterday, This Week, This Month, This Year, Older` |
| pt-BR | "Subir nos fixados" / "Descer nos fixados" / "Editar conteúdo…" / "Copiar cor como…" (Voltar) / "Apagar entradas recentes…" → "Últimos 5 minutos … Últimas 24 horas" / "Apagar N" |
| `_ref/check_i18n.py` | 298 chaves, 264 usos, **0 sem tradução** |

Também caiu um detalhe antigo: a linha "Format" do painel de detalhes passava
`"PNG Image"`/`"GIF Image"` **sem `t()`** — agora traduz.

**Arquivos tocados:** `src-tauri/src/store.rs` (coluna + ordem + teste),
`src-tauri/src/lib.rs` (4 comandos novos, ordenação da lista),
`src-tauri/src/capture.rs` (`write_plain_text`), `src/components/EditContentDialog.tsx`
(novo), `src/lib/color.ts` (novo), `src/api.ts`, `src/App.tsx`,
`src/lib/format.ts`, `src/lib/icons.tsx`, `src/lib/i18n.ts`, `src/styles.css`,
`src/components/DetailsPane.tsx`.

### 8.30 v0.1.22 — idioma da interface (português do Brasil, inglês como padrão)

Pedido: *"coloque o idioma pt br também nas opções para o usuário poder trocar
mas deixe o inglês como padrão"*.

**Como foi feito.** A decisão de projeto foi usar **o texto em inglês como
chave**: `t("Paste")`, `t("No results for “{q}”", { q })`. Vantagens que
pesaram na escolha:

- o código continua legível (`t("Save As…")` diz o que é);
- texto sem tradução **cai no inglês** em vez de mostrar uma chave crua tipo
  `settings.paste.plain`;
- não existe lista paralela de identificadores para manter.

Um único lugar concentra tudo: `src/lib/i18n.ts` →

- `Lang = "en" | "pt-BR"`, `LANGUAGES` (usado no dropdown) e `localeOf()`
  (formatação de datas/horas);
- `makeT(lang)` devolve o tradutor; `makeT("en")` **não consulta tabela
  nenhuma** (o inglês é o texto original);
- `kindLabel()` traduz o tipo da entrada (Texto, Imagem, Link, E-mail…);
- a tabela `PT` (~270 chaves) com interpolação `{n}`, `{msg}`, `{q}`…

**Backend.** `Settings.language: String` com `serde(default)` = `"en"` e
sanitização (`LANGS = ["en", "pt-BR"]`): qualquer outro valor — inclusive
string vazia de um `settings.json` antigo — volta para `"en"`. Teste de unidade
`cargo test --lib settings` passa (4/4).

**Frontend.** `App` calcula `lang` a partir de `cfg.language` e passa `t` para
cada componente (NavBar, EntryList, DetailsPane, ActionBar, ActionPanel,
ConfirmDialog, SaveManyDialog, ImageViewer/ZoomableImage, SettingsPage).
`groupEntries()` e `displayTitle()` recebem `t` (rótulos Fixados/Hoje/Ontem e o
"Imagem (data)"). Os rótulos do painel de ações são montados já traduzidos em
`App`, com variáveis (`t("Paste {n} Items in Order", { n })`).

**Onde fica.** `Settings → Preferences → Language` (`English` | `Português
(Brasil)`), primeira linha da página, com descrição "Idioma de toda a
interface". Trocar ali salva na hora (mesmo caminho `saveSettings` do resto) e
a interface inteira muda sem reiniciar.

**Verificação (build debug + CDP):**

| passo | resultado |
|---|---|
| padrão de fábrica | `settings.json` sem `language` → abre em **inglês** ("Type to filter entries…", "All Types", "Pinned/Today") |
| troca pelo dropdown para Português (Brasil) | grava `"language": "pt-BR"` |
| Settings em pt-BR | nav = Preferências / Atalhos / Colagem / Histórico / Privacidade e indexação / Dados e segurança / Sistema / Sobre; linhas = Idioma, Offline e privado, Buscar prévias de link online, Prévia ao passar o mouse |
| janela principal em pt-BR | "Digite para filtrar as entradas…", "Todos os tipos", "Fixados"/"Hoje", "Informações" com Origem/Tipo/Caracteres/Palavras/Linhas, rodapé "Histórico do clipboard / Colar / Ações" |
| painel de ações | Colar · Colar como texto simples · Colar e manter a janela aberta · Configurações… · Copiar para o clipboard · Salvar como… · Renomear entrada · Desafixar entrada · Apagar entrada · Apagar tudo (manter fixados) |
| seleção múltipla | "Selecionados · 2 itens" + dica "Pressione Enter para colar nessa ordem, ou Esc para limpar a seleção…" |
| confirmação de exclusão | "Apagar 2 entradas?" / "Elas saem do histórico para sempre — inclusive as fixadas. Não dá para desfazer." / botões **Cancelar** (foco inicial) e **Apagar 2** |
| volta para English pelo dropdown | tudo volta ao inglês; `"language": "en"` |

**Cobertura conferida por script, não no olho:** `_ref/check_i18n.py` varre
todo `src/` procurando `t("…")` e compara com as chaves da tabela PT →
**236 usos, 0 sem tradução, 273 chaves**. É o teste que impede um texto novo
entrar em inglês por esquecimento.

Sem mudança de comportamento: nenhum atalho, fluxo de colagem, banco ou janela
foi tocado — a versão só acrescenta o campo `language` e a camada de textos.

### 8.29 Passo a passo das investigações (como cada coisa foi descoberta)

Registro dos "como descobri", na ordem em que aconteceu — para não refazer o
caminho na próxima vez.

**A. Links sem prévia (v0.1.18–0.1.19)**

1. Screenshot do usuário → linhas de link apareciam como URL crua. Primeira
   hipótese (errada): o rótulo. Mexi em `displayTitle` e o usuário corrigiu:
   *"não era sobre o nome, era só a visualização"*.
2. Contei o que existia: `list_entries` no CDP → **25 links, 10 sem
   `link.imagePath`**. Problema no **dado**, não no CSS.
3. Teste isolado em Rust (`#[cfg(test)] mod diag4`, `fetch()` direto nos dois
   primeiros casos): título e `og:image` funcionavam. Logo, o problema
   estava no **caminho do app** (fila/espera), não no buscador.
4. Rodei `link_pending` várias vezes e o número não se mexia → as entradas
   estavam com `tried:true` na versão atual e ficavam presas para sempre.
5. Daí a ideia de `fetch_v`: versão do buscador gravada por entrada. Subir a
   versão = uma tentativa nova para cada entrada incompleta, sem virar
   tráfego a cada abertura.
6. Ainda faltava algo: a fila só olhava **ícone**. Passei a contar "sem capa"
   como incompleto e acrescentei uma re-tentativa de capa por sessão
   (`capa_tentadas`, até 3, só quando a fila normal está vazia).
7. Placar final medido no CDP: **16 de 25** com capa; os 9 restantes receberam
   o bloco `.link-ph` (favicon + domínio). Nunca retângulo vazio.

**B. Fluxo do clipboard (v0.1.20)**

1. Li `capture.rs` inteiro primeiro (formatos, hash, senhas) e só depois
   `start_watcher`. O `sleep(320ms)` no laço chamou atenção.
2. **Teste da rajada**: dois textos escritos no clipboard com 100 ms de
   diferença → só o segundo entrava. Confirmou o diagnóstico sem mexer no
   código (o bug era o `seen = seq` pulando estados intermediários).
3. Troca para `AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE` numa janela
   *message-only*, com worker separado e `LAST_SEQ.swap`.
4. **Re-teste da rajada a 100/50/20/10 ms**: as duas cópias entram em todos os
   casos (antes: 100 ms já perdia a primeira).
5. Suspeita paralela confirmada no log: `atalhos registrados` sumia quando o
   Raycast estava aberto (`Ctrl+Shift+V` tomado). Daí o registro um a um.

**C. Harness de comparação (v0.1.20)** — quatro becos sem saída antes de
funcionar:

1. Janela alvo dentro do próprio harness (elevado) → Raycast colava, mas o
   texto nunca chegava: **UIPI**. Precisa de janela em integridade média.
2. `explorer.exe <bat>` para de-elevar → não funciona a partir de shell
   elevado. `runas /trustlevel:0x20000` → falha silenciosa.
3. Solução: **tarefa agendada `/RL LIMITED`** (média, sem UAC) + conversa por
   dois arquivos (`alvo_cmd.txt`/`alvo_state.txt`), com `perf_counter()`
   (QPC global) dos dois lados.
4. Ainda assim o Raycast não colava com `Enter`: as teclas "de comando"
   passavam, o `Enter` não. Causa: `KEYBDINPUT` com `wScan = 0`. Com
   `MapVirtualKeyW(vk, 0)` o Enter passou a funcionar nos dois apps.
5. Validação cruzada: clique na linha do Raycast colava, `Enter` não — foi o
   que apontou para a camada de teclado e não para o app.
6. Bônus descoberto no caminho: **`Delete` com o foco na busca não apaga no
   Raycast tampouco** — nossa regra de não apagar enquanto digita está
   correta.
7. Efeito colateral tratado: as medições deixam entradas no histórico. O teste
   apaga os marcadores (`/^(harness-|dbg-|hx[0-9a-f]{6}$)/`) via
   `delete_entries` sempre no final.

### 8.28 v0.1.21 — opção para guardar senhas e conteúdo protegido

Pedido: *"coloque uma opção no settings para poder copiar senhas que aí
desativa o filtro de não poder copiar certas coisas em guias anônimas ou
protegidas"*.

- **Settings → Privacy & Indexing → "Also capture passwords & protected
  content"** (desligado por padrão, como tem de ser).
- Ligado, o app **ignora os três avisos** que os apps usam para dizer "não
  registre isto":
  - `ExcludeClipboardContentFromMonitorProcessing`,
  - `Clipboard Viewer Ignore`,
  - `CanIncludeInClipboardHistory = 0`,

  e também a lista de processos de gerenciadores de senha
  (`ignore_password_managers`). É o que cobre gerenciador de senha e cópia
  vinda de aba anônima/protegida do navegador — os dois caminhos usam esses
  mesmos marcadores.
- Campo novo: `Settings.capture_sensitive` (`serde(default)`, então
  `settings.json` antigo continua abrindo — coberto por teste).
- `capture::read_current(ignore_password_managers, capture_sensitive)` —
  a decisão fica na função que lê o clipboard, não espalhada nos chamadores.

**Medido (build debug + CDP, `_ref/test_sigiloso.py`):**

| | resultado |
|---|---|
| texto com `ExcludeClipboardContentFromMonitorProcessing`, opção **off** | **não** guarda (0 entradas) |
| o mesmo, opção **on** | guarda (1 entrada, visível na lista) |
| ao voltar para off | volta a não guardar |

O toggle aparece alinhado na seção, desligado por padrão (conferido em
screenshot do painel).

### 8.27 v0.1.20 — harness de comparação com o Raycast (e o que ele mediu)

Pedido: "crie um harness para o Raycast também para você fazer uma comparação
direta". Está em **`_ref/harness_compare.py`** (+ `_ref/alvo_helper.py`).

O que ele mede — tudo com `SendInput` de verdade, cronometrado, na mesma
máquina:

| medida | como |
|---|---|
| **abertura** | atalho global → `IsWindowVisible` da janela do app (ms) |
| **colagem** | `Enter` → o texto aparece na janela alvo (`GetWindowTextW` de um EDIT nosso) |
| **captura** | cópia → o item já está no histórico (filtra a busca pelo marcador e cola) |

Cada app abre com o **próprio atalho**: Rayclip `Alt+Space`, Raycast
`Ctrl+Shift+V`. Nada de API interna — é o caminho do usuário.

**Resultado (5 repetições, mediana / máx):**

| | Rayclip 0.1.20 | Raycast 2.13.1 |
|---|---|---|
| abrir (atalho → visível) | **14 ms** (13–14) | **19 ms** (18–22) |
| colar (`Enter` → texto no alvo) | **17 ms** (16–23) | **14 ms** (13–15) |
| capturar a cópia | ≤ 30 ms | ≤ 30 ms |

Estamos no mesmo patamar, e a captura deixou de ser o ponto fraco: antes dela
uma cópia só entrava depois de até **320 ms** (o polling) e, se o usuário
copiasse duas coisas rápido, a primeira se perdia para sempre.

**Três armadilhas que o harness teve de resolver** (e que valem para qualquer
comparação futura):

1. **Janela alvo em integridade média.** O Raycast (integridade média) não
   consegue digitar numa janela de processo elevado (UIPI), e o harness precisa
   ser elevado para mexer no Rayclip de release. A janela alvo mora então num
   processo separado, criado por **tarefa agendada `/RL LIMITED`** (média sem
   UAC — o truque de lançar pelo Explorer não funciona a partir de shell
   elevado), e a conversa é por dois arquivos (`alvo_cmd.txt`/`alvo_state.txt`).
   Como `time.perf_counter()` é QueryPerformanceCounter no Windows, os dois
   processos medem na mesma linha do tempo.
2. **Scan code obrigatório.** Com `wScan = 0`, teclas "de comando" passavam
   (`Ctrl+Shift+V`, letras) mas o **Enter era ignorado** pelo WPF do Raycast.
   Com `MapVirtualKeyW(vk, 0)` no `KEYBDINPUT` o Enter funciona nos dois.
3. **Filtrar pela busca, não usar "o primeiro da lista".** Com item fixado
   (pin) no topo, o primeiro da lista não é a cópia mais recente — a primeira
   versão do harness mediu e validou o item errado.

Como rodar:

```
python _ref/harness_compare.py           # tudo
python _ref/harness_compare.py open      # só abertura
python _ref/harness_compare.py paste     # só colagem
python _ref/harness_compare.py capture   # só captura
```

Saída também em `_ref/compare_resultado.txt`.

### 8.26 v0.1.20 — revisão do fluxo de clipboard (e a mudança que faltava)

Revisão ponta a ponta de `capture.rs` + `start_watcher`: o que estava certo e o
que não estava.

**Estava certo** (mantido):

- Formatos "não registre isto" respeitados (`ExcludeClipboardContentFromMonitorProcessing`,
  `Clipboard Viewer Ignore`, `CanIncludeInClipboardHistory = 0`) e lista de
  gerenciadores de senha por processo dono do clipboard.
- Ordem de leitura: arquivos → GIF cru → bitmap → texto (com HTML e RTF), com
  `trim` de CR/LF/NUL e descarte de texto só de espaços.
- Hash só do **conteúdo** (imagem com `hash_bytes`, não com `from_utf8_lossy`),
  então o mesmo texto vindo do navegador e do editor é uma entrada só.
- Nossas próprias escritas no clipboard ficam em `ignore_sequences` (8 posições)
  — `paste_many` escreve várias seguidas e o watcher só consome uma por vez.
- Índice de detecção de link/e-mail/cor em `capture.rs` (com os testes de
  `#ggg`, `rgb()/hsl()`).

**O que estava errado — e foi corrigido:**

1. **Captura por polling de 320 ms.** Duas cópias dentro de uma janela de
   320 ms: a primeira **nunca** entrava no histórico (a sequência já era a da
   segunda). Agora um **listener do Windows** (`AddClipboardFormatListener` +
   `WM_CLIPBOARDUPDATE`, janela *message-only*) avisa no instante da cópia; o
   polling de 700 ms fica só como rede de segurança. Toda a captura roda numa
   thread de trabalho alimentada por canal, então o listener nunca segura a
   fila de mensagens. `LAST_SEQ` com `swap` impede que listener e polling
   processem a mesma cópia duas vezes.
2. **Atalhos registrados em bloco.** `on_shortcuts([show, paste])` falhava
   inteiro se **um** dos dois estivesse tomado (por exemplo `Ctrl+Shift+V` pelo
   Raycast) e o app ficava sem atalho nenhum, sem aviso. Agora cada atalho é
   registrado sozinho e o que falha vira aviso no log/`settings`.

Coberto por: a tabela do §8.27 (captura ≤ 30 ms e colagem em 21 ms).

### 8.25 v0.1.19 — a prévia do link era o problema (não o nome)

Depois da 0.1.18 o usuário foi claro: "melhorou mas não era sobre o nome era só a
visualização que eu queria mesmo o link pode manter". Olhando a Screenshot_25 de
novo com isso em mente: o cartão do link no painel de detalhes era uma **tira
estreita com a URL cortada e nenhuma imagem** — a prévia, não o rótulo.

Correções:

- **Rótulo da lista de volta para a URL** (`displayTitle` original). O título da
  página não foi pedido; o que faltava era a imagem.
- **Cartão volta a usar `link.title`** (título da página) com `entry.text` de
  reserva — o título ali é parte da prévia, não do rótulo.
- **Capa com candidatas em ordem de qualidade**: `og:image` da página e, para
  YouTube, `maxresdefault.jpg` (1280×720) e depois `hqdefault.jpg`. A primeira
  que baixa vira a capa; antes era uma tentativa só e, se falhasse, ficava vazio.
- **Bloco de reserva `.link-ph`**: sem capa (página sem `og:image`, arquivo de
  CDN, arquivo apagado) o topo do cartão mostra o favicon grande + o site sobre
  um gradiente, em vez de um retângulo vazio. `onError` na `<img>` troca para o
  bloco se o arquivo tiver sumido do cache.
- **`FETCH_VERSION = 5` e "sem capa" conta como incompleto** na fila: uma entrada
  com título mas sem imagem também é re-buscada.
- **Uma re-tentativa de capa por execução** (`capa_tentadas`, até 3 por abertura,
  só quando a fila normal está vazia): download recusado (429, site lento) se
  conserta sozinho na sessão seguinte, sem virar tráfego a cada abertura.

**Medido (debug, 25 links no banco real):**

| Entrada | Antes | Depois |
|---|---|---|
| `youtube.com/watch?v=dQw4w9…` | cartão sem imagem, URL cortada | capa **480×360** (hqdefault), altura 168, título da página |
| pichau Abaddon/Hades/mouse | sem prévia | capa OG **300×300** + descrição |
| `github.com/…/Leonida` | sem prévia | capa **opengraph 134 KB** (re-tentativa por execução) |
| `discord.gg/PEsaYjhF` | sem prévia | capa **889 KB** |
| klipy / shopee (Cloudflare) | retângulo vazio | bloco com favicon + `klipy.com` / `shopee.co.th` |
| `cdn.discordapp.com/…mp4` | URL crua | bloco com favicon + `cdn.discordapp.com` |

Placar: **16 de 25** links com capa real; os 9 restantes têm o bloco de site —
nenhum cartão fica vazio.

### 8.24 v0.1.18 — o buscador de metadados confiável (base da prévia)

O usuário apontou "alguns links que não aparecem corretamente" (Screenshot_25).
Investigando, duas falhas de **dados** (não de layout) sustentavam qualquer
prévia ruim:

1. **O `ureq` (rustls) leva 403 de vários sites.** Impressão digital de TLS
   própria: a Pichau responde `403` e o WinINet do sistema, testado como plano
   B, devolvia a página "Site em Manutenção". Só o **`curl.exe`** do próprio
   Windows (existe desde o 10 1803) recebeu o HTML do produto.
2. **A fila de re-tentativa só olhava ícone.** Quem tinha favicon mas não tinha
   título/capa nunca era buscado de novo — ficava incompleto para sempre.

Correções:

- **Plano B por `curl.exe`** (`CREATE_NO_WINDOW`, `Stdio::null()`, `--max-time 9`,
  `--compressed`, `--max-filesize 3 MB`) quando o ureq falha; teto de parse subiu
  para 2,5 MB porque a `<title>` do YouTube fica em ~907 KB de página.
- **YouTube por oEmbed** (`youtube.com/oembed`, parser JSON próprio com pares
  surrogados): título limpo, sem " - YouTube", para vídeo e canal.
- **Cabeçalhos de navegador** (`Accept`, `Accept-Language`) nos dois caminhos.
- **`titulo_ruim()`**: "Just a moment…", "Attention Required", "403 Forbidden",
  "Site em Manutenção" não viram "nome do link".
- **`LinkMeta.fetch_v` + `FETCH_VERSION`**: cada entrada incompleta ganha **uma**
  nova tentativa quando o buscador melhora, em vez de ficar presa no primeiro
  fracasso (`tried` sozinho não permitia isso).

| Link | Antes | Depois |
|---|---|---|
| pichau (3 entradas) | sem metadados (403) | título do produto + capa OG |
| `youtu.be/NrqjrUPgRsI` | sem título | `ABSURDO! Eu trouxe o GTA 6 pro meu PC (E TÁ LINDO!!)` |
| `youtube.com/@raycast` | sem título | `Raycast` |
| klipy (Cloudflare) | título `Just a moment...` | sem título (descartado), sem re-tentar |

### 8.23 v0.1.17 — apagar várias seleções (com confirmação)

Até a 0.1.16 o `Delete` só agia sobre **uma** entrada: com várias marcadas (Ctrl+clique),
o painel de ações não tinha como apagar o conjunto e a tecla apagava apenas o item
selecionado — o usuário via "Paste 3 Items" e "Save 3 Items As…" mas nenhum
"Delete 3". Agora existe o caminho completo, com confirmação obrigatória.

**Backend — um comando para o lote.**

`delete_entries(ids: Vec<String>)` em `lib.rs` pega `lock_store` **uma vez**, remove
todos e devolve `entries_now` (mesmo padrão de `clear_entries`). Apagar 200 itens
chamando `delete_entry` 200 vezes custaria 200 travadas do store e 200 payloads de
lista (~1,7 MB cada) — com o comando de lote é uma travada e um payload.
Exposto no front como `api.deleteEntries(ids)`.

**Confirmação (`src/components/ConfirmDialog.tsx`).**

Card de 430 px no mesmo `modal-backdrop` do diálogo "Save N items": ícone de
lixeira em tile vermelho, título `Delete N entries?`, aviso "They are removed from
the history for good — pinned entries included. This can't be undone." e a **lista
real do que vai embora** (até 5 títulos com o tipo em maiúsculas, o resto vira
"+N more") — o usuário confirma olhando os itens, não um número abstrato.

Teclado, com a regra de nunca apagar por acidente:

| Tecla | Efeito |
|---|---|
| foco inicial | **Cancel** (não o botão destrutivo) |
| `Enter` com foco em Cancel | **cancela** |
| `Enter` com foco em Delete | apaga |
| `←` / `→` | alterna o foco entre os dois botões |
| `Esc` | cancela |

Os eventos são tratados em **capture** com `stopImmediatePropagation`, senão o `Esc`
vazaria para o handler global e esconderia a janela; o handler principal também tem
`if (confirmIds) return`. O botão destrutivo é `.btn-danger` (fundo `var(--red)`).

**Onde dá para chegar na exclusão múltipla.**

1. `Delete` com **mais de uma** entrada marcada → abre a confirmação;
2. `Delete` com **nenhuma/uma** marcada → apaga a entrada selecionada na hora, como
   antes (sem diálogo — apagar um item é barato e reversível pela memória do usuário);
3. painel de ações (`Ctrl+K`) com seleção múltipla → novo item **"Delete N Entries"**
   (danger, keycap `DELETE`) entre "Save N Items As…" e "Clear Selection (N)".

Depois de confirmar: `selection` vai para vazio, o painel fecha e o rodapé mostra
`N entries deleted` (ou `Delete failed - …` em erro, com a lista intacta).

**Medido (debug, mouse/teclado reais via `Input.*` do CDP):**

| O que | Resultado |
|---|---|
| `Ctrl+clique` em 2 linhas + `Delete` | diálogo `Delete 2 entries?`, os 2 títulos listados, foco em **Cancel** |
| `Esc` no diálogo | fecha o diálogo, **as 2 continuam marcadas**, janela viva |
| `Enter` com foco em Cancel | **cancela** (nada apagado) |
| `→` + `Enter` | apaga as 2: restou 1 de 3 entradas de teste, marcadores limpos, rodapé `2 entries deleted` |
| `Delete` com 1 selecionada | apaga direto, **sem diálogo** |
| `Ctrl+K` com 2 marcadas | painel mostra `Delete 2 Entries` + keycap `DELETE`, danger |
| item do painel (mousedown real) | fecha o painel, abre a confirmação com os 2 itens |

**Pegadinhas encontradas no caminho** (para não repetir): os itens do painel de ações
usam `onMouseDown` (não `onClick`) — `.click()` sintético não dispara nada; e o token
de cor é **`--red`**, não `--color-red`: `var(--color-red)` sem fallback resolve
inválido e a declaração é descartada (o botão destrutivo ficava cinza).

### 8.22 v0.1.16 — micro-comportamentos: deduplicação, ordem da busca e digitação

Três ajustes de "parece natural" (a lista veio de uma análise externa, linha por linha
verificada no código):

**1. Copiar a mesma coisa de lugares diferentes.** A deduplicação por hash já existia
(§7.1), mas tinha dois furos:

- o hash incluía **HTML/RTF**: o mesmo texto copiado do navegador e do editor virava
  **duas** entradas. Agora o hash é só do **conteúdo** (`hash_of(&["text", texto])`);
  a origem continua guardada como metadado (`source`, `source_icon`) no mesmo registro.
- ao promover o item repetido, o código **criava um id novo**, copiava os blobs de
  imagem para o id novo e apagava a linha antiga. Agora o **id é preservado**: o
  `upsert` sobrescreve a mesma chave primária, os blobs já estão no lugar e as URLs
  `http://rayclip.localhost/img/<id>` continuam válidas (só o `?v=created_at` muda).

O que se mantém de propósito: `pinned`, `ocr_text`, `link`, `html`/`rtf` antigos quando
o novo não tem, e o `source` passa a ser o da última cópia.

Dados sensíveis: `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer
Ignore` e `CanIncludeInClipboardHistory = 0` já barravam a gravação (§7.1), então
gerenciador de senha **não** entra nem promove nada.

**2. Ordem da busca.** Antes a lista era sempre re-agrupada por data (Pinned/Today/…),
então o ranqueamento era impossível: `groupEntries` destruía qualquer ordem por
relevância. Agora, **com busca ativa**, os resultados vão num bloco único sem rótulo
(`buckets = [{ label: "", entries }]`) em ordem de relevância (`rankEntries` em
`format.ts`):

1. item de texto/vínculo **antes** de imagem que só casou pelo OCR (`tier`);
2. dentro do tipo, **fixados** primeiro;
3. casamento **exato**, depois **início**, depois **começo de palavra**, depois **meio**
   (`hitRank`);
4. empate → **recência** (o `sort` é estável e a lista de entrada já vem da mais recente).

**3. Digitação, seleção e prévia.**

- O **primeiro resultado fica sempre selecionado** (`useEffect` sobre `flat`): antes,
  digitar até o item selecionado sair do filtro deixava a seleção nula e o `Enter` não
  colava nada.
- Foco na busca com **texto selecionado** (`focusSearch()` → `focus()` + `select()`),
  para digitar substituir; `Ctrl+A` dentro do campo pega só o campo.
- `Enter` continua colando e escondendo em ~45 ms (§8.15).

**Medido (debug, teclado/mouse reais via `Input.*` do CDP, 1.620 entradas):**

| O que | Resultado |
|---|---|
| copiar "DEDUP-PROVA-2026" 2× (fixado entre as cópias) | **1** entrada, **mesmo id** `60760bbd`, `pinned=true`, total inalterado, índice 0 |
| copiar a mesma imagem 2× | **1** entrada, mesmo id `55cf995d`, `hasImage=true`, `/img/<id>` ainda carrega **3840×2160** |
| copiar mesmo texto com/sem formatação | (o `CF_HTML` do teste não foi reconhecido pelo Windows — verificado só no código) |
| busca "processos" (1.620 entradas) | 18 de texto primeiro, **4 de OCR depois** (`primeiroOcr=18 > ultimoTxt=17`) |
| busca "windows" | 22 resultados, fixados no topo, primeiro item **já selecionado** |
| busca exata "DEDUP-PROVA-2026" | 1 resultado, **é o primeiro** e está selecionado |
| latência digitar→pintar (1 rAF, 1.620 entradas) | **7,9–12,4 ms** |
| 8 × `↓` seguidos | seleção passou por 8 entradas diferentes, prévia acompanhou, **~7 ms** por passo |
| `Ctrl+A` + `Backspace` no campo | limpa só a busca (não a página) |

### 8.21 v0.1.15 — a prévia quebrou na 0.1.14 (duas pegadinhas de CSS)

O usuário reportou a prévia "toda estranha" e a mãozinha de arrastar errada.
Duas causas independentes, as duas introduzidas no zoom da 0.1.14:

1. **A imagem crescia até o tamanho natural e era cortada pelo rodapé.** O
   `ZoomableImage` aplicava `style={{ maxHeight: "100%" }}` no `<img>`. Contra um
   pai de altura automática, **porcentagem de `max-height` é ignorada** — e isso
   sobrescrevia o `.preview img { max-height: min(340px, 52vh) }`. A imagem ia de
   897×486 para ~897 px de altura dentro de uma coluna de 389 px.
2. **No card grande a imagem continuava estourando.** Mesmo tirando o inline, a
   regra `.viewer-body .zoomwrap img { max-height: 100% }` não resolvia porque o
   `.zoomwrap` era `display: grid` com **track automático**: porcentagem contra
   track de tamanho indefinido também é ignorada (o track era dimensionado pelo
   próprio conteúdo → 377 px dentro de um corpo de 282 px).

Correções:

- `max-width`/`max-height` inline removidos do `<img>`; o enquadramento vem do
  CSS de cada contexto (`.preview` mantém `min(340px, 52vh)` e largura 100%).
- `.zoomwrap` virou **flex** (`align-items/justify-content: center`), em vez de
  grid: em flex a porcentagem de `max-height` resolve contra a altura definida do
  contêiner, então o card grande encaixa a imagem (696×282 de 282 disponíveis).
- Cursor de arrasto: `.preview-zoomable .zoomwrap.zoomed` (0,3,0) vencia
  `.zoomwrap.grabbing` (0,2,0) — a mãozinha ficava em `grab` durante o arrasto.
  Adicionada `.preview-zoomable .zoomwrap.zoomed.grabbing { cursor: grabbing }`.
  O estado `dragging` agora é **estado React** (antes era `ref`, e a transição
  ficava presa em `none` depois do primeiro arrasto).
- `user-select: none` no `.zoomwrap` (arrastar não seleciona a imagem).

**Medido (debug, mouse de verdade via `Input.dispatchMouseEvent`):**

| O que | Antes (0.1.14) | Depois (0.1.15) |
|---|---|---|
| prévia no painel (897×486 natural) | altura estourava a coluna, cortada no rodapé | **469×254** (limite `min(340, 52vh)`), nada cortado |
| imagem no card grande | **696×377** dentro de 282 px disponíveis (`dentro: false`) | **696×282**, `dentro: true` |
| cursor com zoom | `grab` (nunca `grabbing`) | `grab` → **`grabbing`** durante o arrasto → `grab` |
| arrasto no painel | — | `translate(-30px, -20px)` aplicado, card **não** abriu |
| arrasto no card | — | `translate(20px, 12px)` |
| duplo clique | — | volta a `scale(1)` |
| arrasto com zoom < 100% | — | não move (pan só com `zoom > 1`) |
| card do "Save N items" | — | imagem **400×217** dentro de 400×250 |

### 8.20 Correções do zoom e revisão de padrões que quebram o app

Dois defeitos encontrados usando a v0.1.13:

1. **O scroll rolava a coluna em vez de tirar o zoom.** Causa: o React registra
   `onWheel` como **passivo**, então `preventDefault()` era ignorado e o wheel
   vazava para o `overflow-y: auto` da coluna de detalhes. Além disso o zoom
   parava em 100%, então girar para baixo não fazia nada e o mesmo evento rolava.
2. **Não dava para reduzir abaixo de 100%.**

O que mudou:

- `wheel` é registrado numa **ref com `{ passive: false }`** (`ImageViewer.tsx`),
  com `preventDefault` + `stopPropagation` de verdade.
- Faixa de zoom **25% a 800%** (era 100%–800%).
- No painel de detalhes o zoom só é interceptado **depois de clicar na imagem**
  (`armed`). Antes disso o scroll rola a coluna normalmente, como o usuário pediu.
  Um selo "scroll to zoom · Esc to exit" e o cursor `zoom-in` avisam do estado.
- O clique na prévia **não abre mais** o card grande (era o gesto que o usuário
  queria para armar o zoom): o card grande agora tem um **botão ⤢** dedicado, o
  item **Open Preview (Space)** no painel de ações e o próprio `Espaço`.

**Padrões de quebra revisados** (cada linha virou uma guarda no código):

| Risco | Guarda |
|---|---|
| `Esc` no modo zoom fecharia/esconderia a janela | listener em **capture** com `stopImmediatePropagation`, só enquanto `armed` |
| `Esc` enquanto se **renomeia** ou digita na busca | o handler sai fora se `activeElement` é `INPUT`/`TEXTAREA`/`contentEditable` |
| Dois `Esc` concorrentes (zoom da prévia + card grande) fechariam o card no segundo toque | abrir o card **desarma** o zoom (`setZoomArmed(false)` em todos os caminhos: ⤢, `Espaço`, painel de ações) |
| Scroll preso no zoom sem saída (não conseguiria rolar a coluna) | clique **fora** da imagem desarma (`pointerdown` em capture), `Esc` desarma, trocar de entrada desarma, reabrir a janela desarma |
| Arrastar a imagem abriria o card ao soltar o botão | `moved` (>3 px) cancela o clique |
| Pan quando a imagem está menor que 100% | pan só com `zoom > 1` |
| Zoom de página do WebView2 com `Ctrl`+scroll | medido: `devicePixelRatio`, largura e altura não mudam (nada a fazer) |
| Selo de zoom em cima do selo "GIF" | `%` foi para a **esquerda**, dica para o **topo-esquerdo** |
| Preview do "Save N items" herdando o comportamento antigo | o mesmo componente, já com o wheel não-passivo |

**Verificado com entrada real** (`_ref/cdp_wheel.py`, `Input.dispatchMouseEvent`
de wheel e clique de verdade, não evento sintético; coluna forçada a rolar com um
espaçador de 1600 px):

| Passo | Resultado medido |
|---|---|
| wheel **sem** clicar na imagem | `.details.scrollTop` 0 → **360** e zoom **100%** (scroll livre) |
| clique na imagem | `armed=true` |
| wheel **para baixo** armado | zoom **66%**, `scrollTop` continua **0** |
| wheel **para cima** armado | **115%** |
| `Esc` | volta a **100%**, `armed=false`, janela viva |
| ⤢ → card grande | 720×349 |
| wheel para fora no card | **50%** (antes travava em 100%) |
| `Espaço` com a prévia armada | card abre e **desarma** a prévia; o 1º `Esc` fecha o card |
| clique na tabela de informações | desarma |
| teclado real: `Alt+Espaço`, `↓`, `Ctrl+K`, `Esc`, `Esc` | mostra → painel → esconde (nada quebrou) |

### 8.19 Zoom na imagem e visualizador grande (Quick Look do app)

Antes a imagem copiada só aparecia enquadrada no painel de detalhes: para ver um
detalhe de um print, não havia nada a fazer.

**Componente único** (`src/components/ImageViewer.tsx`) com dois usos:

- `ZoomableImage` — imagem com **zoom no scroll** (1× a 8×, passo de 15%) e
  **arrasto** para mover quando ampliada, badge com a porcentagem, duplo clique
  volta a 100%. Arrastar **não** conta como clique: só um clique sem movimento
  dispara `onTap` (senão mover a imagem abria a prévia).
- `ImageViewer` — card grande centrado (padrão 720×349 numa janela de 900×489),
  **redimensionável** pelo canto (320×240 até quase a janela inteira), com zoom e
  arrasto dentro, rodapé com `%` e dica, e fechamento por `Esc`, `Espaço` ou
  clique fora. O `Esc`/`Espaço` são tratados em **capture** com
  `stopImmediatePropagation` para não vazarem para os atalhos do app (um `Esc`
  ali esconderia a janela em vez de fechar a prévia).

**Como abrir:** clique na prévia do painel de detalhes, item **Open Preview**
(`Espaço`) no painel de ações, ou `Espaço` com uma imagem selecionada. O
`Espaço` só abre a prévia quando o campo de filtro está vazio — com busca
digitada ele continua sendo espaço.

O mesmo `ZoomableImage` foi reaproveitado dentro do "Save N items" (§8.17), então
o card grande de lá também ganhou zoom e arrasto.

**Medido (debug, via CDP):** três eventos de scroll na prévia → `scale(1.52)` e
badge **152%**; clique simples → card **720×349 em x=90, y=70**; scroll dentro do
card → **175%**; arrasto → `translate(60px, 40px)`; canto → card de **720×349
para 600×279**; arrastar na prévia **não** abriu o card, clique sem arrasto
abriu; `Espaço` fechou e reabriu; `Esc` fechou e a janela principal continuou.
