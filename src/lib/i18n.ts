/**
 * Idioma da interface.
 *
 * Regra simples: a **chave é o texto em inglês**. Assim a tradução é uma
 * tabela `inglês → outro idioma`, o código continua legível (`t("Paste")`) e
 * qualquer texto que ainda não tenha tradução aparece em inglês em vez de
 * mostrar uma chave crua.
 */

export type Lang = "en" | "pt-BR";

export const LANGUAGES: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "pt-BR", label: "Português (Brasil)" },
];

export const isLang = (v: unknown): v is Lang => v === "en" || v === "pt-BR";

/** Idioma para datas/horas (`toLocaleString`). */
export const localeOf = (lang: Lang) => (lang === "pt-BR" ? "pt-BR" : "en-US");

export type T = (text: string, vars?: Record<string, string | number>) => string;

export function makeT(lang: Lang): T {
  return (text, vars) => {
    let out = lang === "pt-BR" ? PT[text] ?? text : text;
    if (vars) {
      for (const [k, v] of Object.entries(vars)) out = out.split(`{${k}}`).join(String(v));
    }
    return out;
  };
}

/** Nome do tipo de entrada como o usuário lê (lista, painel, confirmação). */
export function kindLabel(kind: string, t: T): string {
  const map: Record<string, string> = {
    text: "Text",
    image: "Image",
    link: "Link",
    email: "Email",
    color: "Color",
    file: "File",
    files: "Files",
    pdf: "PDF",
    video: "Video",
    audio: "Audio",
    archive: "Archive",
  };
  return t(map[kind] ?? kind);
}

