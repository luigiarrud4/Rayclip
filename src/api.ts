import { invoke } from "@tauri-apps/api/core";
import type { Entry, SaveItem, Settings, UpdateInfo } from "./lib/types";

export const listEntries = () => invoke<Entry[]>("list_entries");

/// Busca e filtro acontecem no Rust: a página recebe só o que vai aparecer.
/// Antes vinham todas as entradas (1,8 MB) e o filtro era feito aqui.
export type BuscaPagina = { items: Entry[]; total: number; ms: number };
export const searchEntries = (
  query: string,
  filter: string,
  offset = 0,
  limit = 200
) =>
  invoke<BuscaPagina>("search_entries", { query, filter, offset, limit });

/// Batida do vigia do renderizador: se a página parar de mandar (renderer
/// morto), o Rust recarrega em vez de deixar a janela branca.
export const heartbeat = () => invoke<void>("heartbeat");
export const entryText = (id: string) => invoke<string | null>("entry_text", { id });
export const pasteEntry = (id: string, plainText = false, keepOpen = false) =>
  invoke<void>("paste_entry", { id, plainText, keepOpen });
export const copyEntry = (id: string, plainText = false) =>
  invoke<void>("copy_entry", { id, plainText });
/// Resposta das mutações: antes vinha o histórico inteiro (1,8 MB por clique).
export type Mudanca = { removed: number; total: number };

export const togglePin = (id: string) => invoke<Mudanca>("toggle_pin", { id });
export const movePinned = (id: string, up: boolean) =>
  invoke<Mudanca>("move_pinned", { id, up });
export const updateEntryContent = (id: string, text: string) =>
  invoke<Mudanca>("update_entry_content", { id, text });
export const copyText = (text: string) => invoke<void>("copy_text", { text });
export const deleteRecent = (minutes: number) =>
  invoke<Mudanca>("delete_recent", { minutes });
export const renameEntry = (id: string, title: string) =>
  invoke<Mudanca>("rename_entry", { id, title });
export const deleteEntry = (id: string) => invoke<Mudanca>("delete_entry", { id });
export const deleteEntries = (ids: string[]) => invoke<Mudanca>("delete_entries", { ids });
export const clearEntries = (keepPinned = true) =>
  invoke<Mudanca>("clear_entries", { keepPinned });
export const ocrPending = () => invoke<number>("ocr_pending");
export const linkPending = () => invoke<number>("link_pending");
export const linkEntry = (id: string) => invoke<boolean>("link_entry", { id });
/** Metadados por navegador de verdade (janela escondida). Só para links
 *  sem capa, e só quando a entrada está aberta na tela. */
export const linkWebMeta = (id: string) => invoke<boolean>("link_web_meta", { id });
export const thumbEntry = (id: string) => invoke<void>("thumb_entry", { id });
export const ocrEntry = (id: string) => invoke<void>("ocr_entry", { id });
export const copyOcrText = (id: string) => invoke<void>("copy_ocr_text", { id });

/// Windows sem o componente de OCR: a interface não pode dizer "nenhum texto".
let _ocrDisponivel: Promise<boolean> | null = null;
export const ocrDisponivel = () => {
  if (!_ocrDisponivel) _ocrDisponivel = invoke<boolean>("ocr_disponivel").catch(() => true);
  return _ocrDisponivel;
};
export const hideWindow = () => invoke<void>("hide_window");
export const revealPath = (path: string) => invoke<void>("reveal_path", { path });
export const openUrl = (url: string) => invoke<void>("open_url", { url });
export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) =>
  invoke<string[]>("save_settings", { settings });
export const restartExplorer = () => invoke<void>("restart_explorer");
export const setWindowsTransparency = (enable: boolean) =>
  invoke<boolean>("set_windows_transparency", { enable });

// `force` ignora o cache de 30 s do backend (usado depois de alterar algo).
export const systemStatus = (force = false) =>
  invoke<{
    winVDisabled: boolean;
    version?: string;
    startupTask: boolean;
    elevated: boolean;
    exe: string;
    winTransparency?: boolean;
    encrypted?: boolean;
    dbPath?: string;
  }>("system_status", { force });
/** Consulta a última release do GitHub. `force` ignora o cache de 6 h. */
export const updateCheck = (force = false) =>
  invoke<UpdateInfo | null>("update_check", { force });
/** Baixa, confere e instala a atualização; o app reabre sozinho. */
export const updateInstall = () => invoke<string>("update_install");
/** Itens disponíveis no histórico do Raycast original. */
export const raycastImportCount = () => invoke<number>("raycast_import_count");
/** Importa o histórico do Raycast para o banco cifrado. Devolve o total. */
export const importRaycast = (path?: string) =>
  invoke<number>("import_raycast", { path: path ?? null });
/** Reclama espaço no arquivo do banco (`VACUUM`). Devolve o tamanho antes/depois. */
export const compactDb = () =>
  invoke<{ before: number; after: number }>("compact_db");
export const pasteMany = (ids: string[], plainText: boolean, keepOpen: boolean) =>
  invoke<number>("paste_many", { ids, plainText, keepOpen });
export const saveImageAs = (id: string, format: string) =>
  invoke<string | null>("save_image_as", { id, format });
/** "Save as…" para qualquer entrada (imagem, texto, link, arquivo). */
export const saveEntryAs = (id: string, format: string) =>
  invoke<string | null>("save_entry_as", { id, format });
/** Salva várias entradas de uma vez, cada uma no formato escolhido. */
export const saveMany = (items: SaveItem[]) =>
  invoke<string[] | null>("save_many", { items });

/**
 * Arrasto para fora do app (Explorer, pasta, Discord, Photoshop…).
 * O arrasto em si é do Chromium (`DownloadURL` → `CF_HDROP`); o backend só
 * entrega nome, tipo e os bytes em base64.
 */
export const dragPayload = (id: string) =>
  invoke<{
    name: string;
    mime: string;
    data: string;
    text: string | null;
    link: boolean;
  }>("drag_payload", { id });
/** Marca o começo/fim do arrasto para a janela não esconder no meio. */
export const dragStart = () => invoke<void>("drag_start");
export const dragEnd = () => invoke<void>("drag_end");
/**
 * Caminhos no disco para o arrasto do **sistema** (`CF_HDROP` via OLE).
 *
 * O `DownloadURL` do Chromium leva os bytes e para de funcionar entre 3 e
 * 25 MB; o OLE leva o caminho. `Err` quando a entrada não é arquivo (texto,
 * link) — nesse caso a linha continua arrastando pelo Chromium.
 */
export const dragFiles = (id: string) => invoke<string[]>("drag_files", { id });

/**
 * Cola vários **arquivos** de uma vez: uma `CF_HDROP` com todos os caminhos e
 * uma única colagem, em vez de N colagens em sequência.
 */
export const pasteFiles = (ids: string[], keepOpen = false) =>
  invoke<number>("paste_files", { ids, keepOpen });
