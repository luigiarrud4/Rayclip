# Rayclip — engenharia reversa do Raycast (Clipboard History)

Recriação do **Clipboard History** do Raycast para Windows, com visual copiado do
original (tokens de design extraídos do bundle do próprio Raycast) e funcionalidade
nativa real de clipboard.

> **Documentação completa:** [`DOCUMENTACAO.md`](DOCUMENTACAO.md) — arquitetura,
> módulos, modelo de dados, segurança, funcionalidades e o registro de **como
> cada problema difícil foi diagnosticado e resolvido** (com as medições).

## 1. Engenharia reversa — Raycast para Windows (2.13.1)

**Empacotamento:** MSIX (`AppxManifest.xml`), com dois executáveis
(`Raycast.exe` e `Raycast.UIAccess.exe` com capability `uiaccess` para hotkeys
globais mesmo sem foco).

### Stack identificada (linguagens/arquitetura)

| Camada | Tecnologia | Evidência |
|---|---|---|
| Shell nativo | **C# / .NET 10 (WPF)** | `Raycast.dll` (9.8 MB), `Raycast.deps.json` (`.NETCoreApp,Version=v10.0/win-x64`), assemblies `CommunityToolkit.Mvvm`, `WPF-UI 4.3` (docs `AppxManifest` mencionam `.NETFramework v4.5.1` só p/ o bridge MSIX) |
| Renderer | **WebView2** (Edge Chromium) | `Microsoft.Web.WebView2.*` + pasta `LocalState/EBWebView` |
| UI | **React + TypeScript** (bundles Vite) | `frontend/main-window-k3hl20GT.js` (5.8 MB minificado, `ClipboardHistoryView`, CSS modules `clipboard-list-item__*`) |
| Backend | **Node.js (ESM)** | `backend/index.mjs` (8.4 MB) — IPC + store |
| Nativo | **Rust (N-API addons)** | `data.win32-x64-msvc.node` (78 MB), `indexer.node`, `win32-x64.node` |
| Persistência | **SQLite** (`Microsoft.Data.Sqlite`) + JSON | |
| Outros | Serilog/Sentry (telemetria), NAudio (áudio), PdfPig (PDF), ZXing (QR), H.NotifyIcon (tray) | |

### Padrões observados no binário (strings/IL)

- Clipboard movido por mensagens nativas: `ClipboardChangedNotifyNative` e
  `GetClipboardSequenceNumber` (observador barato — sem ler o clipboard a cada tick).
- Tipos de itens: `ClipboardData+ContentType` (`Text`, `Link`, `Color`, `Image`,
  `File(s)`, `Email`, ...), `HostClipboardEntry*`, `HostClipboardItemRepresentation`.
- IPC com o backend via `Raycast.IPC.Types.Clipboard.*` (`ClipboardAddInput`,
  `ClipboardPasteableEntry*`, `PlaceInClipboardInput`, `CopyFileInput`).
- Config organizacional: `clipboard_history_retention_days`,
  `clipboard_history_disabled_applications`.

### Design tokens (bundled-raycast-dark, extraídos dos bundles do frontend)

```css
--bg:#000000; --fg:#FFFFFF; --accent:#4FA3F8; --selection:#FFFFFF;
--red:#FF6363; --orange:#FF9217; --yellow:#FFC531; --green:#59D499;
--blue:#56C2FF; --purple:#A485FF; --magenta:#CF2F98;
--color-text-secondary:var(--fg-60); --color-text-tertiary:var(--fg-40);
--color-border-separator:var(--fg-10); --color-surface-control:var(--fg-10);
```

- Janela principal: **750 × 475**, header 40 px (Windows), footer 42 px.
- Master-detail: lista flex + detalhes fixo **263 px** (`master-detail-layout__detail[data-size=small]`).
- Fonte: **InterVariable** (UI) + JetBrains Mono (prévia de texto) — arquivos copiados do próprio `frontend/` do Raycast.
- Grouping: `Pinned / Today / Yesterday / This Week / This Month / Earlier`;
  linha 30 px, ícone 20 px em tile `fg-8`, seleção `fg-10`, radius 6.
- Dropdown de filtro: `All Types / Text Only / Images Only / Files Only / Links Only / Emails Only / Colors Only`.

## 2. O que foi recriado (Rayclip)

**Stack:** Tauri v2 + React + TS + Rust (a própria UI do Raycast é web, então
copiar o CSS reproduz o visual 1:1 com um binário ~10 MB).

### Funcionalidades (todas testadas/validadas)

- **Texto do painel de detalhes é selecionável**: `.details` usa
  `user-select: text` e o preview/OCR usa `cursor: text`, então dá para
  arrastar o mouse e selecionar/copiar o texto (antes o `user-select: none`
  global bloqueava). A seleção não move a janela: o arrasto só é capturado em
  `.drag-strip`, `.nav` e `.actionbar-left`.
- **Faixa de arrasto no topo**: 16 px (era 6 px) acima da navbar, além da
  própria navbar — o topo inteiro serve para arrastar a janela.
- **Texto longo rola por inteiro no painel de detalhes**: o preview deixou de
  ser uma linha cortada com fade — agora é um bloco com `overflow-y: auto`
  (altura máxima ~46vh) onde o texto inteiro pode ser lido rolando.
- **Duplo clique cola, clique simples só seleciona**: o clique seleciona/mostra
  o preview; o duplo clique executa o paste daquela entrada (texto ou imagem).
- **Seleção múltipla com ordem de colagem**: `Ctrl+clique` alterna uma entrada,
  `Shift+clique` pega o intervalo. Cada marcada recebe um **badge azul
  numerado** (1, 2, 3…) e o painel da direita lista "Selected · N items" na
  ordem. `Enter` (ou `Ctrl+Enter` mantendo a janela aberta) cola **todas em
  ordem** via `paste_many` — copia a entrada, cola com `Ctrl+V`, espera ~340 ms
  e repete. `Esc` limpa a seleção sem fechar a janela.
- **Salvar imagem como…** (`Save Image As…` no painel de ações, só aparece em
  imagens): abre o diálogo nativo do Windows (`IFileSaveDialog`) com nome
  `Rayclip <data> <hora>.png` e **PNG como padrão**, e um submenu para escolher
  **PNG / JPEG / WebP / BMP / GIF**. JPEG sai em qualidade 92, WebP lossless,
  e GIF de origem mantém os **bytes originais** quando o destino é GIF (animação
  preservada). O submenu é **estável ao mouse**: passar o cursor pelas linhas só
  move o destaque (antes o hover fechava o submenu e o painel voltava para as
  ações gerais) e o destaque define o formato salvo no `Enter`.