const PT: Record<string, string> = {
  // ---------------------------------------------------------------- tipos
  Text: "Texto",
  Image: "Imagem",
  Link: "Link",
  Email: "E-mail",
  Color: "Cor",
  File: "Arquivo",
  Files: "Arquivos",
  PDF: "PDF",
  Video: "Vídeo",
  Audio: "Áudio",
  Archive: "Compactado",
  Untitled: "Sem título",
  "Image ({when})": "Imagem ({when})",

  // --------------------------------------------------------- barra / status
  Paste: "Colar",
  Actions: "Ações",
  Copy: "Copiar",
  Cancel: "Cancelar",
  Back: "Voltar",
  Settings: "Configurações",
  "Settings…": "Configurações…",
  Close: "Fechar",
  "Esc to close": "Esc para fechar",
  "Esc to cancel · ← → to switch": "Esc para cancelar · ← → para alternar",
  "+{n} more": "+{n} mais",
  "Pasted into the app you came from": "Colado no app de onde você veio",
  "Pasted — window kept open": "Colado — janela mantida aberta",
  "Paste as plain text": "Colar como texto simples",
  "Copied to clipboard": "Copiado para o clipboard",
  "Renamed": "Renomeado",
  "Could not rename": "Não foi possível renomear",
  "Entry deleted": "Entrada apagada",
  "{n} entries deleted": "{n} entradas apagadas",
  "Delete failed - {msg}": "Falha ao apagar — {msg}",
  "Image text copied": "Texto da imagem copiado",
  "Reading image…": "Lendo imagem…",
  "History cleared": "Histórico limpo",
  "This entry has nothing to save": "Esta entrada não tem nada para salvar",
  "Saved to {name}": "Salvo em {name}",
  "Save cancelled": "Salvamento cancelado",
  "Save failed — {msg}": "Falha ao salvar — {msg}",
  "Paste {n} items in order": "Colar {n} itens em ordem",
  "{n} items pasted in order": "{n} itens colados em ordem",
  "Paste {n} files": "Colar {n} arquivos",
  "{n} files pasted": "{n} arquivos colados",
  "Delete {n} entries?": "Apagar {n} entradas?",

  // --------------------------------------------------------- ações (painel)
  "Paste as Plain Text": "Colar como texto simples",
  "Paste and Keep Window Open": "Colar e manter a janela aberta",
  "Copy to Clipboard": "Copiar para o clipboard",
  "Rename Entry": "Renomear entrada",
  "Pin Entry": "Fixar entrada",
  "Unpin Entry": "Desafixar entrada",
  "Open Link": "Abrir link",
  "Open Preview": "Abrir prévia",
  "Copy Text from Image": "Copiar texto da imagem",
  "Extract Text (OCR)": "Extrair texto (OCR)",
  "Show in Folder": "Mostrar na pasta",
  "Delete Entry": "Apagar entrada",
  "Clear All (keep pinned)": "Apagar tudo (manter fixados)",
  "Save As…": "Salvar como…",
  "Paste {n} Items in Order": "Colar {n} itens em ordem",
  "Paste in Order and Keep Window Open": "Colar em ordem e manter a janela aberta",
  "Save {n} Items As…": "Salvar {n} itens como…",
  "Delete {n} Entries": "Apagar {n} entradas",
  "Clear Selection ({n})": "Limpar seleção ({n})",
  "Save as PNG (default)": "Salvar como PNG (padrão)",
  "Save as JPEG": "Salvar como JPEG",
  "Save as WebP": "Salvar como WebP",
  "Save as BMP": "Salvar como BMP",
  "Save as GIF": "Salvar como GIF",
  "Save as TXT (default)": "Salvar como TXT (padrão)",
  "Save as Markdown (.md)": "Salvar como Markdown (.md)",
  "Save as JSON": "Salvar como JSON",
  "Save as CSV": "Salvar como CSV",
  "Save as HTML": "Salvar como HTML",
  "Save as Rich Text (.rtf)": "Salvar como Rich Text (.rtf)",
  "Save as Internet Shortcut (.url)": "Salvar como atalho da Internet (.url)",
  "Save as Lua": "Salvar como Lua",
  "Save as JavaScript": "Salvar como JavaScript",
  "Save as Python": "Salvar como Python",
  "They are removed from the history for good — pinned entries included. This can't be undone.":
    "Elas saem do histórico para sempre — inclusive as fixadas. Não dá para desfazer.",

  // ------------------------------------------------------------- barra de cima
  "Type to filter entries…": "Digite para filtrar as entradas…",
  "All Types": "Todos os tipos",
  "Text Only": "Somente texto",
  "Images Only": "Somente imagens",
  "Files Only": "Somente arquivos",
  "Links Only": "Somente links",
  "Emails Only": "Somente e-mails",
  "Colors Only": "Somente cores",

  // ------------------------------------------------------------------- lista
  "No results for “{q}”": "Nenhum resultado para “{q}”",
  "Try a different search term.": "Tente outro termo de busca.",
  "No clipboard entries yet": "Ainda não há nada no histórico",
  "Copy something and it will show up here.": "Copie algo e vai aparecer aqui.",
  Pinned: "Fixados",
  Today: "Hoje",
  Yesterday: "Ontem",
  "This Week": "Esta semana",
  "This Month": "Este mês",
  Earlier: "Anteriores",

  // ------------------------------------------------------------ painel lateral
  "Select an entry to see details": "Selecione uma entrada para ver os detalhes",
  "Selected · {n} items": "Selecionados · {n} itens",
  "Press {enter} to paste them in this order, or {esc} to clear the selection. Ctrl+click toggles an item, Shift+click takes a range.":
    "Pressione {enter} para colar nessa ordem, ou {esc} para limpar a seleção. Ctrl+clique marca/desmarca um item, Shift+clique pega um intervalo.",
  "Click the image to zoom": "Clique na imagem para dar zoom",
  "Open the big preview": "Abrir a prévia grande",
  Metadata: "Metadados",
  Information: "Informações",
  Source: "Origem",
  Unknown: "Desconhecida",
  Dimensions: "Dimensões",
  Type: "Tipo",
  Size: "Tamanho",
  Domain: "Domínio",
  Address: "Endereço",
  Characters: "Caracteres",
  Words: "Palavras",
  Lines: "Linhas",
  Name: "Nome",
  Where: "Onde",
  "Text in Image": "Texto na imagem",
  "No text detected": "Nenhum texto detectado",
  "OCR unavailable on this Windows": "OCR indisponível neste Windows",
  "Space or Esc to close": "Espaço ou Esc para fechar",

  // --------------------------------------------------------------- salvar N
  "Save {n} items": "Salvar {n} itens",
  "Pick a format for each entry — images keep animation, text is written as-is.":
    "Escolha um formato para cada item — imagens mantêm a animação, texto vai como está.",
  "Apply to all": "Aplicar a todos",
  "Same format…": "Mesmo formato…",
  "Images → Custom…": "Imagens → Personalizado…",
  "Text → Custom…": "Texto → Personalizado…",
  "Custom…": "Personalizado…",
  "Set every row to the same format": "Colocar todas as linhas no mesmo formato",
  "Open the big preview (Space)": "Abrir a prévia grande (Espaço)",
  Preview: "Prévia",
  "Space to enlarge": "Espaço para ampliar",
  "No preview": "Sem prévia",
  "Files are written to the folder you pick next.":
    "Os arquivos são gravados na pasta que você escolher a seguir.",
  "Choose folder & save": "Escolher pasta e salvar",
  "Saving…": "Salvando…",
  "Type an extension for the highlighted items": "Digite uma extensão para os itens destacados",
  "{n} files saved to {folder}": "{n} arquivos salvos em {folder}",
  "Quick look preview": "Prévia rápida",
};

