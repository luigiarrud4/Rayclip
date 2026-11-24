# Plano de continuação (VM de testes)

Estado no momento em que este plano foi escrito:

- versão publicada: **v0.1.38** (https://github.com/luigiarrud4/Rayclip/releases/tag/v0.1.38)
- código-fonte empacotado em `rayclip-projeto.zip` (sha256 `70fada2e…`), sem `target/`,
  sem `node_modules`, sem a flag de CDP;
- `cargo test --lib`: **43/43**; `npx tsc --noEmit`: 0 erros.

---

## 1. Objetivo desta rodada

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
6. Para os testes automatizados: adicionar temporariamente
   `"additionalBrowserArgs": "--remote-debugging-port=9222"` em
   `src-tauri/tauri.conf.json` (e **remover antes de qualquer build release** —
   `grep -c remote-debugging src-tauri/tauri.conf.json` tem que dar 0).

---

## 4. Ordem de trabalho na VM

### 4.1 Isolar o travamento do OLE (primeiro, é o que destrava o resto)

1. Reintroduzir `dragout.rs` (cópia do histórico do git ou reescrever a partir da
   seção 2) e o comando `drag_files`.
2. **Instrumentar cada etapa** com log antes/depois: `OleInitialize`,
   `CreateWindowExW`, `ShowWindow`, `SHParseDisplayName`, `SHCreateDataObject`,
   `SHDoDragDrop`. O log que faltou é o que aponta a etapa travada.
3. Rodar um arrasto para o **Notepad** (uma janela que aceita soltura de arquivo)
   em vez do Explorer: elimina variável do Explorer.
4. Rodar dois cenários com o cursor **começando longe da janela do app**
   (arrastar a partir de uma linha, mas com a janela já escondida/em outra posição)
   para testar a hipótese "`DragEnter` na própria janela do WebView2".
5. Se for o `DragEnter` da própria janela: avaliar, em ordem,
   - `DoDragDrop` chamado com o cursor fora da nossa janela (mover o cursor
     programaticamente para a borda antes de iniciar e devolver depois — feio, mas
     serve para **provar** a hipótese);
   - iniciar o arrasto OLE a partir de `dragend` (depois do gesto do Chromium
     terminar) — sozinho não resolveu porque `preventDefault()` no `dragstart`
     **não dispara `dragend`**; testar `setTimeout(…, 0)`/`requestAnimationFrame`
     em vez de `dragend`;
   - `SHDoDragDrop` com `IDropTarget` desligado no `WebView2` (testar se a
     implementação do WebView2 é quem trava: `WebView2` tem `AllowExternalDrop`).
6. Critério de aceite: arrastar o `.exe` de 25 MB para uma pasta do Explorer cria o
   arquivo com o **nome e a extensão certos**, em menos de 1 s, sem congelar a
   janela e sem copiar os bytes para o app (`%TEMP%` sem arquivo a mais).

Se o OLE não fechar em ~2 h de VM, plano B documentado: manter o `DownloadURL`
para tudo até 8 MB e, acima disso, mostrar um aviso curto ("arquivo grande demais
para arrastar — use *Save as…*") em vez de não fazer nada em silêncio. Medir o teto
real do `DownloadURL` (8 MB, 12 MB, 16 MB) antes de escolher o número.

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

1. `cargo test --lib` (43/43 ou mais, com testes novos do arrasto OLE).
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
- Release nunca sai com a flag de CDP.
- Nenhuma otimização entra sem ganho medido no mesmo build.

---

## 6. O que eu preciso do usuário

1. Acesso à VM (RDP: usuário/senha, ou agente já rodando).
2. Confirmar que é uma **VM descartável** (posso instalar Rust/Node/Python e
   reiniciar).
3. Se possível, uma segunda resolução de tela/monitor para o item 4.3.
