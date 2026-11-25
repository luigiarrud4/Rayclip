# Plano de continuação (VM de testes)

Estado no momento em que este plano foi escrito:

- versão publicada: **v0.1.39** (<https://github.com/luigiarrud4/Rayclip/releases/tag/v0.1.39>)
- instalador 3.791.598 B, `rayclip.exe` 11.049.984 B, sha256 `8e80fdbf…`
- `cargo test --lib`: **47/47**; `npx tsc --noEmit`: 0 erros;
- CDP não precisa mais de flag no `tauri.conf.json`: `RAYCLIP_CDP=1` (§8.51).

---

## 1. Objetivo desta rodada

> **Feito nesta máquina (24/11):** toolchain completo instalado (VS 2022 Build
> Tools + MSVC 14.44, Windows SDK 10.0.26100, Rust 1.100, Node 26, Python 3.12 +
> `websocket-client`), `npm install`, build debug OK, **arrasto de arquivo
grande resolvido** pelo §4.1 (`tauri-plugin-drag`) — medido até **120 MB** —,
> CDP sob demanda (§8.51), nome de arquivo em hora local e OCR honesto (§8.52),
> `cargo test --lib` **47/47**. Falta o release (item 6 do §4.0) — e `git`/`gh`
> **não existem** nesta máquina. Pendência de sistema: esta máquina **não tem** o
> componente de OCR do Windows (`Windows.Media.Ocr.dll` ausente do System32), então
> o OCR do app não pode ser medido aqui.

Fechar o **arrasto de arquivos grandes e de `.exe` para fora do app** e, de quebra,
medir o que a VM permite medir (elevação, UAC, autostart, multi-monitor, teclado).

O problema aberto, medido:

| Entrada | Resultado hoje |
| --- | --- |
| imagem do histórico (497 KB) → pasta | funciona (bytes idênticos) |
| `.zip` de 3 MB → pasta | funciona |
| `.zip` de 25 MB → pasta | **nada é criado** |
| `.exe` de 25 MB → pasta | **nada é criado** |

Ou seja: **não é o tipo do arquivo, é o tamanho**. O `DownloadURL` do Chromium (o
único caminho do WebView2 que vira `CF_HDROP` no Explorer) para de funcionar entre
3 e 25 MB. O caminho correto para arquivos que já estão no disco é OLE
(`SHDoDragDrop` + `IDataObject` com `CF_HDROP` apontando para o **caminho**): não
copia bytes, não tem teto de tamanho e não passa pela memória do app.

---

## 2. O que já foi tentado no OLE (não repetir do zero)

> **Atualização da revisão:** antes de tentar o OLE de novo, ver §4.1 — existe
> crate maduro que faz exatamente isso (`tauri-plugin-drag`). O histórico abaixo
> continua valendo como lista do que **não** repetir à mão.

Feito na máquina do usuário, sem sucesso:

1. `SHDoDragDrop` numa **thread própria** com janela oculta criada por
   `CreateWindowExW(WS_POPUP)`.
2. Janela oculta **visível** (`ShowWindow(SW_SHOWNOACTIVATE)`, fora da tela, com
   `WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE`) porque `SetCapture` falha em janela invisível.
3. `IDropSource` implementado com vtable na mão (`QueryInterface`/`AddRef`/`Release`
   + `QueryContinueDrag` retornando `DRAGDROP_S_DROP` quando o botão esquerdo está
   solto e `DRAGDROP_S_CANCEL` no Esc).
4. `IDataObject` de `SHCreateDataObject` com PIDLs de `SHParseDisplayName`.
5. Watchdog de 60 s soltando `ARRASTANDO` (funcionou; provou que o laço não volta).

Resultado: `dragout: iniciando 1 arquivo(s)` e **nunca** `terminou`; e — a pista
principal — **nenhuma** chamada a `QueryContinueDrag` (contador com log nunca
apareceu). Portanto o bloqueio é **antes** do laço: quase certamente no
`IDropTarget::DragEnter` da **própria janela do Rayclip** (o cursor está sobre a
lista no instante em que o arrasto começa) ou no `OleInitialize` da thread nova.

O código do `dragout.rs` foi removido do repositório de propósito (não deixa laço
travado no build do usuário); este plano descreve como retomá-lo.

---

## 3. Preparar a VM (30–40 min)

1. Windows 10/11 x64, conta **administrador**.
2. Instalar, nesta ordem:
   - **VS 2022 Build Tools** com "Desktop development with C++" (precisa de `cl.exe`);
   - **Rust** (`rustup`, toolchain `stable-x86_64-pc-windows-msvc`);
   - **Node.js 20+**;
   - **WebView2 Runtime** (checar `C:\Program Files (x86)\Microsoft\EdgeWebView`);
   - **Python 3** e `pip install websocket-client`;
   - **NSIS** não precisa: o Tauri baixa sozinho.
3. Conferir que **não** há OpenSSL/Perl no caminho do build (o build não usa).
4. Descompactar `rayclip-projeto.zip`, rodar `npm install`.
5. `npx tauri build --debug --no-bundle` (≈60 s) e confirmar que o app abre com
   `src-tauri\target\debug\rayclip.exe --open`.
6. Para os testes automatizados **não precisa mais mexer no
   `tauri.conf.json`**: o app liga o DevTools só quando existe `RAYCLIP_CDP`
   (`1` = porta 9222, ou o número da porta), e sempre em loopback.

   ```powershell
   $env:RAYCLIP_CDP = "1"; src-tauri\target\debug\rayclip.exe --open
   ```

   Os scripts de `_ref/` já passam a variável (`import cdp_env`) e o
   `_ref/reiniciar.ps1 -Cdp` faz o mesmo para teste manual. O release sai do
   **mesmo** binário, sem porta aberta — `grep -c remote-debugging
   src-tauri/tauri.conf.json` responde 0. Detalhe que custou uma medição: a
   variável de ambiente do WebView2 (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`)
   **não** funciona aqui, porque o wry sempre define esses argumentos sozinho;
   o jeito é alterar a config em memória (`aplicar_cdp` no `lib.rs`).

---

## 4. Ordem de trabalho na VM

### 4.0 Pendências pequenas da rodada anterior (do relato do agente)

| # | Pendência | Estado |
| --- | --- | --- |
| 1 | `monta_payload` (~l. 235-351) manda arquivo de disco por bytes/base64 até o `DownloadURL` | **feito**: `drag_files` + `tauri-plugin-drag` (§4.1). O log não mostra mais `drag payload` para arquivo |
| 2 | `MAX_ARRASTO` descer para o teto real medido do `DownloadURL` + aviso visível | **não é mais necessário**: o teto do `DownloadURL` deixou de valer para arquivo. `MAX_ARRASTO` (128 MB) só limita o caminho de texto/imagem que ainda usa o Chromium |
| 3 | Entrada de texto que **é** caminho de arquivo não anuncia `text/plain` (`EntryList.tsx` ~l. 179, `monta_payload` ~l. 300) | **decidido: fica como está** — só o arquivo viaja. Nada de `text/plain` com o caminho, para não trocar anexo por texto no Discord/Word |
| 4 | Log de diagnóstico `hook diag: pt=(…)` (`lib.rs` ~l. 2666) | **feito**: removido. Os argumentos do `format!` chamavam `cursor_point`, `class_of_hwnd`, `foreground_hwnd` e `window_rect` a **cada clique de qualquer janela**, e rodavam também em release, onde `log` descarta a mensagem |
| 5 | `DOCUMENTACAO.md` §8.50 | **feito**: §8.50 com a causa (teto do `DownloadURL`), o que resolveu, a tabela medida e os erros do harness |
| 6 | Release 0.1.39 (bump, build, `/S`, `reiniciar.ps1`, limpar entradas de teste, commit/push/`gh release`) | pendente: ver §4.4 |

Correções de harness já commitadas pelo agente anterior (não desfazer): Explorer
vinha minimizado em `(-32000,-32000)`; Explorer ficava debaixo da janela do app e
o arrasto soltava sobre o Rayclip; `SetWindowPos` tinha que ser refeito depois de
medir a posição.

### 4.1 Destravar o arrasto: adotar `tauri-plugin-drag` — **FEITO**

Resultado medido (build debug, Explorer à direita do app): `.zip` 3 MB,
`.zip` 25 MB, `.exe` 25 MB e imagem do banco → pasta criam o arquivo com nome,
extensão e bytes certos; texto → Notepad continua igual; janela não esconde
durante o gesto; clique e `Alt+Space` funcionam depois. Gesto apressado (sem os
250 ms de hover) também passa. Testes: `_ref/teste_arrasto_ole.py`
(`ALVO_TXT`, `ALVO_NOME`, `ALVO_BYTES`, `SEM_HOVER=1`), `_ref/teste_notepad.py`,
`_ref/teste_pos_arrasto.py`. O texto abaixo fica como registro do plano.

O §2 descreve cinco tentativas de OLE à mão, todas travadas. Antes da sexta, o
caminho pronto: o **`tauri-plugin-drag` 2.1.1** (CrabNebula, ~90 mil downloads no
último ciclo, atualizado em maio/2026) faz exatamente o desenho que este plano
tenta reproduzir — `DoDragDrop` + `CF_HDROP` apontando para o **caminho**, sem
copiar bytes e sem teto de tamanho — e já roda em outros apps Tauri 2.

- Papel do Rust (`tauri-plugin-drag`) + NPM (`@crabnebula/tauri-plugin-drag`).
- API: `startDrag({ item: ["C:\\...\\arq.zip"], icon }, cb)` — `item` são
  **caminhos que já existem no disco**.
- Permissão em `src-tauri/capabilities/default.json`: `"drag:default"`.
- Registro em `lib.rs`: `.plugin(tauri_plugin_drag::init())`.

Passos:

1. `cargo add tauri-plugin-drag`, `npm add @crabnebula/tauri-plugin-drag`,
   registrar o plugin e a permissão. Build debug e confirmar que o app sobe
   **antes** de mexer em qualquer arrasto (muda o `Cargo.lock`).
2. No frontend, trocar o caminho `DownloadURL` por `startDrag` **quando a entrada
   tem caminho em disco** (`entry.paths[0]` de arquivo existente). Sem arquivo, o
   `startDrag` não tem o que arrastar.
3. Imagens/GIF (que não estão no disco): escrever em
   `%LOCALAPPDATA%\com.rayclip.app\drag\<id>.<ext>`, uma vez, com cache por id, e
   arrastar **esse caminho**. Custo: um arquivo a mais no disco do app — aceitar e
   documentar; é o preço de não ter teto de tamanho.
4. Medir: `.zip` de 3/25/120 MB e `.exe` de 25 MB para uma pasta do Explorer,
   cronometrando até o arquivo aparecer e comparando o `sha256`; conferir que
   `%TEMP%` não ganhou arquivo e que a janela não congela
   (`SendMessageTimeout` + `IsHungAppWindow`, como no §8.49).
5. Regressões: texto → Notepad/Word precisa continuar funcionando. Se o
   `startDrag` só aceitar arquivo, manter os dois caminhos — `DownloadURL` para
   texto puro (`text/plain`/`text/uri-list`) e `startDrag` quando houver arquivo.
6. Armadilhas conhecidas do plugin, para não perder tempo:
   - **caminho UNC/rede derruba o app** (bug aberto, drag-rs #72): passar só
     caminho local; se `paths[0]` começar com `\\`, cair no `DownloadURL` ou
     avisar;
   - `icon` vazio (`""`) é aceito, mas o arrasto fica "vazio" no Explorer — vale
     mandar a miniatura quando existir;
   - o comando é assíncrono por IPC: o gesto tem que continuar com o botão
     apertado. Se o arrasto não nascer, chamar no `mousedown`/limiar de 4 px em
     vez do `dragstart`.

**Plano B se o plugin também travar (não perder mais que ~1 h):** o §2 continua
valendo para uma sexta tentativa manual de OLE, com o que faltou instrumentar da
primeira vez — `GetCapture()` e `GetAsyncKeyState(VK_LBUTTON)` **antes** do
`SHDoDragDrop`, e o id da thread dona da captura. A hipótese que o §2 não testou
é a mais provável: `SetCapture` chamado de uma thread que **não** é a dona da
janela sob o cursor (a do WebView2) não captura nada, e sem captura não chega
mensagem de mouse — daí **nenhum** `QueryContinueDrag`, que é exatamente o
sintoma medido. Se for isso, o OLE tem que nascer na thread do WebView2, e a
pergunta vira "como pedir isso ao Chromium", não "como implementar o
`IDropSource`".

**Plano C (último recurso):** manter `DownloadURL` até um teto medido
(8/12/16 MB) e mostrar um aviso curto ("arquivo grande demais para arrastar — use
*Save as…*") em vez de não fazer nada em silêncio.

**Critério de aceite (igual ao do plano):** arrastar o `.exe` de 25 MB para uma
pasta do Explorer cria o arquivo com o **nome e a extensão certos**, em menos de
1 s, sem congelar a janela e sem copiar os bytes para o `%TEMP%`.

### 4.2 Retomar o resto do arrasto (o que falta medir/fechar)

1. Texto → Notepad/Word: **funciona**; Discord anexa arquivo (limitação do app
   deles). Só documentar.
2. GIF animado → pasta: funciona (medido pelo usuário).
3. Regressões do arrasto: a janela não pode fechar durante nem depois; clique
   seguinte ainda seleciona; `Alt+Space` funciona depois de arrastar.
4. `ARRASTANDO` no Rust + `arrastandoAgora()` no frontend continuam válidos e
   cobertos pelo watchdog.

### 4.3 O que só a VM pode testar

1. **Elevação/UAC**: instalar com `/S`, conferir `requireAdministrator`, a tarefa
   agendada (`schtasks /Query /TN RayclipElevated`), e o app reabrindo elevado.
2. **Autostart** raiz (`HKCU|HKLM …\Run`) com e sem elevação.
3. **Multi-monitor** e DPI 125/150%: posição restaurada, captura do `Alt+Space`.
4. **`Win+V`** tomado pelo app e devolvido ao desinstalar.
5. **Mica/transparência** com "efeitos de transparência" desligados.
6. **Moldura nativa** num Windows onde ela reaparece (o caminho DWM já está
   implementado; falta ver em outra máquina).

### 4.4 Fechar e publicar

1. `cargo test --lib` (43/43 ou mais, com testes novos do caminho do arrasto).
2. `_ref/bump_version.ps1 -V 0.1.39`.
3. `npx tauri build` → instalar `/S` → `_ref/reiniciar.ps1` (que agora já mata o
   `msedgewebview2` órfão do user-data-dir).
4. Limpar da lista as entradas de teste (`TESTE ARRASTO TEXTO 987654`,
   `teste_3mb.zip`, `teste_25mb.zip`).
5. `DOCUMENTACAO.md` §8.50 com: teto do `DownloadURL`, o travamento do
   `SHDoDragDrop` no `DragEnter`, e o que resolveu.
6. `git commit` + `git push` + `gh release create v0.1.39` com `rayclip.exe`,
   `rayclip.exe.sha256` e o instalador.

---

## 5. Regras que continuam valendo

- Depois de **todo** build: fechar e reabrir (`_ref/reiniciar.ps1`), matando
  `msedgewebview2` órfão antes.
- Teste longo/flaky → em background, saída em arquivo, e **conferir o arquivo**.
- Entrada sintética só por `_ref/entrada.py` e só depois de confirmar o foco
  (`_ref/keysafe.py`).
- Nunca `Stop-Process` no meio de um arrasto OLE sem um watchdog que solte
  `ARRASTANDO`.
- Release sai sem porta de depuração: o CDP só existe com `RAYCLIP_CDP`, nunca
  fixo no `tauri.conf.json`.
- Nenhuma otimização entra sem ganho medido no mesmo build.

---

## 6. O que eu preciso do usuário

1. Acesso à VM (RDP: usuário/senha, ou agente já rodando).
2. Confirmar que é uma **VM descartável** (posso instalar Rust/Node/Python e
   reiniciar).
3. Se possível, uma segunda resolução de tela/monitor para o item 4.3.

---

## 7. Situação em 24/11 (fim do dia) — o que já está fechado

- **Arrasto para fora**: resolvido por `tauri-plugin-drag` (§8.50 e §8.53).
  Medido nesta máquina com `.exe` de 25.752.442 B → arquivo idêntico na pasta;
  na VM, até **120 MB**. GIF, imagem e texto conferidos pelo usuário.
- **Temas**: 0 textos abaixo de 3.0 de contraste nos 5 esquemas (§8.49).
- **Moldura nativa**: removida por `DWMWA_NCRENDERING_POLICY` (§8.49c).
- **Nome de arquivo em hora local** e **OCR honesto** (§8.52).
- **DevTools sob demanda** (§8.51) — acabou o passo de tirar a flag do build.
- Release **v0.1.39** publicada (exe + sha256 + instalador).

### Ainda em aberto

1. **OCR de verdade**: esta máquina não tem o componente
   (`Windows.Media.Ocr.dll` ausente do `System32`). Só mede em Windows reparado.
2. **Elevação/UAC, autostart, `Win+V`, multi-monitor, Mica sem transparência**:
   continuam sem medição (§4.3).
3. **Moldura nativa num Windows onde ela reaparece** (falta a segunda máquina).
4. **Discord**: soltar texto vira arquivo anexado — limitação do app deles.
5. ~~Multi-seleção + colar arquivos~~ — feito na v0.1.40 (`CF_HDROP` com N
   caminhos), medido com `_ref/teste_colar_arquivos.py`.
6. **Vídeo dentro do app (v0.1.41)**: depende dos codecs do WebView2/Media
   Foundation. `.mp4` H.264 toca aqui; numa VM enxuta (Windows sem o *Media
   Feature Pack*) pode aparecer só o erro do `<video>` — vale medir no Windows
   limpo, junto com o item 1.
7. **GIF de link (v0.1.41/0.1.43)**: precisa de rede para baixar a capa (teto de
   12 MB, sem truncar); com **Offline & private** ligado a capa não vem, de
   propósito.
8. **Chromium escondido (v0.1.43)**: páginas atrás do Cloudflare (klipy.com) são
   lidas por uma janela WebView2 invisível, aberta só quando a entrada está sem
   capa na tela. Na VM vale conferir três coisas: (a) o desafio do Cloudflare
   passa dentro de uma janela invisível (aqui passou); (b) o custo (~2,5–3 s)
   numa máquina sem GPU; (c) a janela extra não aparece no Alt+Tab/taskbar.
9. **Barra de título clássica (v0.1.45/0.1.46)**: era o defeito que só aparecia
   fora daqui. Na VM vale duas coisas: (a) abrir o app **no logon** (autostart),
   que é quando o DWM pode ainda não estar compondo, e confirmar que nenhuma
   barra aparece — `_ref/teste_moldura.py` mede isso pelo atributo do DWM;
   (b) arrastar a borda da janela para confirmar que o resize continua, porque
   foi justamente o que a primeira tentativa de correção quebrou.
   Na v0.1.46 entrou o passo que faltava (recálculo da área não-cliente com
   `SWP_FRAMECHANGED`, §8.60). **Teste novo na VM**: abrir, **maximizar e
   restaurar** e confirmar que a barra **não aparece em momento nenhum** — era
   restaurar que a apagava, agora ela já não deve existir. Vale medir a diferença
   `janela − cliente` (tem de ser 0, em `_ref/teste_faixa_presa.py` o mesmo
   cálculo na janela de brinquedo).
10. **Cor e transparência do fundo (v0.1.46)**: escolher uma cor de fundo e
    transparência de 0% a 65% e confirmar, na VM: (a) o que está atrás aparece
    (com *Janela translúcida* ligada e desligada); (b) com **efeitos de
    transparência do Windows desligados** a janela continua legível (é o caso
    que degrada); (c) a escolha sobrevive ao fechar e abrir.