// ---------------------------------------------------------------- Settings
Object.assign(PT, {
  Preferences: "Preferências",
  Shortcuts: "Atalhos",
  Pasting: "Colagem",
  History: "Histórico",
  "Privacy & Indexing": "Privacidade e indexação",
  "Data & Security": "Dados e segurança",
  System: "Sistema",
  About: "Sobre",
  Language: "Idioma",
  "Language of the whole interface": "Idioma de toda a interface",
  "Loading settings…": "Carregando as configurações…",
  "Back (Esc)": "Voltar (Esc)",
  "Settings saved": "Configurações salvas",
  "Privacy": "Privacidade",
  "Preview": "Prévia",
  "Appearance": "Aparência",

  "Offline & private": "Offline e privado",
  "Rayclip makes no network request at all. Link entries keep just the copied URL.":
    "O Rayclip não faz nenhuma requisição de rede. Entradas de link guardam apenas a URL copiada.",
  "Fetch link previews online": "Buscar prévias de link online",
  "Reads title, favicon and cover from the site — only for links you copy":
    "Lê título, favicon e capa do site — só para os links que você copia",
  "Offline mode is on.": "O modo offline está ligado.",
  "Nothing leaves this computer: no title, no favicon and no cover is downloaded, and previews already cached stay as they are.":
    "Nada sai deste computador: nenhum título, favicon ou capa é baixado, e as prévias já guardadas ficam como estão.",
  "With previews on, only the domain of a copied link is contacted, once, to read its page title and cover image. The result is cached on disk; nothing is uploaded and no account is involved.":
    "Com as prévias ligadas, apenas o domínio do link copiado é contatado, uma vez, para ler o título da página e a imagem de capa. O resultado fica guardado em disco; nada é enviado e não existe conta.",
  "Preview on hover": "Prévia ao passar o mouse",
  "Show the content of the entry under the mouse without clicking it":
    "Mostra o conteúdo da entrada sob o mouse sem precisar clicar",
  "With this off, use ↑ ↓ or click to move the preview. Text in the preview stays selectable, so you can copy just one part of a long entry.":
    "Desligado, use ↑ ↓ ou clique para mover a prévia. O texto da prévia continua selecionável, então dá para copiar só um pedaço de uma entrada longa.",
  "Theme": "Tema",
  "Colour scheme of the whole window": "Esquema de cores de toda a janela",
  "Accent colour": "Cor de destaque",
  "Selection, highlight and badges": "Seleção, realce e etiquetas",
  "Accent colour hex": "Cor de destaque em hexadecimal",
  "Translucent window": "Janela translúcida",
  "Mica backdrop: you can see what is behind the window (Windows 11)":
    "Fundo Mica: você vê o que está atrás da janela (Windows 11)",
  "Windows transparency effects are off, so the blur can’t be drawn.":
    "Os efeitos de transparência do Windows estão desligados, então o desfoque não aparece.",
  "Enable it": "Ativar",
  "Windows transparency enabled": "Transparência do Windows ativada",

  "Global shortcuts": "Atalhos globais",
  "Inside the window": "Dentro da janela",
  "Open Clipboard History": "Abrir o histórico do clipboard",
  "Global shortcut, works in any app": "Atalho global, funciona em qualquer app",
  "Alternative Shortcut": "Atalho alternativo",
  "Second global shortcut for the same action": "Segundo atalho global para a mesma ação",
  "↑ ↓ move the selection, Esc closes. Hold Shift while pasting to strip formatting.":
    "↑ ↓ movem a seleção, Esc fecha. Segure Shift ao colar para remover a formatação.",
  "Press keys…": "Aperte as teclas…",
  "Click a shortcut and press the new combination. At least one modifier key (Ctrl, Alt, Shift or Win) is required.":
    "Clique em um atalho e aperte a nova combinação. É obrigatório pelo menos um modificador (Ctrl, Alt, Shift ou Win).",

  "Strip formatting from every paste": "Remove a formatação de toda colagem",
  "Keep window open after pasting": "Manter a janela aberta depois de colar",
  "Paste several entries in a row": "Cola várias entradas em sequência",
  "Maximum entries": "Máximo de entradas",
  "Oldest unpinned entries are removed first": "As entradas não fixadas mais antigas saem primeiro",
  "Keep history for": "Manter o histórico por",
  "Pinned entries are never removed": "As entradas fixadas nunca são removidas",
  "Unlimited": "Sem limite",
  "1 day": "1 dia",
  "1 week": "1 semana",
  "1 month": "1 mês",
  "3 months": "3 meses",
  "1 year": "1 ano",
  "{n} entries": "{n} entradas",

  "Ignore password managers": "Ignorar gerenciadores de senha",
  "Never record content copied from 1Password, Bitwarden, KeePass…":
    "Nunca registra o que é copiado do 1Password, Bitwarden, KeePass…",
  "Also capture passwords & protected content": "Capturar também senhas e conteúdo protegido",
  "Keeps what apps ask us not to record: password-manager copies, private/incognito browser windows and clipboard items marked as secret. Off by default.":
    "Mantém o que os apps pedem para não registrar: cópias de gerenciador de senha, janelas anônimas/privadas do navegador e itens marcados como secretos. Desligado por padrão.",
  "Index text in images (OCR)": "Indexar texto em imagens (OCR)",
  "Search inside screenshots with Windows OCR":
    "Busca dentro de capturas de tela com o OCR do Windows",
  "Everything stays on this computer. To switch off every network request, see Preferences → Offline & private.":
    "Tudo fica neste computador. Para desligar qualquer requisição de rede, veja Preferências → Offline e privado.",

  "Database encryption": "Criptografia do banco",
  "Clipboard text, rich text and images are stored in a SQLCipher database (AES-256 + HMAC-SHA512)":
    "Texto, rich text e imagens do clipboard ficam em um banco SQLCipher (AES-256 + HMAC-SHA512)",
  "The 256-bit key lives in rayclip.key, sealed with Windows DPAPI, so only your Windows user can open it — copying the files to another account or computer is useless. Everything (text, HTML/RTF, images) is inside rayclip.db; no loose files are left behind.":
    "A chave de 256 bits fica em rayclip.key, lacrada com Windows DPAPI: só o seu usuário do Windows abre. Copiar os arquivos para outra conta ou computador não serve para nada. Tudo (texto, HTML/RTF, imagens) está dentro de rayclip.db; nenhum arquivo solto fica para trás.",
  "Import from Raycast": "Importar do Raycast",
  "Copies your Raycast clipboard history into this encrypted database":
    "Copia o histórico do clipboard do Raycast para este banco criptografado",
  "Check Raycast history": "Verificar histórico do Raycast",
  "Raycast history not found": "Histórico do Raycast não encontrado",
  "Import {n} items": "Importar {n} itens",
  "Importing {done}/{total}": "Importando {done}/{total}",
  "Imported {n} items from Raycast": "{n} itens importados do Raycast",
  "Import failed — {msg}": "Falha na importação — {msg}",
  "Raycast keeps its own database locked with a DPAPI key, but it also writes each item as a file in %LOCALAPPDATA%\Raycast\clipboard. The import reads those files (images, CF_HTML, RTF, text), keeps the original dates, and leaves the Raycast files untouched. Raycast itself is not installed or modified.":
    "O Raycast tranca o próprio banco com uma chave DPAPI, mas também grava cada item como arquivo em %LOCALAPPDATA%\Raycast\clipboard. A importação lê esses arquivos (imagens, CF_HTML, RTF, texto), mantém as datas originais e não mexe em nada do Raycast. O Raycast em si não é instalado nem modificado.",

  "Take over Win+V": "Assumir o Win+V",
  "Disable the native Windows clipboard panel so Win+V opens Rayclip":
    "Desliga o painel nativo do Windows para o Win+V abrir o Rayclip",
  "Windows keeps its own shortcut in Explorer. Rayclip writes DisabledHotkeys = \"V\" under HKCU\…\Explorer\Advanced (the same trick CopyQ and Ditto use). The change only applies after Explorer restarts.":
    "O Windows guarda o próprio atalho no Explorer. O Rayclip grava DisabledHotkeys = \"V\" em HKCU\…\Explorer\Advanced (o mesmo truque do CopyQ e do Ditto). A mudança só vale depois de reiniciar o Explorer.",
  "Windows status:": "Situação no Windows:",
  "Win+V disabled": "Win+V desativado",
  "Win+V active in Windows": "Win+V ativo no Windows",
  "Apply now": "Aplicar agora",
  "Restarts Explorer (open File Explorer windows close)":
    "Reinicia o Explorer (janelas do Explorador abertas fecham)",
  "Compact database": "Compactar banco",
  "Rewrites the database file without the free pages left behind by deletions":
    "Reescreve o arquivo do banco sem as páginas livres deixadas pelas exclusões",
  "This only reclaims disk space: deleted entries were already zeroed inside the encrypted file (secure_delete). Freeing a page does not prove the old bytes are gone from the SSD, and nothing here can promise that.":
    "Isto só devolve espaço em disco: as entradas apagadas já foram zeradas dentro do arquivo cifrado (secure_delete). Liberar uma página não prova que os bytes antigos saíram do SSD, e nada aqui promete isso.",
  "Compact now": "Compactar agora",
  "Compacting…": "Compactando…",
  "Database compacted — {a} to {b}": "Banco compactado — {a} para {b}",
  "Compact failed — {msg}": "Falha ao compactar — {msg}",
  "Restart Explorer": "Reiniciar o Explorer",
  "Explorer restarted": "Explorer reiniciado",
  "Restart failed — {msg}": "Falha ao reiniciar — {msg}",
  "Start with Windows": "Iniciar com o Windows",
  "Registers a Task Scheduler job that starts Rayclip as administrator without a UAC prompt":
    "Registra uma tarefa no Agendador que abre o Rayclip como administrador sem pedir UAC",
  "Startup job:": "Tarefa de inicialização:",
  "scheduled task active": "tarefa agendada ativa",
  "not registered": "não registrada",
  "This window:": "Esta janela:",
  "running as administrator": "rodando como administrador",
  "standard user": "usuário comum",
  "Rayclip is designed to run elevated: that is what lets it paste into elevated apps (Task Manager, admin consoles) and change Win+V and auto-start without being blocked. A UAC prompt appears on manual starts; the auto-start job and the relaunch task open it already elevated.":
    "O Rayclip foi feito para rodar elevado: é isso que permite colar em apps elevados (Gerenciador de Tarefas, consoles de admin) e mudar o Win+V e a inicialização automática sem ser bloqueado. Ao abrir na mão aparece um pedido de UAC; a tarefa de inicialização e a tarefa de relançamento já abrem elevadas.",

  "Clipboard History for Windows, inspired by Raycast":
    "Histórico do clipboard para Windows, inspirado no Raycast",
  Executable: "Executável",
  Data: "Dados",
  "Built with Tauri + React · no account, no telemetry":
    "Feito com Tauri + React · sem conta, sem telemetria",
});