- **Captura real**: watcher Rust verifica `GetClipboardSequenceNumber()` a cada
  320 ms e classifica como **texto, link, email, cor (#hex), imagem (CF_BITMAP→PNG)
  ou arquivos (CF_HDROP)**.
- **OCR nas imagens** (igual ao Raycast, "OCR index for images and scanned
  PDFs"): usa o motor nativo do Windows (`Windows.Media.Ocr` via WinRT) em thread
  própria após capturar a imagem; o texto fica em `entry.ocrText`, entra na busca
  (a linha ganha o selo azul **Aa**) e aparece no painel de detalhes em
  **Text in Image**. Imagens antigas são indexadas no start (`ocr_pending`), e
  `Ctrl+T` / "Copy Text from Image" copia o texto reconhecido.
- **Paste automático**: Enter cola na janela que estava em foco antes do app
  (`SetForegroundWindow` + `SendInput` Ctrl+V, com fallback `SwitchToThisWindow`).
- **Hotkeys globais**: `Alt+Space` ou `Ctrl+Shift+V`; `Esc` esconde, `Ctrl+K`
  abre o painel de ações, setas navegam, digitação filtra, `Ctrl+P` fixa,
  `Ctrl+Backspace`/`Ctrl+Shift+Backspace` deleta/limpa, `Ctrl+O` abre link.
- **Botão direito** numa entrada (ou em qualquer ponto da janela) abre o painel
  de ações daquela entrada, como no Raycast; o menu nativo do WebView2 é suprimido.
- **Cantos arredondados** de verdade na janela via DWM
  (`DWMWA_WINDOW_CORNER_PREFERENCE`), já que a janela é opaca.
- **Janela arrastável e redimensionável** (o Raycast também é: changelog tem
  "Fixed slow window dragging" e "non-resizable Raycast windows"): arraste pelo
  header ou pelo rodapé (`data-tauri-drag-region`), redimensione pelas bordas
  (`resizable` + mínimo 520×340). Tamanho/posição ficam salvos em `window.json`
  e são restaurados no próximo start.
- **Cópia/colagem fiel ao original** (como o Raycast, que "saves every original
  format"): imagens vão para o clipboard em **CF_DIB + CF_DIBV5 + CF_BITMAP +
  PNG** (só CF_BITMAP fazia vários apps colarem nada ou conteúdo antigo), e texto
  com formatação carrega o **HTML/RTF originais**; "Paste as Plain Text" descarta.
- **Miniaturas reais de arquivos (PDF, vídeo, doc)**: geradas pelo mesmo
  provedor do Explorer (`IShellItemImageFactory`), com cache em
  `images/thumb-*.png`. Só o item **selecionado** gera miniatura — nunca o
  histórico inteiro.
- **Miniatura de vídeo em links**: YouTube (`youtu.be/<id>`, `watch?v=`,
  `/shorts/`, `/embed/`, `/live/`) usa `i.ytimg.com/vi/<id>/hqdefault.jpg` como
  capa quando a página não traz `og:image`.
- **Performance / economia de recursos** (como o Raycast, que só desenha o
  visível):
  - **lista virtualizada** — com centenas ou milhares de itens só existem no DOM
    as linhas da tela (medido: 26–33 linhas com 3.526 entradas em vez de 3.526);
  - **GIF não anima na lista**: cada GIF ganha um *poster* estático (1º quadro) e
    só a prévia selecionada é o GIF animado — com 50 GIFs no histórico, apenas 1
    anima;
  - imagens com `loading="lazy"` + `decoding="async"`;
  - **no máximo 2 trabalhos pesados em paralelo** (OCR, metadados de link,
    miniatura) com fila própria — o app não dispara 500 requisições de uma vez;
  - painel de detalhes **rolável** (rolagem testada: 126 mil pixels mudaram).
- **Links viram cartões de site (como no Raycast)**: ao copiar uma URL, o app
  busca **uma vez** o HTML da página e guarda em cache no próprio registro:
  `og:title`/`<title>`, `og:description`, `og:site_name`, o **favicon** (inline,
  data URL) e a **capa** `og:image` (salva em `images/og-*.png|jpg`). O favicon
  aparece na linha da lista e o painel mostra um card com capa, site, título e
  descrição — igual ao "https://youtube.com/@raycast" do Raycast original.
  Entradas antigas são indexadas ao abrir o app (`link_pending`) e recopiar a
  mesma URL **não perde** os metadados já baixados.
- **GIF animado toca no preview**: quando o clipboard publica o formato `GIF`
  (`image/gif`), os bytes originais são gravados como `.gif` (não viram PNG),
  então o preview **anima** de verdade e ganha o selo "GIF" + tipo "GIF Image".
- **Feedback visual de sucesso/erro**: quando copia/cola dá certo, a bolinha do
  clipboard no rodapé fica **verde com brilho e névoa** e mostra a mensagem;
  quando falha, fica **vermelha** com o motivo (ex.: "Copy failed — …"). O paste
  espera 220 ms para você ver o verde antes de esconder a janela.
- **Layout corrigido medindo as suas capturas do Raycast original**: nas duas
  referências a divisória vertical cai em **~35% da largura** (x=235 de 674 e
  x=223 de 582). Ou seja, os **263 px do CSS extraído são a LISTA**, não o painel
  de detalhes — eu havia invertido. Agora: **lista fixa de 315 px (~35%) e
  painel de detalhes ocupando os ~65% restantes**, como no Raycast real (títulos
  da lista truncados com "…", preview grande, metadados e OCR à direita).
- **Janela 900×570 por padrão** (mesma proporção 1.578 das referências e do
  750×475 do bundle), mínima 520×340; abaixo de 700 px de largura a lista passa a
  dividir em 46%.
- **Segurança (como o Raycast)**:
  - respeita os marcadores nativos que apps usam para "não grave isto":
    `ExcludeClipboardContentFromMonitorProcessing`,
    `Clipboard Viewer Ignore` e `CanIncludeInClipboardHistory = 0`
    (o equivalente Windows ao `ConcealedType` do macOS que o Raycast ignora);
  - nunca grava conteúdo de **gerenciadores de senha** (1Password, Bitwarden,
    KeePass/KeePassXC, LastPass, Dashlane, Keeper, Enpass, RoboForm, NordPass,
    ProtonPass) — verificado: texto com o marcador não entrou no histórico;
  - o Raycast ainda tem `clipboard_history_disabled_applications` (ignorar apps)
    e banco **criptografado** (`clipboard.db`/`settings.db` com header aleatório,
    SQLCipher-style). **O nosso também é cifrado** — e não "no futuro": desde a
    v0.1.4 todo o conteúdo mora em um banco SQLCipher com a chave selada por
    DPAPI (§ *Banco cifrado* logo abaixo).
- **Build de release pede UAC** (`requireAdministrator` no manifesto embutido via
  `tauri_build::WindowsAttributes`), permitindo colar em janelas elevadas; o debug
  fica sem pedido para não atrapalhar o desenvolvimento. A janela também força
  **always-on-top** ao ser exibida.
- **Esconder ao perder foco**: três camadas, e a principal usa **hook do
  sistema** (não polling):
  1. **`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`** (`win32::on_foreground_change`)
     — o Windows avisa no instante em que outra janela vira primeiro plano e a
     **decisão roda dentro do próprio callback** (`try_hide_now`), sem thread
     intermediária: esconde em **2–3 ms** depois do `mouse-up`.
  2. o evento `Focused(false)` da janela (com confirmação dupla, ~480 ms) — rede
     de segurança para trocas que não geram evento de primeiro plano.
  3. um vigia de **150 ms** que relê `GetForegroundWindow` + `GetCursorPos`
     (último recurso; é ele que esconde ~180–260 ms depois de você tirar o
     cursor da janela num Alt+Tab).

  As **três** só decidem pelo primeiro plano real do Windows (`foreground_is_foreign()`: `GetForegroundWindow` +
  PID + classe + owner). Isso corrigiu a janela que sumia sozinha ao abrir as
  **configurações**, ao abrir **menus** (painel de ações, dropdown de
  "Maximum entries") e ao abrir o **"Salvar como"**: o WebView2 às vezes move o
  foco para uma janela filha e o tao reporta `focused = false`, mas o app
  continua sendo o dono do primeiro plano. Regras:
  - só esconde com **duas leituras seguidas** de "outro app em primeiro plano"
    (um solavanco de foco não fecha nada);
  - janela sem primeiro plano (`GetForegroundWindow` nulo) ou invisível = não
    esconde;
  - qualquer janela do **nosso processo** (inclui o diálogo nativo "Save image
    as") e popups `Chrome_WidgetWin*` sem título contam como "nós";
  - `dialog_open` (trava em `AppState`) mantém a janela viva enquanto o diálogo
    de salvar está aberto, mesmo que outro app vá para o primeiro plano;
  - **`cursor_over_window()`**: com o ponteiro do mouse sobre a janela ela nunca
    é escondida (mexer o mouse, abrir menu, clicar num campo) — só esconde
    quando o cursor está fora dela **e** o primeiro plano é de outro app;
  - **`foreground_is_transient()`**: janelas de console (`ConsoleWindowClass`,
    `CASCADIA_HOSTING_WINDOW_CLASS`, `PseudoConsoleWindow`) nunca contam como
    "outro app", porque aparecem e somem em milissegundos (`reg.exe`,
    `taskkill`, `cmd`);
  - carência de 900 ms depois de sermos o primeiro plano, para o foco voltar.

- **Nenhum processo de console piscando** (a causa real do foco perdido): o app
  é GUI, então cada `reg.exe`/`taskkill`/`cmd start` spawnado criava uma
  **janela de console que piscava e roubava o primeiro plano** — era o "cmd
  piscando" ao abrir Configurações (que consulta `DisabledHotkeys` no
  registro). Todos os spawns agora usam
  `CREATE_NO_WINDOW` (`0x0800_0000`) + `Stdio::null()`:
  `sysinteg::reg`, `sysinteg::restart_explorer`, `open_url`, `reveal_path` e o
  PowerShell do autostart.
  O log de depuração registra a decisão com o motivo:
  `blur: hide [fg=hwnd=… pid=… class=… title=…]`.
- **Persistência**: `%APPDATA%\com.rayclip.app\clipboard\store.json` (JSON) +
  `images/*.png`; pin fixa itens; instância única (mutex);
  dedupe por hash de conteúdo.
- **UI**: header (voltar, busca "Type to filter entries…", dropdown "All Types"),
  lista agrupada por data com ícone/prévia/amostra de cor, painel de detalhes com
  preview + Metadata/Information + Text in Image, barra inferior ("Clipboard
  History" · "Paste ↵" · "Actions Ctrl K"), painel de ações com keycaps, e
  estados vazios distintos ("No clipboard entries yet" vs "No results for ‘…’").

### Configurações (v0.1.x)

Página de configurações acessível por três caminhos: **clique no ícone de
prancheta** na barra inferior, **Ctrl + ,** ou a ação **"Settings…"** (clique
direito → painel de ações). Ela substitui a janela inteira e fecha com `Esc`.

| Aba | Conteúdo |
| --- | --- |
| **Preferences** | modo **100% offline/privado**, preview ao passar o mouse, **5 temas** e cor de destaque |
| Shortcuts | 2 atalhos globais + 9 atalhos internos, todos regraváveis |
| Pasting | colar sempre sem formatação; manter a janela aberta após colar |
| History | limite de entradas (100…5000) e retenção (1 dia…1 ano, ilimitado) |
| Privacy & Indexing | ignorar gerenciadores de senha, OCR (o preview de link mora em Preferences) |
| System | assumir o **Win+V**, reiniciar o Explorer, iniciar com o Windows |
| About | versão, caminho do executável e da pasta de dados |

**Atalhos configuráveis.** Os atalhos globais são re-registrados na hora
(`apply_shortcuts`), sem reiniciar o app. Os atalhos internos viram um mapa
(`inAppKeys`) usado tanto pelo teclado quanto pelos keycaps do painel de ações,
então o painel sempre mostra a combinação real:

| Ação | Padrão |
| --- | --- |
| Abrir histórico | `Alt+Space` / `Ctrl+Shift+V` |
| Colar | `Enter` (Shift+Enter = texto puro) |
| Colar e manter aberto | `Ctrl+Enter` |
| Painel de ações | `Ctrl+K` |
| Fixar / desafixar | `Ctrl+P` |
| Apagar entrada | `Ctrl+Backspace` |
| Limpar histórico | `Ctrl+Shift+Backspace` |
| Copiar texto da imagem | `Ctrl+T` |
| Abrir link | `Ctrl+O` |
| Configurações | `Ctrl+,` |

Gravar um atalho: clique no botão do atalho (ele fica azul, "Press keys…") e
pressione a combinação. É exigido ao menos um modificador; durante a gravação o
resto da interface ignora o teclado (`body[data-recording]`).

**Como o Rayclip toma o Win+V do Windows.** O Explorer captura `Win+<letra>` a
partir da lista `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced`
→ `DisabledHotkeys`. Ativar "Take over Win+V" acrescenta/remove a letra `V`
sem mexer nas outras letras que já estejam lá, e a aba mostra o **estado real**
do registro (`Win+V disabled` / `Win+V active in Windows`). A mudança só vale
depois que o Explorer reinicia — por isso existe o botão **Restart Explorer**
(as janelas do Explorador abertas fecham). Desativar remove o valor do
registro, devolvendo o painel nativo do Windows.

**Iniciar com o Windows (já como administrador).** O app tem manifesto
`requireAdministrator`, e a chave `Run` **não eleva** — no login o Windows
mostraria UAC ou simplesmente não abriria. Por isso o caminho principal é uma
**Tarefa Agendada**:

```
schtasks /Create /TN Rayclip /TR "<caminho do exe>" /SC ONLOGON /RL HIGHEST /F
```

`/RL HIGHEST` = privilégios máximos, então o Rayclip sobe no login **sem prompt
de UAC** (o que é obrigatório para o takeover do Win+V funcionar em qualquer
app). Verificado nesta máquina: `RunLevel=HighestAvailable`,
`LogonType=InteractiveToken`, `Status=Pronto`, `Tipo de Agendamento=No logon`.
Ao ativar, as formas antigas (`Run` e o `.lnk` em Inicializar) são removidas
para não abrir duas instâncias; ao desativar, `schtasks /Delete` e as duas
alternativas são limpas. A aba **System** mostra o estado real: *scheduled task
active / not registered* e *running as administrator / standard user*
(`system_status.startupTask` e `.elevated`).

Se o agendador não estiver disponível, o app cai para a chave `Run` e, por
último, para um atalho `.lnk` na pasta Inicializar via PowerShell oculto.

**Bandeja (tray) e instância única.** `setup_tray` cria um ícone
(`tray_icon_app`) com tooltip "Rayclip · Clipboard History": **clique esquerdo
abre a janela**, o menu (clique direito) tem *Open Rayclip*, *Settings…* e
*Quit Rayclip* (`app.exit(0)`; fechar a janela continua apenas escondendo). Uma
segunda execução do `.exe` (duplo clique no atalho, menu Iniciar, `Executar`)
não abre outra instância: o processo novo sinaliza um evento nomeado
(`Local\RayclipActivateV1`) e sai, e a instância viva mostra a janela
(`win32::start_activate_watch`). Verificado: dois lançamentos → 1 processo,
log `show_window` na instância original.

Os seletores usam um **dropdown próprio** (`components/Dropdown.tsx`): o
`<select>` nativo abria um popup **branco com texto claro** no WebView2 — os
itens ficavam ilegíveis. O menu custom segue os tokens do Raycast e aceita
setas, Enter e Esc.

Tudo fica em `%APPDATA%\com.rayclip.app\settings.json`; a página mostra
`Settings saved` ou `Saved with warnings: …` num toast, para avisos de registro
não passarem em branco.

### Preferences — offline, hover e temas

| Opção | Efeito |
| --- | --- |
| **Offline & private** | nenhuma requisição de rede: `run_link_for` retorna antes de qualquer fetch |
| **Fetch link previews online** | lê título/favicon/capa do site **só** de links copiados, com cache em disco |
| **Preview on hover** | com isto desligado, só clique/↑↓ movem o preview |
| **Theme** | Raycast Dark · Midnight · Nord · Rosé · Paper Light |
| **Accent colour** | 8 cores + campo hex livre; vale para seleção, badges e botões |
| **Translucent window** | fundo translúcido/desfocado (Mica + acrílico do Windows); avisa e oferece ligar os "efeitos de transparência" do Windows se estiverem desligados |

Tema e cor são variáveis CSS (`html[data-theme]` + `--accent`): trocar de tema
não re-renderiza a lista, e `applyTheme` roda sempre que `settings.json` muda
(evento `settings:changed`). O tema claro redefine a rampa `--fg-3…--fg-80`
para preto, então todos os hovers/bordas derivados continuam legíveis sem
nenhuma regra duplicada.

**Selecionar e copiar parte de um texto.** O painel de detalhes é
`user-select: text` (o `body` global é `none`, o que antes bloqueava o arrasto)
e o arrasto do mouse **não** arrasta a janela (a zona de drag fica só na barra
superior/rodapé/nav). Com texto selecionado, `Ctrl+C` copia **apenas o
fragmento** (o handler do app só intercepta `Ctrl+C` quando
`getSelection()` está vazio), e o fragmento entra no histórico como entrada
nova — o equivalente ao "notepad interno" do Raycast.

### Linha "Source" com ícone do app de origem

Cada entrada mostra **de qual app ela veio** (linha "Source" no painel de
informações), com o **ícone** desse app extraído via `IShellItemImageFactory`
(32 px, cacheado em `images/src-<nome>.png`). Dois truques:

- Quando quem copiou é o **próprio Rayclip** — inclusive o processo do WebView2
  (`msedgewebview2.exe` é filho nosso, detectado via Toolhelp `th32ParentProcessID`)
  — o Source vira **"Rayclip"** com o ícone do app, em vez de "msedgewebview2".
- Apps que copiam e fecham logo (ex.: `Set-Clipboard`) não deixam dono: cai no
  fallback **janela em primeiro plano** na hora da captura.
- Nomes amigáveis: `explorer` → "File Explorer", `WINWORD` → "Word", etc.

### "Save as…" agora vale para tudo (imagens, texto e arquivos)

O diálogo do Windows (`IFileSaveDialog`) aceita a extensão pedida e já mostra
o filtro certo ("Markdown (\*.md)", "Text file (\*.txt)", "Internet shortcut
(\*.url)"…). Entradas de imagem mantêm os 5 formatos; texto/links ganham um
submenu próprio (TXT, MD, JSON, CSV, HTML, RTF, URL, LUA, JS, PY…). O título do
diálogo e o nome sugerido foram ajustados (nome curto, sem cortar no meio).

### Salvar várias entradas de uma vez

Selecionando 2+ entradas (Ctrl/Shift + clique) o painel de ações ganha
**"Save N Items As…"**. A janela é **bem maior** (até ~86% da altura, lista
rolável com scrollbar fina) e tem um controle **"Apply to all"** que aplica um
formato para **todas as imagens** ou para **todo o texto** de uma vez ("Images
→ WEBP", "Text → MD", custom…). As entradas viram uma **lista com o formato de
cada uma do lado do nome**:

- imagens → `PNG` (padrão), `JPEG`, `WebP`, `BMP`, `GIF`
- texto/link → `TXT` (padrão), `MD`, `JSON`, `CSV`, `HTML`, `RTF`, `URL`, `LUA`,
  `JS`, `TS`, `PY`, `CSS`, `XML`, `YML`, `INI`, `LOG`, `SQL`, `BAT`, `PS1`
- **Custom…** → o usuário digita a extensão (`toml`, `lua`, `kml`…), o app
  normaliza (tira `.`, só alfanumérico, máx. 8) e grava o conteúdo certo para o
  tipo da entrada

Depois é só escolher a pasta (`IFileOpenDialog` + `FOS_PICKFOLDERS`); os nomes
saem do título da entrada já sanitizado, com sufixo ` 2`, ` 3`… quando repete.
Imagem GIF→GIF preserva os bytes originais (animação intacta) e texto em
`html`/`rtf` usa o formato rico guardado no clipboard quando existe.

Verificado: 3 entradas (imagem + texto + link) em `webp`, `lua` e `.url` —
`RIFF/WebP lossless 112 KB`, `537413528.lua` com o texto e
`https golive…-669386.url` com a URL; extensão inválida marca a linha em
vermelho e bloqueia o botão; cancelar no seletor devolve `null` e a janela
continua visível (o `dialog_open` também cobre esse diálogo).

### Banco cifrado (SQLCipher) — o histórico não fica legível no disco

Tudo o que vem do clipboard agora mora em **um banco SQLCipher**:
`%APPDATA%\com.rayclip.app\clipboard
ayclip.db`.

- **Cifra**: SQLCipher 4 (AES-256-CBC + HMAC-SHA512, PBKDF2-HMAC-SHA512 com
  256 000 iterações) compilado **do fonte** junto com o app — com o provider
  **libtomcrypt** (`sqlcipher-src` + `cc` no `build.rs`), então o build não
  precisa de OpenSSL nem de Perl no Windows.
- **Chave**: 32 bytes aleatórios (`BCryptGenRandom`) guardados em
  `rayclip.key` **selados com DPAPI** (`CryptProtectData`). Só o usuário do
  Windows que criou o arquivo abre a chave; copiar `rayclip.db` para outra
  conta/máquina não serve de nada. Se o DPAPI não abrir (outro perfil, backup
  restaurado), o app **não sobrescreve** a chave: o banco fica fechado em vez
  de ser trocado.
- **O que vai para o banco**: texto, HTML/RTF originais, OCR, metadados de
  link, e as **imagens e miniaturas como BLOB** (bytes PNG/GIF originais). Não
  sobrou nenhum arquivo de conteúdo em claro — o `store.json` e a pasta
  `images/` do formato antigo são **migrados na primeira abertura e apagados**.
- `PRAGMA secure_delete = ON`: o que é apagado (limite de histórico, "Clear
  History") é zerado nas páginas do banco em vez de ficar para trás no arquivo.
  Isso é o máximo que o SQLite promete: em SSD, por causa do *wear leveling* e
  do cache do controlador, **nenhum software consegue garantir** que os bytes
  físicos sumiram — a promessa honesta é "não há cópia em claro no arquivo nem
  no sistema de arquivos", não "destruição física do setor".
- A pasta `media/` guarda só mídia **derivada** (ícone do app de origem,
  favicon/capa de link), que não é conteúdo do clipboard.
- As imagens chegam à interface pelo protocolo próprio
  `http://rayclip.localhost/img/<id>` (e `/thumb/<id>`), servido direto do
  banco — nenhum PNG é escrito no disco para o preview.

Verificado nesta máquina: com 1.457 entradas (1.306 imagens, 390 MB de banco)
o arquivo não contém a assinatura `SQLite format 3`, nem `PNG`, nem
`IHDR`/`IDAT`, nem as URLs/textos do histórico (busca por `http`, `pichau`…:
zero ocorrências). Abrir o banco com a chave errada devolve erro de decifra,
como esperado.

### Importar o histórico do Raycast

**Settings → Data & Security → Import from Raycast** (o botão mostra quantos
itens existem).

O Raycast guarda o banco dele (`clipboard.db`) em SQLCipher com a chave em
`%LOCALAPPDATA%\Raycast\creds\BackendDBKey.dpapi` — nessa máquina o blob
DPAPI é **órfão** (a master key que ele referencia não existe mais no perfil),
então o `.db` é ilegível para qualquer processo, inclusive para nós. Mas o
Raycast também grava **cada item como arquivo** em
`%LOCALAPPDATA%\Raycast\clipboard\`: `.png` (prints), `.html` (CF_HTML do
texto rico), `.rtf` e `.txt`. É por aí que a importação funciona:

- arquivos do mesmo "copiar" (mesmo instante, formatos diferentes) viram **uma
  entrada só** — HTML + TXT + RTF juntos, com o texto extraído do fragmento
  CF_HTML (`<!--StartFragment-->` e decodificação de entidades);
- as imagens entram como BLOB no banco cifrado e os textos com `source =
  "Raycast"`;
- a **data original** (mtime do arquivo) é preservada, então o histórico
  aparece na ordem certa;
- itens repetidos do que já existe são ignorados;
- o limite de histórico é **aumentado automaticamente** (100 → 300 → 500 →
  1.000 → 2.000 → 5.000) para a importação não ser cortada;
- nada no Raycast é alterado (os arquivos dele ficam onde estão).

Verificado: **1.381 itens** (1.272 imagens + 105 textos, 1 repetido ignorado)
importados em **10,6 s**, resultando em 1.457 entradas; limite ajustado para
2.000 automaticamente; todas as miniaturas da lista carregam do banco
(`naturalWidth > 0`, zero imagens quebradas) e "Save as…"/copiar de uma imagem
importada devolve os bytes originais.

### Correção da v0.1.7 — clique dentro da janela

O hook de clique da v0.1.6 comparava o **PID** da janela sob o cursor com o
nosso. Dentro do app o alvo do clique é uma janela filha do WebView2 (processo
`msedgewebview2`), então o clique era tratado como "clique fora": a janela
fechava e nada era selecionado nem colado. Agora a comparação é pela **janela
raiz** (`GetAncestor(GA_ROOT)`), que é sempre a nossa. Verificado: clique numa
linha seleciona e atualiza o painel de detalhes; duplo clique cola e fecha.

### Novidades da v0.1.28 — senhas: o filtro testado de verdade

Não havia gerenciador de senha nesta máquina para testar o filtro, então ele foi
**simulado**: `_ref/fake_pm.rs` (Rust puro, sem dependências) cria uma janela
oculta para ser dono do clipboard, como um app real, e publica os marcadores que
os apps usam. Compilado com `rustc` e renomeado para `Bitwarden.exe`,
`1Password.exe`, `KeePassXC.exe`, `NordPass.exe`…

**13 cenários, 13 certos:** os cinco nomes de gerenciador não entram no
histórico; `bloco_de_notas.exe` entra; `ExcludeClipboardContentFromMonitorProcessing`,
`Clipboard Viewer Ignore` e `CanIncludeInClipboardHistory = 0` bloqueiam mesmo
vindo de um app comum; `CanIncludeInClipboardHistory = 1` não bloqueia; e com
"Also capture passwords & protected content" ligado o conteúdo do gerenciador é
guardado.

Dois "fracassos" iniciais eram **do simulador**, não do app: `SetClipboardData(fmt, NULL)`
é *rendering atrasado* e falha, então o marcador nem existia no clipboard. Do jeito
que os apps fazem (um DWORD 1), passou sem tocar no Rayclip.

Filtro ganhou `is_ignored_app()` (testável) e dois nomes genéricos — `"password"` e
`"vaultwarden"` — que pegam `WindowsPasswordManager.exe` e `MeuPasswordVault.exe`
sem pegar `notepad.exe`, `chrome.exe` ou `Code.exe`.

**Limite medido:** se o app copia **sem janela**, o Windows não informa quem
escreveu (`GetClipboardOwner` = NULL) e o nome não pode ser checado — nesse caso só
o marcador protege. Testado: `Bitwarden.exe` sem janela e sem marcador **entra**;
com o marcador, **não**. Na prática 1Password e Bitwarden fazem as duas coisas.

### Novidades da v0.1.27 — auditoria de colar, apagar e exportar

Segunda passada, agora nos comandos IPC. O que já estava certo (SQL sempre com
parâmetros `?1`, `open_url` restrito a http/https/mailto, `reveal_path` só para
caminho existente, nomes de arquivo saneados e sem traversal, `paste_many` com as
8 sequências ignoradas, log sem conteúdo, nenhum `dangerouslySetInnerHTML`) ficou
como estava. O que faltava:

- **Comandos só de teste desligados em release**: `save_image_to` e
  `save_many_to` escrevem em qualquer caminho sem diálogo — agora recusam fora da
  build de desenvolvimento.
- **A pasta de destino precisa existir** antes de "Salvar N itens" gravar nada.
- **O plano B por `curl.exe` não grava mais arquivo temporário**: a página ia
  para `%TEMP%
ayclip-fetch-*.out` em claro. Agora o corpo vem pelo stdout e o
  `Content-Type` por uma marca no fim (a marca é separada pela **última**
  ocorrência, porque o HTML pode contê-la).
- **A limpeza de `%TEMP%` cobre os três restos** (`rayclip-ocr-*`,
  `rayclip-fetch-*.out`, `rayclip-thumb-*`), inclusive os de builds antigas.

**Verificado:** 35 testes passando; `save_many_to` grava 3 arquivos com nomes
saneados e recusa pasta inexistente; `%TEMP%` sem nenhum `rayclip-*` depois de
rodar OCR, buscar links e salvar.

### Novidades da v0.1.26 — auditoria de segurança (falhar fechado)

Uma revisão externa do código apontou cinco coisas. Quatro eram de código e
foram corrigidas; a quinta é uma decisão, e ficou registrada como tal.

- **A chave nunca é "improvisada".** Se o `BCryptGenRandom` falhar, não existe
  chave — antes o código montava uma a partir de tempo + endereço de memória
  (previsível, e o próprio comentário dizia isso). Agora a função devolve
  `None`, o banco não abre e o app explica o motivo em **Configurações → Data &
  Security**. Melhor não guardar do que guardar mal.
- **A chave nunca é gravada em claro.** Se o `CryptProtectData` (DPAPI) falhar,
  antes os 32 bytes crus iam para `rayclip.key`. Agora não se grava nada —
  exceto uma chave **antiga** de build pré-0.1.26, que é aceita e
  **re-protegida na hora** (ninguém perde o histórico).
- **O OCR não toca mais o disco.** A imagem ia em claro para
  `%TEMP%
ayclip-ocr-*.png`. Agora vai por `InMemoryRandomAccessStream` +
  `BitmapDecoder`: reduzida e decodificada na memória. Zero arquivo.
- **A prévia de links não é mais uma porta para a sua rede.** Copiar
  `http://192.168.0.1/admin` ou `http://169.254.169.254/` fazia o app fazer a
  requisição. Agora só passa http/https para host que resolva **exclusivamente**
  para IP público (loopback, 10/8, 172.16/12, 192.168/16, link-local, CGNAT,
  IPv6 local, multicast e nomes de máquina sem domínio barrados), e cada salto
  de redirecionamento é revalidado (o `curl.exe` de plano B não segue nenhum).
- **A interface não pode mais ler o disco inteiro.** `csp` era `null` e o
  escopo do asset protocol era `**`. Agora há CSP (`script-src 'self'`,
  `object-src 'none'`, `frame-src 'none'`) e o asset protocol foi **desligado**:
  o ícone do app de origem vem de `http://rayclip.localhost/src/<nome>`, com o
  nome validado e só a pasta `media/`.
- **Promessa corrigida:** `secure_delete` zera as páginas do banco, mas em SSD
  (wear leveling) nenhum software garante apagar o setor físico. O README não
  promete mais isso.

**Verificado:** `cargo test --lib` 34/34; copiar um link interno não dispara
requisição (`destino nao publico bloqueado` no log) enquanto um link público
continua ganhando site + ícone; OCR reindexou 3 imagens sem deixar um único
`rayclip-ocr-*.png`; 0 violações de CSP com todas as imagens carregando; e
`/src/rayclip.db` responde 404.

### Novidades da v0.1.24 — atualização automática pelo GitHub

O Rayclip agora se atualiza sozinho pelas **releases do GitHub**, em
**Settings → Updates**:

- **Check now** — consulta a última release e mostra a versão, as notas e o
  tamanho do download.
- **Install update** — baixa o `rayclip.exe` novo, confere o **SHA-256**
  publicado na release, troca o arquivo no lugar e reabre na versão nova. Sem
  checksum, não instala.
- **Find updates automatically** (ligado por padrão) — procura na abertura e a
  cada 6 horas. **Nunca instala sozinho**: só aparece uma pílula discreta no
  rodapé (`Update 0.1.25`) que abre essa tela.
- Com **Offline & private** ligado, nada é consultado.

A troca é segura por construção: o exe em execução **nunca é sobrescrito** —
ele é renomeado para `rayclip.exe.old`, o download entra no lugar e o processo
novo espera o antigo sair antes de abrir o banco. Se o download for bloqueado
pelo antivírus ou o novo exe não subir, o `.old` volta a ser `rayclip.exe` e
você continua na versão que tinha. Histórico, ajustes e banco cifrado não são
tocados.

O código está em **<https://github.com/luigiarrud4/Rayclip>** e as releases em
**<https://github.com/luigiarrud4/Rayclip/releases>** (o repositório é público
porque o updater lê a API do GitHub sem token).

Para publicar uma versão nova existe o `deploy.bat`:

```bat
deploy.bat                 :: sobe o patch (0.1.24 -> 0.1.25) e publica
deploy.bat 0.2.0           :: publica essa versão
deploy.bat --build-only    :: só compila e calcula o hash
```

Ele grava a versão nos três arquivos (`package.json`, `tauri.conf.json`,
`Cargo.toml`), compila, calcula o SHA-256, comita, faz push e cria a release
com `rayclip.exe`, `rayclip.exe.sha256` e o instalador NSIS. Na primeira vez,
é preciso ter o repositório criado e o `gh` logado:

```bat
gh auth login
gh repo create luigiarrud4/Rayclip --public --source . --remote origin --push
```

### Novidades da v0.1.23 — o que ainda faltava do Raycast no painel de ações

Reli o Raycast (strings do `Raycast.dll` e os bundles JS do frontend) para
comparar a lista de ações dele com a nossa. Cinco coisas faltavam e entraram:

- **Reordenar fixados** — `Move Up in Pinned` / `Move Down in Pinned`
  (`Ctrl+Shift+↑` / `Ctrl+Shift+↓`). O recém-fixado vai para o topo, e a ordem
  escolhida é gravada no banco (coluna nova `pin_order`, migrada sozinha em
  bancos antigos).
- **Editar conteúdo** — `Edit Content…` (`Ctrl+Shift+E`) edita o **texto** da
  entrada sem mexer no título (o `Ctrl+E` continua sendo o renomear). O texto
  inteiro é carregado antes de abrir o editor, e `Ctrl+Enter` salva.
- **Copiar cor como…** — para entradas de cor: HEX, HEX + alpha, RGB, RGBA,
  HSL e HSV, com o valor já calculado na própria linha do menu.
- **Apagar entradas recentes…** — `Last 5 / 15 / 30 Minutes`, `Last Hour`,
  `Last 24 Hours`; apaga **só o que não está fixado** e sempre com confirmação
  (o foco começa em **Cancelar**).
- **Grupos de data iguais aos do Raycast** — `… This Month`, **`This Year`**,
  **`Older`**.

O que ficou de fora, de propósito: `Copy Text from QR Code` (precisa de um
decodificador de QR) e `Filters` (etiquetas automáticas por aplicativo, com
tela de configuração própria).

### Novidades da v0.1.22 — idioma da interface (inglês / português do Brasil)

A interface agora fala **inglês (padrão)** e **português do Brasil**. A escolha
fica em **Settings → Preferences → Language / Idioma** e vale na hora, sem
reiniciar.

- Traduzidos: barra de busca e filtros, grupos da lista (**Fixados, Hoje,
  Ontem…**), painel de detalhes (Origem, Tipo, Caracteres, Palavras, Linhas,
  Texto na imagem), painel de ações inteiro, "Salvar N itens", confirmação de
  exclusão, visualizador de imagem e a página de configurações completa
  (inclusive todos os textos de ajuda).
- Mensagens do rodapé também: *Copiado para o clipboard · Colado no app de onde
  você veio · 3 entradas apagadas …*
- **Padrão é inglês**: quem não mexer em nada vê o app igual ao Raycast. Se
  faltar alguma tradução nova no futuro, o texto aparece em inglês (nunca uma
  chave crua).
- Datas e horas seguem o idioma escolhido.

Conferido trocando o idioma pelo próprio dropdown: **inglês → português →
inglês**, com as telas principais verificadas em cada direção.

### Novidades da v0.1.21 — "Also capture passwords & protected content"

Em **Settings → Privacy & Indexing** há agora uma opção para **guardar também o
conteúdo que os sites e apps pedem para não guardar** — desligada por padrão.

- Cobra os três avisos que os programas usam para dizer "não registre isto"
  (`ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore`,
  `CanIncludeInClipboardHistory = 0`) e também a lista de gerenciadores de
  senha. É o caminho que cobre **gerenciador de senha** e **aba anônima/protegida
do navegador** — os dois usam esses mesmos marcadores.
- Desligada (padrão), o comportamento é o seguro: nada disso entra no
  histórico.
- Testado: com a opção desligada um texto marcado como sigiloso **não** é
guardado; ligada, aparece na lista; desligada de novo, volta a não guardar.

### Novidades da v0.1.20 — captura sem perder nada (e comparação com o Raycast)

Revisão do fluxo de clipboard + um harness que mede Rayclip e Raycast no mesmo
Windows, com o mesmo método.

- **Captura no instante da cópia.** O app escutava o clipboard em polling de
  320 ms: duas cópias seguidas dentro dessa janela e a **primeira sumia** para
  sempre. Agora o Windows avisa na hora (`AddClipboardFormatListener`), com o
  polling de 700 ms só como rede de segurança — e tudo é processado numa thread
de trabalho, fora da fila de mensagens.
- **Atalhos registrados um a um.** Com o Raycast aberto (que usa
  `Ctrl+Shift+V`), o registro em bloco falhava inteiro e o app ficava **sem
  atalho nenhum**, sem avisar. Agora um atalho indisponível só gera aviso; o
  outro continua funcionando.
- **Harness de comparação** (`_ref/harness_compare.py`): abre cada app pelo
  próprio atalho (`Alt+Space` / `Ctrl+Shift+V`), cola com `Enter` numa janela
  alvo e cronometra cada etapa.

| | Rayclip 0.1.20 | Raycast 2.13.1 |
|---|---|---|
| abrir (atalho → visível) | 14 ms | 19 ms |
| colar (`Enter` → texto no alvo) | 17 ms | 14 ms |
| capturar a cópia | ≤ 30 ms | ≤ 30 ms |

O harness (`_ref/harness_compare.py`) repete as três medidas a qualquer momento:
`python _ref/harness_compare.py`.

### Novidades da v0.1.19 — a prévia do link aparece de verdade

O que faltava não era o nome na lista (a linha continua mostrando o link) — era a
**prévia**: o cartão do link no painel aparecia como uma tira vazia, só com a URL
cortada.

- **Capa com candidatas**: `og:image` da página e, no YouTube, `maxresdefault` e
  depois `hqdefault` — a primeira que baixar vira a imagem do cartão.
- **Nunca mais cartão vazio**: sem capa (site sem `og:image`, arquivo de CDN) o
  topo do cartão mostra o favicon grande e o site sobre um degradê; se o arquivo
  da capa sumiu do cache, a imagem quebrada é substituída pelo mesmo bloco.
- **Re-tentativa de capa**: downloads recusados (429, site lento) são tentados de
  novo na sessão seguinte — até 3 por execução, sem gerar tráfego a cada abertura.
- O título da página continua no cartão (`Rick Astley - Never Gonna Give You
  Up`), agora acompanhado da miniatura do vídeo.

### Novidades da v0.1.18 — metadados de link que não falham calados

- **As linhas continuam mostrando o link** (a URL copiada) — a mudança de rótulo
  da 0.1.18 foi revertida na 0.1.19, que era sobre a **prévia**.
- **YouTube pelo oEmbed**: o título vem limpo (`ABSURDO! Eu trouxe o GTA 6 pro
  meu PC`), sem o sufixo " - YouTube", tanto para vídeo quanto para canal.
- **Sites que bloqueiam cliente não-navegador** (Cloudflare — a Pichau é uma):
  quando o cliente HTTP interno leva 403, a página é buscada de novo pelo
  `curl.exe` do próprio Windows, sem abrir console nenhum. Foi o que destravou
  Pichau, Shopee e afins.
- **Página de bloqueio não vira título**: "Just a moment...", "403 Forbidden",
  "Site em Manutenção" são descartados (a linha fica com a URL) e o app **não**
  fica re-tentando a cada abertura.
- Quando o buscador de metadados melhora, as entradas incompletas ganham **uma**
  nova tentativa (campo de versão no registro), em vez de ficarem presas no
  primeiro fracasso.

### Novidades da v0.1.17 — apagar várias entradas de uma vez

- **`Delete` com várias entradas marcadas** apaga todas — e pede confirmação:
  um card com o número, o aviso de que a exclusão é definitiva (fixados
  inclusive) e a **lista dos itens que vão embora**, para conferir antes. O foco
  começa em **Cancel**, `←`/`→` troca de botão e `Esc` sai.
- **No painel de ações** (`Ctrl+K`) apareceu **Delete N Entries**, entre
  "Save N Items As…" e "Clear Selection (N)".
- Com uma única entrada selecionada o `Delete` continua apagando na hora, sem
  diálogo: pedir confirmação a cada exclusão simples seria atrito.
- A exclusão em lote é um comando só no backend (uma travada do banco e uma
  listagem de volta), então apagar 200 itens não custa mais que apagar um.

### Novidades da v0.1.16 — dedup, ordem da busca e digitação

- **Copiar de novo não duplica:** o mesmo conteúdo vindo do navegador (com
  formatação) ou do editor (texto puro) é a **mesma** entrada — ela sobe para o
  topo mantendo o id, o pin, o OCR, os metadados do link e as imagens.
- **A busca ordena por relevância:** texto e links antes de imagens que só
  casaram pelo texto lido no OCR; dentro disso, fixados primeiro, depois
  casamento exato, início, começo de palavra e meio — e a recência desempata.
  Com busca ativa a lista vira um bloco único (sem os rótulos de data).
- **Primeiro resultado sempre selecionado:** dá para digitar e apertar `Enter`
  direto, mesmo que o item anterior tenha saído do filtro.
- Foco na busca com o texto selecionado; digitar substitui a busca anterior.

### Novidades da v0.1.15 — prévia e mãozinha de arrastar corrigidas

A v0.1.14 quebrou o enquadramento: a imagem da prévia crescia até o tamanho
natural e era cortada pelo rodapé, e no card grande ela também estourava a caixa.
As duas coisas eram porcentagem de `max-height` sendo ignorada (contra altura
automática, no painel; contra track automático de grid, no card). O enquadramento
voltou a ser do CSS de cada contexto e o `.zoomwrap` passou a ser **flex**, onde a
porcentagem resolve. O cursor do arrasto agora vira `grabbing` de verdade e
arrastar não seleciona a imagem.

### Novidades da v0.1.14 — zoom sem quebrar o scroll

O zoom da prévia agora **só é ativado quando você clica na imagem**: antes disso,
girar a rodinha rola a coluna de detalhes normalmente. Depois do clique, o scroll
dá zoom **para dentro e para fora** (de **25% a 800%** — antes parava em 100%), o
arrasto move a imagem, duplo clique volta ao normal e `Esc` sai do modo zoom. Uma
dica "scroll to zoom · Esc to exit" aparece no canto da imagem quando o zoom está
ativo, e clicar fora da imagem também sai dele.

O preview grande continua no botão **⤢** da imagem, no item **Open Preview
(Space)** do painel de ações e na tecla `Espaço`; lá dentro o scroll também
diminui a imagem abaixo de 100% e o canto redimensiona o card.

Corrigido de passagem: o `wheel` era registrado como passivo pelo React, então o
`preventDefault` era ignorado e o zoom "vazava" para a rolagem da coluna.

### Novidades da v0.1.13 — zoom na imagem e preview grande

A imagem copiada agora responde ao mouse: **scroll dá zoom** (até 800%, com a
porcentagem no canto) e **arrastar move** a imagem ampliada; **duplo clique**
volta a 100%. Um **clique simples** abre o preview grande — um card centrado de
720×349 que também tem scroll para zoom, arrasto, **canto para redimensionar** e
fecha com `Esc`/`Espaço`. Chega-se a ele pelo clique na prévia, pelo painel de
ações (**Open Preview — Space**) ou apertando `Espaço` (só quando o filtro está
vazio, para não roubar o espaço da busca).

### Novidades da v0.1.12 — prévia em lugar fixo, estilo Quick Look

O tooltip que seguia o mouse atrapalhava (caía sobre o cabeçalho e pulava de
lugar). Agora a prévia do "Save N items" fica **sempre no mesmo lugar**: uma
coluna fixa à direita da lista mostra a miniatura grande da linha sob o mouse (ou
o texto em monoespaçado). E `Espaço`, duplo clique ou clique no ícone abrem o
**card grande centralizado** — como o Quick Look do macOS: `Espaço`/`Esc` fecham,
`↑`/`↓` trocam a entrada com ele aberto e clicar fora fecha.

### Novidades da v0.1.11 — prévia no "Save N items"

Cada linha do diálogo de salvar agora mostra a **miniatura de verdade** (22 px,
a mesma `/thumb` da lista). Passar o mouse mostra um **tooltip maior** (240 px,
perto do ponteiro, invertendo quando estouraria a janela): imagem inteira — GIF
continua animando — ou o texto em monoespaçado com fade no fim. **Clicar no
ícone fixa a prévia** (não some ao mexer o mouse); `Esc` fecha primeiro a prévia
e só depois o diálogo, e rolar a lista também fecha.

### Novidades da v0.1.10 — colagem instantânea, sempre elevado, ícone do site, lista que não engasga

**Ícone do site:** todo link agora mostra o favicon do site (como o Raycast), inclusive
linhas que antes ficavam com o clipe genérico — o caso clássico era um CDN do Discord
(`cdn.discordapp.com/attachments/...`), que responde **403** no `/favicon.ico`. O
favicon é baixado **uma vez por host** (cache em `media/fav/<host>`) e servido pela
própria app; a busca tenta `host` → domínio registrável → `www`, em
`/favicon.ico`, `/favicon.png` e `/apple-touch-icon.png`, e se nenhum existir lê a
página inicial e usa o `<link rel="icon">` declarado (foi assim que o Discord
apareceu, 256×256). Sem rede/sem ícone, a linha mostra um tile com a inicial do
domínio — nunca imagem quebrada.

**Sempre elevado (de propósito):** o binário de release é compilado com
`requireAdministrator` — é o que permite colar no Gerenciador de Tarefas e em
consoles de administrador, sem o helper UIAccess que o Raycast usa. A tarefa
`RayclipElevated` (criada no primeiro start elevado) reabre o app **sem UAC**
por `schtasks /Run /TN RayclipElevated`, e o autostart dispara já elevado.

**Colagem instantânea:** um único `SendInput` (bloco atômico de Ctrl+V, antes eram
quatro `keybd_event` separados), **zero espera** quando o app de destino já voltou ao
primeiro plano e, quando não voltou, `AttachThreadInput` + sondagem de 1 ms com nova
tentativa. `AllowSetForegroundWindow` ao abrir deixa o Windows devolver o foco sem a
foreground lock, e uma trava garante uma colagem por vez. Medido no Bloco de Notas:
clipboard em **3,9 ms**, tecla em **+19 ms**; colagem múltipla sem as esperas fixas de
220+60 ms (segundo item injetado em **638 µs**).

**Lista que não engasga:** a linha pedia a **imagem inteira** do histórico
(1.305 imagens, 348 MB) sem cache; agora pede só a miniatura de 64 px, gerada na
primeira rolagem e cacheada. Capa de link e favicon deixaram de viajar como data URL
dentro do JSON: viraram arquivos (migração automática de 24 entradas antigas). A
listagem caiu de **3.014 KB para 1.727 KB** e o `list_entries` de **148 ms para
83 ms** (build debug); `system_status` passou a ter cache de 30 s (**125 ms → 1 ms**
por chamada). Abrir a janela mantém a **seleção e a rolagem** de onde você parou.

### Novidades da v0.1.9 — renomear entrada (`Ctrl+E`)

O título da entrada selecionada vira um campo de texto **inline** (como no
Raycast): `Enter` grava, `Esc` cancela, clicar fora grava, `Backspace` dentro do
campo edita o texto (nunca apaga a entrada). Vale para qualquer tipo — imagens
inclusive (o nome não é sobrescrito pelo OCR/miniatura) — e o novo nome entra na
busca. O atalho é configurável em Settings → Shortcuts ("Rename item") e a ação
está no painel de ações (`Ctrl+K` → **Rename Entry — Ctrl E**).

Dois perigos de UI tratados: com a janela escondida o `IsWindowVisible` mentia
(o Tauri mapeia a janela desde a criação) — agora a visibilidade é rastreada
pelo próprio app; e o modo "renomeando" nunca engasga o teclado da janela
inteira (trava é condicionada ao foco real no campo, e é limpa se o campo
sumir da lista virtualizada).

### Correções da v0.1.8 — auditoria + desempenho com o histórico real

Auditoria de riscos do projeto (o que estava solto e o que era perigoso) e
medição com as **1.457 entradas** importadas do Raycast:

- **Listagem pesada:** `list_entries` enviava `html`/`rtf` e até 30 MB de texto
  por item — **34 MB e 1,4 s** por chamada. Agora manda ≤ 16 KB de texto por
  item (**1,9 MB / 101 ms**) e o painel de detalhes pede o texto integral da
  entrada selecionada (`entry_text`).
- **Stack overflow (`0xc00000fd`):** `notify_changed` ficou em recursão infinita
  numa refatoração e o app **morria sozinho** durante a indexação de OCR.
  Corrigido; verificado com minutos de backfill.
- **`IsWindowVisible` mentia** (o Tauri mapeia a janela desde a criação, mesmo
  com `visible: false`): o app agora rastreia a própria visibilidade.
- **Nada de recarregar o histórico escondido:** eventos só saem com a janela na
  tela, e a UI junta 2–3 avisos da mesma mudança numa recarga só (debounce).
- **Banco à prova de apagão:** se a leitura do banco falhar, o app entra em modo
  degradado e `save()` nunca apaga linhas (antes a memória vazia apagaria tudo).
- **OCR repetido para sempre:** imagens sem texto eram re-indexadas a cada
  inicialização (702 threads disputando 2 vagas). Agora uma falha fica marcada e
  o backfill é uma thread sequencial que pausa com a janela aberta.
- **`open_url` executava comandos** (`cmd /C start` com texto do clipboard):
  agora só `http/https/mailto` via `ShellExecuteW`.

| Medida (1.457 itens) | antes | depois |
|---|---|---|
| `list_entries` | 1.436 ms | **101 ms** |
| JSON da resposta | 34.114 KB | **1.911 KB** |
| serialização no Rust | 583 ms | **36 ms** |
| busca (filtro na UI) | — | **2,1 ms** |

### Correções da v0.1.6 — sair da tela no instante do clique

O que atrasava o hide não era o hook, era o **caminho** do hide:

- `WebviewWindow::hide()` e `is_visible()` despacham para a thread principal do
  Tauri; logo depois de a janela abrir ela fica ocupada ~600 ms pintando o
  WebView, e todo pedido de esconder ficava na fila.
- Agora o hide é `ShowWindow(SW_HIDE)` direto no HWND (`win32::hide_now`, ~40 µs)
  e o `hide()` do Tauri só sincroniza o estado interno, numa thread separada.
  A visibilidade também é lida do Win32 (`IsWindowVisible`), sem round-trip.
- Novo **hook global de clique** (`WH_MOUSE_LL`): o app sai no momento do botão
  pressionado em outra janela, sem depender de o outro app processar a
  ativação (Discord demora a ganhar foco). Clique na barra de tarefas/bandeja
  até 450 ms após abrir é ignorado, para o duplo-clique no ícone da bandeja não
  fechar o que acabou de abrir.
- O pedido que chega junto com a abertura não é mais descartado: confirma ao
  vivo (`recheck`) em 160 ms.

Medido: abrir e clicar em outro app **32–40 ms depois** → escondida em ~10 ms
(antes: só saía quando o alcançava o watchdog, ~500–600 ms). Enter para colar:
**48 ms** até o `Ctrl+V` chegar no app anterior.

### Correções da v0.1.5

- **Backspace não apaga mais nada.** Os atalhos de apagar eram
  `Ctrl+Backspace`/`Ctrl+Shift+Backspace` — quem digitava no campo de busca e
  fazia `Ctrl+A` + `Backspace` para limpar o texto apagava uma entrada do
  histórico. Agora apagar entrada é `Delete` / `Ctrl+Shift+Delete` (atalhos
  antigos salvos são migrados sozinhos), e `Backspace`/`Delete` **nunca** agem
  no histórico quando o cursor está num campo de texto.
- **Esconder ficou instantâneo.** O hook de `SetWinEventHook` virou o caminho
  primário; o cursor sobre a janela não segura mais o app na tela (caso do Alt+Tab);
  console que pisca (processos auxiliares) é confirmado em 140 ms em vez de
  segurar o app; o watchdog de emergência caiu de 900 ms para 300 ms de
  carência. Medido: clique em outro app → escondida em ~7 ms (antes ~620 ms).
- **Colar ficou rápido.** A cópia não reconcilia mais as ~1.500 entradas no
  banco a cada operação (`save()` → `touch()`, uma linha por vez — copiar um
  texto passou de ~50 ms para ~4 ms); o `past_into` espera o alvo voltar ao
  primeiro plano em vez de dormir 80 ms fixos. Medido: Enter → `Ctrl+V` no app
  anterior em **29 ms** (antes ~400 ms com os `sleep`s).

### Como rodar

```bash
npm install
npm run tauri dev        # desenvolvimento (janela aparece via Alt+Space)
npm run tauri build      # instalador NSIS em src-tauri/target/release/bundle/
```

Binário debug já compilado: `src-tauri/target/debug/rayclip.exe`.

### Limitações

- Imagens são lidas via `CF_BITMAP`; alguns apps expõem apenas `CF_DIB`/PNG
  (fallback razoável: aparece como sem prévia ou texto).
- OCR depende dos pacotes de idioma instalados no Windows (usa os do perfil do
  usuário); imagens acima de 2400 px são reduzidas antes de reconhecer.
- "Paste as Plain Text" usa o conteúdo de texto quando disponível.
- Detecção de origem usa `GetClipboardOwner()` → **caminho do processo** (nome
  amigável + ícone). Se o dono já morreu, cai na janela em primeiro plano.
- O histórico agora é um banco cifrado (v0.1.4+): versões antigas não leem o
  `rayclip.db` e vice-versa. Quem tiver um `store.json` antigo tem a migração
  automática na primeira abertura.
- O fundo translúcido depende dos **efeitos de transparência do Windows**
  (Personalização → Cores): com isso desligado o Mica/acrílico não desenha e a
  opção fica inativa (o app avisa e oferece ligar com um clique).

## Testes executados

| Teste | Resultado |
|---|---|
| Captura de texto/link/email/cor/imagem/arquivo | 7 tipos no store |
| OCR de imagem nova (`ZEBRAFISH 4711…`) | texto reconhecido em ~2 s |
| Backfill de OCR no start | imagem antiga indexada |
| Busca por texto dentro da imagem (`zebrafish`, `jellyfin`, `128.45`) | 1 resultado + selo Aa |
| Persistência do OCR após reiniciar | mantido, sem re-indexar |
| Busca sem resultados | "No results for ‘zzzq’" |
| Hover do mouse pelas 6 linhas do submenu "Save as" | submenu continua aberto (antes voltava para "Paste/Copy…") |
| Hover em "Save as WebP" + Enter | diálogo abre com `*.webp` / "WebP image" |
| Hover em "Save as JPEG" + Enter (build release) | diálogo abre com `*.jpg;*.jpeg` / "JPEG image" |
| Mouse mexendo 40 passos por cima do submenu | janela continua visível e em primeiro plano |
| Clique numa linha / duplo clique (após o hook de mouse) | seleciona e cola normalmente |
| Clicar em outro app 32–40 ms depois de abrir | escondida em ~10 ms (antes ~500–600 ms) |
| Trocar para outro app (clique no Discord → janela escondida) | **~7 ms** (hook; 107 ms medidos incl. 100 ms do script) |
| Clicar no desktop | esconde em ~54 ms |
| Clicar num console/terminal (sem o confirmador) | ~141 ms (confirma que não é "flash") |
| `reg query` piscando com cursor dentro da janela | janela continua visível |
| Alt+Tab com o cursor sobre a janela | esconde na hora (cursor não segura mais o app) |
| Tray: ícone criado | janela `tray_icon_app` no processo, log `tray: criado` |
| Duplo clique com o app aberto | 1 processo só, `show_window` na instância viva |
| Startup como admin | tarefa `\Rayclip` com `RunLevel=HighestAvailable`; `system_status.startupTask=true` |
| Desligar startup | `schtasks /Delete` → `startupTask=false`, sem chave Run e sem `.lnk` |
| Save N items (3 tipos) | WebP 112 KB válido + `.lua` com o texto + `.url` com o link |
| Save com extensão custom | `toml` na linha, estado inválido some, arquivo com a extensão digitada |
| Save N items — cancelar | `null`, log `save_many: cancelado pelo usuário`, janela visível |
| Tema Paper Light | `data-theme=light`, `--accent=#2563eb`, fundo claro, texto legível |
| Tema volta ao Raycast Dark | `data-theme=dark`, `--accent=#4fa3f8` |
| Hover preview ON | passar o mouse troca a seleção |
| Hover preview OFF | passar o mouse **não** troca (clique e ↑↓ continuam) |
| Offline mode | `link_entry` não gera nenhuma linha `link: buscando` no log |
| Selecionar texto no preview | `user-select: text`, `getSelection()` pega o trecho |
| Ctrl+C com texto selecionado | clipboard do Windows recebe **só** o fragmento selecionado |
| 15× `system_status` (15 `reg query`) com sampler de janelas | **0** janelas de console criadas |
| Mouse passeando 2,5 s dentro da janela | continua visível e em primeiro plano |
| Clique no ícone de configurações (build release) | abre e continua em primeiro plano |
| Clique fora com o cursor fora da janela | esconde (`blur: hide`) |
| Abrir configurações com clique real no ícone | janela continua visível e em primeiro plano |
| Abrir painel de ações com botão direito | janela continua visível (`panel: fg=<nosso pid>`) |
| Clicar na aba "History" e abrir o dropdown | menu abre, janela continua em primeiro plano |
| Trocar "Maximum entries" pelo teclado no menu | valor salvo (`settings saved`), janela viva |
| "Salvar como" aberto (diálogo nativo) | app **não** esconde com outro app em primeiro plano (trava `dialog_open`) |
| Selecionar texto com o mouse (arrasto real) | `getSelection()` = "formation", janela não se move |
| Faixa de arrasto do topo | 16 px (medido no DOM) |
| Texto longo no painel de detalhes | scrollHeight 1572 vs 225 visível, sem máscara/fade, rolagem ok |
| Clique simples numa entrada | só seleciona (janela continua aberta) |
| Duplo clique numa entrada | `paste_entry` executado no log |
| Ctrl+clique em 3 entradas | 3 badges numerados 1-2-3 e lista "Selected · 3 items" |
| Shift+clique | intervalo de 3 linhas marcado na ordem |
| Enter com 3 marcadas | log `paste_many n=3` (todas coladas em ordem) |
| Esc com seleção ativa | limpa a seleção, janela continua visível |
| Salvar imagem como PNG/JPEG/WebP/BMP | 4 arquivos válidos, 354×294, formatos corretos (PIL) |
| Salvar GIF a partir de GIF | 6290 bytes originais, 4 frames (animação preservada) |
| Salvar GIF a partir de PNG | 1 frame, 354×294 |
| Diálogo "Save image as" | abre com `Rayclip 2026-11-20 01-35-09.png` e filtro PNG |
| Cancelar o diálogo | toast "Save cancelled", nada gravado |
| Confirmar o diálogo | arquivo gravado + log `save_image_as <id> -> <path>` |
| Pin (`Ctrl+P`) | grupo Pinned criado |
| Delete / Clear all but pinned | store fica só com o fixado |
| Botão direito numa linha | painel de ações da entrada |
| Cantos arredondados | DWM, sem artefato |
| Paste real de texto em janela alvo | texto colado (conferido no conteúdo do campo) |
| Paste real de imagem no Paint | imagem correta colada (ver `docs/screenshots/paste-image-paint.png`) |
| Formatos no clipboard após copiar imagem | `CF_DIB`, `CF_DIBV5`, `CF_BITMAP`, `PNG` |
| Clipboard marcado como sensível | NÃO entra no histórico (verificado) |
| Copiar com imagem apagada do disco | bolinha **vermelha** + mensagem do erro |
| Copiar com sucesso | bolinha **verde** brilhando + "Copied to clipboard" |
| Arrastar pelo topo (sobre o campo de busca) | janela move (testado: +124,+76) |
| Tamanho da janela | 900×570 por padrão, redimensionável |
| Link copiado (GitHub) | favicon na lista + card com capa, título e descrição |
| GIF no clipboard (`GIF` format) | salvo como `.gif`, **animando** (2 frames diferem) |
| Recopiar link já indexado | mantém metadados (não volta para vazio) |
| Miniatura de vídeo (.mp4) | frame real do vídeo na lista e no painel |
| Miniatura de PDF | capa do PDF na lista e no painel |
| Miniatura YouTube | `i.ytimg.com/vi/<id>/hqdefault.jpg` como capa |
| Lista com 3.526 entradas | só 26–33 linhas no DOM (virtualização) |
| Arrastar dentro do campo de busca | **seleciona texto**, janela não move |
| Arrastar pela faixa do topo (6 px) | janela move (testado: +80 px) |
| Clicar fora da janela | esconde em <0.4 s (`blur: hide` no log) |
| Clicar fora **logo após abrir** (316 ms) | esconde assim que o foco assenta |
| Clicar dentro do painel de detalhes | janela **continua** visível |
| Vigia de foco com o handler de blur desligado | esconde igual (`focus-watch: hide`) |
| Abrir configurações pelo ícone do rodapé | página abre (ver `docs/screenshots/settings-shortcuts.png`) |
| `Ctrl+,` abre/fecha as configurações | alterna a página |
| Ação "Settings…" no painel de ações | painel fecha e a página abre |
| Trocar atalho interno para `Ctrl+Shift+A` | painel de ações abre com a nova tecla |
| Trocar atalho pelo gravador | não dispara a ação antiga; keycaps do painel atualizam |
| Dropdown de "Maximum entries" (antes `<select>` nativo branco/ilegível) | menu escuro próprio, 6 itens legíveis (`docs/screenshots/settings-dropdown.png`) |
| Escolher item no dropdown | valor aplicado (`maxEntries: 500`, `retentionDays: 7`) |
| Setas + Enter no dropdown | muda a seleção e aplica |
| Esc com o dropdown aberto | fecha só o menu, a página de configurações continua |
| Trocar atalho global | re-registro sem reiniciar (`atalhos registrados` no log) |
| Ativar "Take over Win+V" | `DisabledHotkeys = V` gravado no registro |
| Desativar "Take over Win+V" | valor `DisabledHotkeys` **removido** |
| Iniciar com o Windows (chave `Run` bloqueada) | fallback criou `Startup\Rayclip.lnk` |
| Desativar "Start with Windows" | `.lnk` removido |
| Retenção/limite de histórico | `store.max_entries` + `purge_older_than` aplicados ao salvar |
| "Start with Windows" com `reg` negado | toast "Saved with warnings: …" (erro visível) |
| Painel de detalhes | rola (barra visível) |
| Ctrl+Enter | cola e mantém a janela aberta (multi-paste) |
| Ctrl+C no preview (processo `msedgewebview2`, filho do app) | Source = **"Rayclip"** + ícone do app |
| Cópia de app externo (janela WinForms do PowerShell) | Source = "PowerShell" + `images/src-powershell.png` (32 px) |
| Ícone carregado no painel | `img.complete = true`, `naturalWidth = 32` |
| "Save as…" em texto (`format: md`) | diálogo "Save as" com filtro "Markdown (*.md)" e nome curto |
| "Apply to all" → Images → WEBP (seleção mista de 22) | só as 2 imagens viraram `.webp`, texto ficou `.txt` |
| Janela "Save 22 items" | 780 × 421 (antes 540 de largura), lista rola (`scrollHeight > clientHeight`) |
| Transparência do Windows ligada (teste temporário) | Mica aparece: wallpaper desfocado atrás do painel |
| Transparência do Windows desligada (padrão da máquina) | `data-translucency="off"`, fundo sólido `rgb(10,10,12)` |
| Transições de hover | `background-color 130ms cubic-bezier(.2,0,.2,1)` na linha |
| Banco cifrado criado na 1ª abertura | `rayclip.db` sem header `SQLite format 3`, 0 ocorrências de `PNG`/`IHDR`/`http`/texto do histórico |
| Chave errada no banco | `Db::open` devolve erro de decifra (`error decrypting page 1`) |
| Migração do formato antigo | 80 entradas (34 imagens) migradas; `store.json` e `images/` apagados; 1.457 entradas depois do import |
| Importação do Raycast | 1.381 itens em 10,6 s (1.272 imagens, 105 textos), limite → 2.000, 1 repetido ignorado |
| Imagens pelo protocolo `rayclip://` | 22 linhas visíveis, 22 `<img>` com `naturalWidth > 0`, 0 quebradas; capa grande 976×721 |
| Copiar/salvar imagem vinda do banco | `copy_entry` publica DIB/DIBV5/Bitmap/PNG; `save_image_to` gera PNG de 202 KB |

## 3. Projeto

```
src/                 UI React (App, NavBar, EntryList, DetailsPane, ActionBar, ActionPanel)
src-tauri/src/       Rust: lib.rs (comandos/hotkeys/observer), capture.rs (leitura do clipboard),
                     store.rs (persistência), win32.rs (janela/foco/teclado)
docs/screenshots/    capturas do app + referências do Raycast (Screenshot_14/15)
_ref/                scripts de extração dos tokens e de teste automatizado
```