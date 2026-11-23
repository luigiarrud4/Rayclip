# Rayclip — documentação completa

Rayclip é um **gestor de histórico de área de transferência** para Windows, feito do
zero, que recria **apenas** o recurso *Clipboard History* do Raycast para Windows —
inclusive a aparência. O Raycast original foi desmontado (engenharia reversa) para
saber o que copiar: layout, cores, fontes, tempos e comportamento. Nada de código
dele é usado; só o resultado visual e funcional foi reproduzido.

- Pasta do projeto: `C:\Users\Lugia\Projects\rayclip`
- Stack: **Tauri v2 (Rust) + React + TypeScript + WebView2**
- Versão atual: **0.1.16**
- Binário: `src-tauri/target/release/rayclip.exe`
- Instalador: `src-tauri/target/release/bundle/nsis/Rayclip_0.1.16_x64-setup.exe`
  (3.709.170 B, SHA256 `8843b55643b6dd48dfb8f5048040d819d84539464e085da3bccc462116a18580`)

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