Object.assign(PT, {
  "Clipboard History": "Histórico do clipboard",
});

Object.assign(PT, {
  // atalhos internos
  "Paste selected item": "Colar o item selecionado",
  "Hold Shift while pasting to strip formatting": "Segure Shift ao colar para remover a formatação",
  "Paste and keep window open": "Colar e manter a janela aberta",
  "Great for pasting several items in a row": "Ótimo para colar vários itens em sequência",
  "Show actions panel": "Mostrar painel de ações",
  "Rename, pin, delete, open, OCR…": "Renomear, fixar, apagar, abrir, OCR…",
  "Rename item": "Renomear item",
  "Edit the name inline": "Edita o nome ali mesmo",
  "Pin / unpin item": "Fixar / desafixar item",
  "Pinned items stay on top": "Itens fixados ficam no topo",
  "Delete item": "Apagar item",
  "Clear history (keeps pinned)": "Limpar histórico (mantém os fixados)",
  "Copy text found in image": "Copiar o texto achado na imagem",
  "Requires the OCR badge": "Precisa do selo de OCR",
  "Open link in browser": "Abrir o link no navegador",
  "Open these settings": "Abrir estas configurações",

  // avisos e detalhes
  "Saved with warnings: {list}": "Salvo com avisos: {list}",
  "Open shortcut": "Atalho de abrir",
  "Paste shortcut": "Atalho de colar",
  "{field} {key} is already used by another app — kept {kept}":
    "{field} {key} já está em uso por outro app — mantive {kept}",
  "{field} {key} could not be registered": "{field} {key} não pôde ser registrado",
  "shortcut unavailable": "atalho indisponível",
  "Encrypted": "Criptografado",
  "Unavailable": "Indisponível",
  "GIF · animated": "GIF · animado",
  "{n} chars": "{n} caracteres",
  "Save selected items": "Salvar itens selecionados",
  "Reset": "Redefinir",
  "Click to zoom": "Clique para dar zoom",
  "Drag to resize": "Arraste para redimensionar",
  "Image preview": "Prévia da imagem",
  "Scroll to zoom · drag to move · corner to resize · Esc to close":
    "Scroll para dar zoom · arraste para mover · canto para redimensionar · Esc para fechar",
  "Scroll to zoom · double-click to reset · Esc to exit":
    "Scroll para dar zoom · duplo clique para voltar · Esc para sair",
  "Scroll to zoom · drag to move · double-click to reset · Esc to exit":
    "Scroll para dar zoom · arraste para mover · duplo clique para voltar · Esc para sair",
  "scroll to zoom · Esc to exit": "scroll para dar zoom · Esc para sair",
  "URL": "URL",
  "GIF Image": "Imagem GIF",
  "PNG Image": "Imagem PNG",
});

