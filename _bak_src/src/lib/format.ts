import { convertFileSrc } from "@tauri-apps/api/core";
import type { Entry } from "./types";

/** Arquivo local (ícone de app, capa de link) servido pelo protocolo de asset. */
export const fileUrl = (path?: string | null) =>
  path ? convertFileSrc(path.replace(/\\/g, "/")) : "";

/**
 * Imagem/miniatura que mora **dentro do banco cifrado**: o backend serve em
 * `http://rayclip.localhost/img/<id>`. `v` só serve para o navegador não
 * reaproveitar a versão antiga quando a entrada muda.
 */
export const mediaUrl = (kind: "img" | "thumb", id: string, v?: number | string) =>
  `http://rayclip.localhost/${kind}/${id}${v ? `?v=${v}` : ""}`;

/** Compatibilidade: caminho local vai pelo asset protocol. */
export const imageUrl = fileUrl;

export function displayTitle(e: Entry): string {
  if (e.title) return e.title;
  if (e.kind === "image") {
    return `Image (${new Date(e.createdAt).toLocaleString("en-US")})`;
  }
  return "Untitled";
}

export function formatBytes(n: number): string {
  if (!n) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function timeLabel(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
}

export function shortPath(p?: string | null): string {
  if (!p) return "—";
  const parts = p.split(/[\\/]/);
  if (parts.length <= 3) return p;
  return `~/${parts.slice(-3, -1).join("/")}/`;
}

function startOfDay(d: Date) {
  const c = new Date(d);
  c.setHours(0, 0, 0, 0);
  return c.getTime();
}

export type Bucket = { label: string; entries: Entry[] };

/** Same grouping Raycast uses: Pinned, Today, Yesterday, This Week, This Month, Earlier. */
export function groupEntries(entries: Entry[]): Bucket[] {
  const pinned = entries.filter((e) => e.pinned);
  const rest = entries.filter((e) => !e.pinned);
  const now = Date.now();
  const today = startOfDay(new Date(now));
  const day = 86_400_000;

  const buckets: Bucket[] = [
    { label: "Pinned", entries: pinned },
    { label: "Today", entries: [] },
    { label: "Yesterday", entries: [] },
    { label: "This Week", entries: [] },
    { label: "This Month", entries: [] },
    { label: "Earlier", entries: [] },
  ];
  const todayDate = new Date();
  const weekStart = today - todayDate.getDay() * day;

  for (const e of rest) {
    if (e.createdAt >= today) buckets[1].entries.push(e);
    else if (e.createdAt >= today - day) buckets[2].entries.push(e);
    else if (e.createdAt >= weekStart) buckets[3].entries.push(e);
    else if (
      new Date(e.createdAt).getMonth() === todayDate.getMonth() &&
      new Date(e.createdAt).getFullYear() === todayDate.getFullYear()
    )
      buckets[4].entries.push(e);
    else buckets[5].entries.push(e);
  }
  return buckets.filter((b) => b.entries.length > 0);
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

export function matchesQuery(e: Entry, query: string): boolean {
  if (!query.trim()) return true;
  const q = query.toLowerCase();
  return (
    displayTitle(e).toLowerCase().includes(q) ||
    (e.text ?? "").toLowerCase().includes(q) ||
    (e.paths ?? []).join(" ").toLowerCase().includes(q) ||
    (e.source ?? "").toLowerCase().includes(q) ||
    (e.ocrText ?? "").toLowerCase().includes(q)
  );
}

/** True quando a busca só casou com o texto lido dentro da imagem. */
export function matchedOnlyInOcr(e: Entry, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q || !e.ocrText) return false;
  const visible =
    displayTitle(e).toLowerCase().includes(q) ||
    (e.text ?? "").toLowerCase().includes(q) ||
    (e.paths ?? []).join(" ").toLowerCase().includes(q) ||
    (e.source ?? "").toLowerCase().includes(q);
  return !visible && e.ocrText.toLowerCase().includes(q);
}
