import type { Entry } from "./types";
import { localeOf, type Lang, type T } from "./i18n";

/**
 * Arquivo derivado em `media/` (ícone do app de origem) servido pelo nosso
 * próprio protocolo: `media/src-opera.png` -> `/src/src-opera.png`.
 *
 * Antes usávamos o **asset protocol** do Tauri, que precisa de um escopo de
 * disco — o nosso era `**`, ou seja, a interface podia pedir qualquer arquivo
 * da máquina. Aqui só existe uma pasta, sem caminho e sem `..`.
 */
export const fileUrl = (path?: string | null) => {
  if (!path) return "";
  const nome = path.split(/[\\/]/).pop() || "";
  return nome ? `http://rayclip.localhost/src/${encodeURIComponent(nome)}` : "";
};

/**
 * Imagem/miniatura que mora **dentro do banco cifrado**: o backend serve em
 * `http://rayclip.localhost/img/<id>`. `v` só serve para o navegador não
 * reaproveitar a versão antiga quando a entrada muda.
 */
export const mediaUrl = (kind: "img" | "thumb", id: string, v?: number | string) =>
  `http://rayclip.localhost/${kind}/${id}${v ? `?v=${v}` : ""}`;

/** Compatibilidade: caminho local vai pelo asset protocol. */
export const imageUrl = fileUrl;

export function displayTitle(e: Entry, t?: T, lang: Lang = "en"): string {
  if (e.title) return e.title;
  if (e.kind === "image") {
    const when = new Date(e.createdAt).toLocaleString(localeOf(lang));
    return t ? t("Image ({when})", { when }) : `Image (${when})`;
  }
  return t ? t("Untitled") : "Untitled";
}

export function formatBytes(n: number): string {
  if (!n) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function timeLabel(ts: number, lang: Lang = "en"): string {
  const d = new Date(ts);
  return d.toLocaleTimeString(localeOf(lang), { hour: "numeric", minute: "2-digit" });
}

export function shortPath(p?: string | null): string {
  if (!p) return "—";
  const parts = p.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 2) return p;
  // Antes devolvia so as pastas e comia o NOME do arquivo, que e justamente
  // o que o usuario procura na lista.
  return `…/${parts.slice(-2).join("/")}`;
}

