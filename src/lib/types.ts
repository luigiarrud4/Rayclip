export type EntryKind =
  | "text"
  | "link"
  | "email"
  | "color"
  | "image"
  | "file"
  | "files"
  | "pdf"
  | "video"
  | "audio"
  | "archive";

export interface LinkMeta {
  title?: string | null;
  description?: string | null;
  siteName?: string | null;
  icon?: string | null;
  imagePath?: string | null;
  url?: string | null;
  /** Já tentamos buscar o título e o site não entregou (bloqueio/JS). */
  tried?: boolean;
}

export interface Entry {
  id: string;
  kind: EntryKind;
  title: string;
  text?: string | null;
  paths: string[];
  /** A imagem está no banco cifrado (servida por mediaUrl). */
  hasImage?: boolean;
  imageWidth?: number | null;
  imageHeight?: number | null;
  source?: string | null;
  /** PNG do ícone do app de origem (Raycast mostra na linha "Source"). */
  sourceIcon?: string | null;
  ocrText?: string | null;
  /** Miniatura/poster guardado no banco (servida por mediaUrl). */
  hasThumb?: boolean;
  /** A imagem é um GIF animado. */
  gif?: boolean;
  link?: LinkMeta | null;
  sizeBytes: number;
  createdAt: number;
  pinned: boolean;
  hash: string;
}

export type FilterId =
  | "all"
  | "text"
  | "image"
  | "file"
  | "link"
  | "email"
  | "color";

export type StatusKind = "ok" | "err";

export interface Status {
  kind: StatusKind;
  text: string;
}

export interface InAppKeys {
  paste: string;
  pasteKeepOpen: string;
  actions: string;
  pin: string;
  delete: string;
  clearAll: string;
  copyText: string;
  rename: string;
  openLink: string;
  settings: string;
}

export const DEFAULT_IN_APP_KEYS: InAppKeys = {
  paste: "Enter",
  pasteKeepOpen: "Ctrl+Enter",
  actions: "Ctrl+K",
  pin: "Ctrl+P",
  delete: "Delete",
  clearAll: "Ctrl+Shift+Delete",
  copyText: "Ctrl+T",
  rename: "Ctrl+E",
  openLink: "Ctrl+O",
  settings: "Ctrl+,",
};

export interface Settings {
  inAppKeys: InAppKeys;
  showHotkey: string;
  pasteHotkey: string;
  pastePlainDefault: boolean;
  keepOpenAfterPaste: boolean;
  maxEntries: number;
  retentionDays: number;
  replaceWinV: boolean;
  ignorePasswordManagers: boolean;
  captureSensitive: boolean;
  ocrEnabled: boolean;
  linkPreviews: boolean;
  launchAtLogin: boolean;
  /** Nenhuma requisição de rede (links ficam só com o texto). */
  offlineMode: boolean;
  /** Mostrar o conteúdo ao passar o mouse pelas entradas. */
  hoverPreview: boolean;
  /** Tema pronto: dark | light | midnight | nord | rose */
  theme: string;
  /** Cor de destaque (#rrggbb). */
  accent: string;
  /** Fundo translúcido (Mica) — dá para ver o que está atrás da janela. */
  translucent: boolean;
  /** Idioma da interface: "en" (padrão) ou "pt-BR". */
  language: string;
  /** Procurar atualização sozinho (só avisa; nunca instala sozinho). */
  autoUpdateCheck: boolean;
}

/** Release mais nova encontrada no GitHub. */
export interface UpdateInfo {
  current: string;
  version: string;
  tag: string;
  name: string;
  notes: string;
  publishedAt: string;
  pageUrl: string;
  size: number;
  hasAsset: boolean;
}

export type SaveItem = { id: string; format: string };
