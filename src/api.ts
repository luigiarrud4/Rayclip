import { invoke } from "@tauri-apps/api/core";
import type { Entry, SaveItem, Settings, UpdateInfo } from "./lib/types";

export const listEntries = () => invoke<Entry[]>("list_entries");
export const entryText = (id: string) => invoke<string | null>("entry_text", { id });
export const pasteEntry = (id: string, plainText = false, keepOpen = false) =>
  invoke<void>("paste_entry", { id, plainText, keepOpen });
export const copyEntry = (id: string, plainText = false) =>
  invoke<void>("copy_entry", { id, plainText });
export const togglePin = (id: string) => invoke<Entry[]>("toggle_pin", { id });
export const movePinned = (id: string, up: boolean) =>
  invoke<Entry[]>("move_pinned", { id, up });
export const updateEntryContent = (id: string, text: string) =>
  invoke<Entry[]>("update_entry_content", { id, text });
export const copyText = (text: string) => invoke<void>("copy_text", { text });
export const deleteRecent = (minutes: number) =>
  invoke<Entry[]>("delete_recent", { minutes });
export const renameEntry = (id: string, title: string) =>
  invoke<Entry[]>("rename_entry", { id, title });
export const deleteEntry = (id: string) => invoke<Entry[]>("delete_entry", { id });
export const deleteEntries = (ids: string[]) => invoke<Entry[]>("delete_entries", { ids });
export const clearEntries = (keepPinned = true) =>
  invoke<Entry[]>("clear_entries", { keepPinned });
export const ocrPending = () => invoke<number>("ocr_pending");
export const linkPending = () => invoke<number>("link_pending");
export const linkEntry = (id: string) => invoke<boolean>("link_entry", { id });
export const thumbEntry = (id: string) => invoke<void>("thumb_entry", { id });
export const ocrEntry = (id: string) => invoke<void>("ocr_entry", { id });
export const copyOcrText = (id: string) => invoke<void>("copy_ocr_text", { id });
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