Object.assign(PT, {
  "Delete {n}": "Apagar {n}",
  "Key action failed - {msg}": "Ação do teclado falhou — {msg}",
});

// v0.1.23 — grupos de data, reordenar fixados, editar conteúdo, copiar cor
// em outros formatos e "apagar recentes".
Object.assign(PT, {
  "This Year": "Este ano",
  Older: "Mais antigos",
  Save: "Salvar",
  "Loading…": "Carregando…",
  "Move Up in Pinned": "Subir nos fixados",
  "Move Down in Pinned": "Descer nos fixados",
  "Edit Content…": "Editar conteúdo…",
  "Edit Content": "Editar conteúdo",
  "Add content…": "Adicionar conteúdo…",
  "Esc to cancel · Ctrl+Enter to save": "Esc para cancelar · Ctrl+Enter para salvar",
  "Entry saved": "Entrada salva",
  "Could not save — {msg}": "Não foi possível salvar — {msg}",
  "Copy Color as…": "Copiar cor como…",
  "Copied {label} to clipboard": "{label} copiado para o clipboard",
  "Copy failed — {msg}": "Falha ao copiar — {msg}",
  "Delete Recent Entries…": "Apagar entradas recentes…",
  "Delete recent entries?": "Apagar entradas recentes?",
  "Entries copied in the last {when} are removed for good. Pinned entries are kept.":
    "As entradas copiadas nos últimos {when} são apagadas de vez. Os fixados são mantidos.",
  "Last {n} Minutes": "Últimos {n} minutos",
  "Last Hour": "Última hora",
  "Last 24 Hours": "Últimas 24 horas",
  "{n} minutes": "{n} minutos",
  "1 hour": "1 hora",
  "24 hours": "24 horas",
  "{n} recent entries deleted": "{n} entradas recentes apagadas",
});