// Minusculas e sem acento: "Café São Paulo" casa com "cafe sao".
export function fold(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function startOfDay(d: Date) {
  const c = new Date(d);
  c.setHours(0, 0, 0, 0);
  return c.getTime();
}

export type Bucket = { label: string; entries: Entry[] };

/** Same grouping Raycast uses: Pinned, Today, Yesterday, This Week, This Month, This Year, Older. */
export function groupEntries(entries: Entry[], t?: T): Bucket[] {
  const pinned = entries.filter((e) => e.pinned);
  const rest = entries.filter((e) => !e.pinned);
  const now = new Date();
  const today = startOfDay(now);
  // setDate em vez de subtrair milissegundos: horario de verao nao muda o dia.
  const dayStart = (n: number) => {
    const c = new Date(now);
    c.setHours(0, 0, 0, 0);
    c.setDate(c.getDate() - n);
    return c.getTime();
  };
  const yesterday = dayStart(1);
  const weekStart = dayStart(7);
  const month = now.getMonth();
  const year = now.getFullYear();

  const todayEntries: Entry[] = [];
  const yday: Entry[] = [];
  const week: Entry[] = [];
  const mo: Entry[] = [];
  const yr: Entry[] = [];
  const older: Entry[] = [];

  for (const e of rest) {
    const d = new Date(e.createdAt);
    if (e.createdAt >= today) todayEntries.push(e);
    else if (e.createdAt >= yesterday) yday.push(e);
    else if (e.createdAt >= weekStart) week.push(e);
    else if (d.getMonth() === month && d.getFullYear() === year) mo.push(e);
    else if (d.getFullYear() === year) yr.push(e);
    else older.push(e);
  }

  const buckets: Bucket[] = [
    { label: "Pinned", entries: pinned },
    { label: "Today", entries: todayEntries },
    { label: "Yesterday", entries: yday },
    { label: "This Week", entries: week },
    { label: "This Month", entries: mo },
    { label: "This Year", entries: yr },
    { label: "Older", entries: older },
  ];
  return buckets
    .filter((x) => x.entries.length > 0)
    .map((b) => ({ ...b, label: t ? t(b.label) : b.label }));
}
export function matchesFilter(e: Entry, filter: string): boolean {
  switch (filter) {
    case "all":
      return true;
    case "text":
      return e.kind === "text";
    case "image":
      return e.kind === "image";
    case "file":
      return ["file", "files", "pdf", "video", "audio", "archive"].includes(e.kind);
    case "link":
      return e.kind === "link";
    case "email":
      return e.kind === "email";
    case "color":
      return e.kind === "color";
    default:
      return true;
  }
}

/**
 * Busca: o **Rust** decide (§8.44, `search.rs`); estas duas funções ficaram como
 * referência das regras e para o selo "só OCR" na lista. Por isso usam `title`
 * cru e não o rótulo sintético "Image (data)" — o Rust não tem como montar esse
 * rótulo (ele muda com o idioma) e procurar "image" casando com toda imagem era
 * ruído.
 */
export function matchesQuery(e: Entry, query: string): boolean {
  if (!query.trim()) return true;
  const q = fold(query);
  return (
    fold(e.title ?? "").includes(q) ||
    fold(e.text ?? "").includes(q) ||
    fold((e.paths ?? []).join(" ")).includes(q) ||
    fold(e.source ?? "").includes(q) ||
    fold(e.ocrText ?? "").includes(q)
  );
}

/** True quando a busca só casou com o texto lido dentro da imagem. */
export function matchedOnlyInOcr(e: Entry, query: string): boolean {
  const q = fold(query.trim());
  if (!q || !e.ocrText) return false;
  const visible =
    fold(e.title ?? "").includes(q) ||
    fold(e.text ?? "").includes(q) ||
    fold((e.paths ?? []).join(" ")).includes(q) ||
    fold(e.source ?? "").includes(q);
  return !visible && fold(e.ocrText).includes(q);
}

/**
 * Posição do casamento dentro de um campo:
 * 0 = igual, 1 = começa com, 2 = começa uma palavra, 3 = no meio, 9 = não achou.
 */
const WORD_START = /[\s\-_/\\.,;:()[\]{}'"<>@#$%&*+=!?|~^`]/;

function hitRank(hay: string, q: string): number {
  if (!hay) return 9;
  const i = hay.indexOf(q);
  if (i < 0) return 9;
  if (i === 0) return hay.length === q.length ? 0 : 1;
  return WORD_START.test(hay[i - 1]) ? 2 : 3;
}

/**
 * Ordem dos resultados de busca (o que separa "parece natural" de clone):
 * 1. texto/vínculo antes de imagem que só casou pelo OCR;
 * 2. dentro do tipo, fixados primeiro;
 * 3. casamento exato, depois início, depois começo de palavra, depois meio;
 * 4. recência (a lista de entrada já vem da mais recente).
 */
export function rankEntries(entries: Entry[], query: string): Entry[] {
  const q = fold(query.trim());
  if (!q) return entries;
  const score = (e: Entry) => {
    const fields = [
      fold(displayTitle(e)),
      fold(e.text ?? ""),
      fold((e.paths ?? []).join(" ")),
      fold(e.source ?? ""),
    ];
    const visible = fields.reduce((best, f) => Math.min(best, hitRank(f, q)), 9);
    // Só casou no texto lido de dentro da imagem → fila de trás.
    const tier = visible === 9 ? 1 : 0;
    const rank = tier === 0 ? visible : hitRank(fold(e.ocrText ?? ""), q);
    return tier * 1000 + (e.pinned ? 0 : 100) + rank * 10;
  };
  // sort é estável: empate mantém a ordem por recência.
  return [...entries].sort((a, b) => score(a) - score(b));
}