Object.assign(PT, {
  // ---- Atualização automática (GitHub releases) -------------------------
  Updates: "Atualizações",
  About: "Sobre",
  Preferences: "Preferências",
  Shortcuts: "Atalhos",
  Pasting: "Colagem",
  History: "Histórico",
  "Privacy & Indexing": "Privacidade e indexação",
  "Data & Security": "Dados e segurança",
  System: "Sistema",
  "Installed version": "Versão instalada",
  "Rayclip updates itself from the GitHub releases of its repository":
    "O Rayclip se atualiza pelas releases do GitHub do próprio repositório",
  "Check for updates automatically": "Procurar atualizações automaticamente",
  "Looks for a new release on start-up and every 6 hours. Never installs on its own.":
    "Procura uma release nova ao abrir e a cada 6 horas. Nunca instala sozinho.",
  "Check now": "Verificar agora",
  "Ask GitHub for the latest release": "Pergunta ao GitHub qual é a última release",
  "Checking…": "Verificando…",
  "Check for updates": "Procurar atualizações",
  "Version {v} is available": "A versão {v} está disponível",
  "You are on the latest version": "Você está na versão mais nova",
  "Could not check for updates — {msg}": "Não foi possível verificar — {msg}",
  "Installed v{v} — Rayclip is reopening": "v{v} instalada — o Rayclip está reabrindo",
  "Update failed — {msg}": "Falha na atualização — {msg}",
  "Download, check and restart in the new version":
    "Baixa, confere e reabre na versão nova",
  "This release has no rayclip.exe to download":
    "Esta release não tem o rayclip.exe para baixar",
  "Install update": "Instalar atualização",
  "Release page": "Página da release",
  "Downloading… {pct}%": "Baixando… {pct}%",
  "Downloading…": "Baixando…",
  "The encrypted database could not be opened, so nothing is being saved this session: {msg}. The application fails closed on purpose — it never writes a key that is not protected by Windows, and never falls back to a predictable one.":
    "O banco cifrado não pôde ser aberto, então nada está sendo salvo nesta sessão: {msg}. O aplicativo falha fechado de propósito — nunca grava uma chave que não esteja protegida pelo Windows, nem cai para uma chave previsível.",
  "Update {v}": "Atualizar {v}",
  "Update available — click to install": "Atualização disponível — clique para instalar",
  "The update downloads the new rayclip.exe, checks its SHA-256 against the one published in the release, and swaps the file in place. The running exe is only renamed, never overwritten, and the previous version is kept next to it until the next start — so a failed update always leaves you with a working Rayclip. Your history, settings and database are never touched.":
    "A atualização baixa o rayclip.exe novo, confere o SHA-256 com o publicado na release e troca o arquivo no lugar. O exe em execução só é renomeado, nunca sobrescrito, e a versão anterior fica ao lado até a próxima abertura — então uma atualização que falha sempre deixa um Rayclip funcionando. Seu histórico, suas configurações e o banco não são tocados.",
});
