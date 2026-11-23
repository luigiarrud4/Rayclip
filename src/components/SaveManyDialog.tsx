import { useEffect, useMemo, useState } from "react";
import type { Entry, SaveItem } from "../lib/types";
import { Dropdown, type Option } from "./Dropdown";
import * as api from "../api";
import { FileIcon, TextIcon } from "../lib/icons";
import { mediaUrl } from "../lib/format";
import { ZoomableImage } from "./ImageViewer";
import { localeOf, type Lang } from "../lib/i18n";

const IMAGE_FORMATS = ["png", "jpg", "webp", "bmp", "gif"];
const TEXT_FORMATS = [
  "txt",
  "md",
  "json",
  "csv",
  "html",
  "rtf",
  "url",
  "lua",
  "js",
  "ts",
  "py",
  "css",
  "xml",
  "yml",
  "ini",
  "log",
  "sql",
  "bat",
  "ps1",
];

const CUSTOM = "__custom";
const ALL = "__all";

function isImage(e: Entry) {
  return !!e.hasImage;
}

function formatsFor(e: Entry): string[] {
  if (isImage(e)) return IMAGE_FORMATS;
  const own = e.paths[0]?.split(".").pop()?.toLowerCase();
  return own && own.length <= 5 ? [own, ...TEXT_FORMATS] : TEXT_FORMATS;
}

function defaultFormat(e: Entry): string {
  if (isImage(e)) return e.gif ? "gif" : "png";
  if (e.kind === "link") return "url";
  return "txt";
}

function label(e: Entry, max = 44) {
  const raw = (e.title || e.text || e.paths[0] || e.kind).replace(/\s+/g, " ").trim();
  return raw.length > max ? raw.slice(0, max) + "…" : raw;
}

/** O que a miniatura pode mostrar desta entrada (nenhum, imagem ou texto). */
function peekKind(e: Entry): "image" | "text" | null {
  if (e.hasImage) return "image";
  if ((e.text && e.text.trim()) || e.paths.length) return "text";
  return null;
}

function peekText(e: Entry): string {
  const raw = e.text?.trim() ? e.text : e.paths.join("\n");
  return raw.slice(0, 1200);
}

/** Lista de entradas selecionadas com o formato de cada uma antes de salvar. */
export function SaveManyDialog({
  entries,
  onClose,
  onDone,
  t,
  lang = "en",
}: {
  entries: Entry[];
  onClose: () => void;
  onDone: (msg: string, kind?: "ok" | "err") => void;
  t: (text: string, vars?: Record<string, string | number>) => string;
  lang?: Lang;
}) {
  const [formats, setFormats] = useState<Record<string, string>>(() =>
    Object.fromEntries(entries.map((e) => [e.id, defaultFormat(e)]))
  );
  const [custom, setCustom] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Prévia em **lugar fixo** (estilo Quick Look): uma coluna fixa do lado
  // direito mostra a entrada sob o mouse, e `Espaço` abre a ampliação grande
  // centralizada — nada segue o ponteiro, nada fica por cima do cabeçalho.
  const [previewId, setPreviewId] = useState<string | null>(entries[0]?.id ?? null);
  const [zoom, setZoom] = useState(false);
  const previewEntry =
    entries.find((e) => e.id === previewId) ?? entries[0] ?? null;

  const rows = useMemo(
    () =>
      entries.map((e) => {
        const chosen = formats[e.id] ?? defaultFormat(e);
        const customText = custom[e.id] ?? "";
        const ext =
          chosen === CUSTOM
            ? customText.trim().replace(/^\./, "").replace(/[^A-Za-z0-9]/g, "").slice(0, 8)
            : chosen;
        return { entry: e, chosen, ext };
      }),
    [entries, formats, custom]
  );

  const invalid = rows.some((r) => !r.ext);

  // "Apply to all": um formato por tipo (imagens / texto), já que a seleção
  // costuma ser mista e os formatos não se cruzam.
  const kinds = useMemo(() => {
    const hasImage = entries.some((e) => isImage(e));
    const hasText = entries.some((e) => !isImage(e));
    return { hasImage, hasText };
  }, [entries]);

  const applyAll = (value: string) => {
    const [kind, fmt] = value.split("|");
    setFormats((prev) => {
      const next = { ...prev };
      for (const e of entries) {
        if (kind === "image" && isImage(e)) next[e.id] = fmt;
        if (kind === "text" && !isImage(e)) next[e.id] = fmt;
      }
      return next;
    });
    if (fmt === CUSTOM) {
      setCustom((prev) => {
        const next = { ...prev };
        for (const e of entries) {
          if (kind === "image" && isImage(e)) delete next[e.id];
          if (kind === "text" && !isImage(e)) delete next[e.id];
        }
        return next;
      });
    }
  };

  const applyOptions: Option<string>[] = useMemo(() => {
    const opts: Option<string>[] = [{ value: ALL, label: t("Same format…") }];
    if (kinds.hasImage) {
      for (const f of IMAGE_FORMATS) {
        opts.push({ value: `image|${f}`, label: `Images → ${f.toUpperCase()}` });
      }
      opts.push({ value: `image|${CUSTOM}`, label: t("Images → Custom…") });
    }
    if (kinds.hasText) {
      for (const f of ["txt", "md", "json", "csv", "html", "rtf", "url"]) {
        opts.push({ value: `text|${f}`, label: `Text → ${f.toUpperCase()}` });
      }
      opts.push({ value: `text|${CUSTOM}`, label: t("Text → Custom…") });
    }
    return opts;
  }, [kinds, t]);

  // Esc fecha (primeiro a ampliação, depois o diálogo); Espaço alterna a
  // ampliação estilo Quick Look; setas andam a seleção com ela aberta.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (document.activeElement as HTMLElement | null)?.tagName;
      const typing = tag === "INPUT" || tag === "TEXTAREA";
      if (e.key === "Escape" && !busy) {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (zoom) setZoom(false);
        else onClose();
        return;
      }
      if ((e.key === " " || e.code === "Space") && !typing && !busy) {
        e.preventDefault();
        e.stopImmediatePropagation();
        setZoom((z) => !z);
        return;
      }
      if (zoom && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
        e.preventDefault();
        e.stopImmediatePropagation();
        const i = entries.findIndex((x) => x.id === previewId);
        const next = e.key === "ArrowDown" ? Math.min(entries.length - 1, i + 1) : Math.max(0, i - 1);
        if (entries[next]) setPreviewId(entries[next].id);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [busy, onClose, zoom, entries, previewId]);

  const save = async () => {
    if (invalid) {
      setError(t("Type an extension for the highlighted items"));
      return;
    }
    setBusy(true);
    setError(null);
    const items: SaveItem[] = rows.map((r) => ({ id: r.entry.id, format: r.ext }));
    try {
      const written = await api.saveMany(items);
      if (!written) {
        onDone(t("Save cancelled"));
      } else {
        const folder = written[0].slice(0, Math.max(0, written[0].lastIndexOf("\\")));
        onDone(t("{n} files saved to {folder}", { n: written.length, folder }));
      }
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="save-many" role="dialog" aria-label={t("Save selected items")}>
        <div className="sm-head">
          <div className="sm-title">{t("Save {n} items", { n: entries.length })}</div>
          <div className="sm-sub">
            {t("Pick a format for each entry — images keep animation, text is written as-is.")}
          </div>
          <div className="sm-all">
            <span className="sm-all-label">{t("Apply to all")}</span>
            <Dropdown<string>
              value={ALL}
              width={168}
              options={applyOptions}
              onChange={(v) => {
                if (v !== ALL) applyAll(v);
              }}
            />
            <button
              type="button"
              className="btn sm-all-btn"
              onClick={() => {
                // Volta cada linha ao padrao DELA: aplicar o formato de
                // entries[0] em todas virava imagem .txt (e vice-versa).
                setFormats(
                  Object.fromEntries(entries.map((e) => [e.id, defaultFormat(e)])),
                );
                setCustom({});
              }}
              title={t("Set every row to the same format")}
            >
              {t("Reset")}
            </button>
          </div>
        </div>

        <div className="sm-body">
          <div className="sm-list">
          {rows.map(({ entry, chosen, ext }) => {
            const opts: Option<string>[] = [
              ...formatsFor(entry).map((f) => ({ value: f, label: f.toUpperCase() })),
              { value: CUSTOM, label: t("Custom…") },
            ];
            return (
              <div
                className="sm-row"
                key={entry.id}
                data-invalid={!ext}
                data-previewed={previewEntry?.id === entry.id || undefined}
                onMouseEnter={() => setPreviewId(entry.id)}
                onDoubleClick={() => {
                  setPreviewId(entry.id);
                  setZoom(true);
                }}
              >
                <button
                  type="button"
                  className={`sm-icon${isImage(entry) ? " image" : ""}`}
                  data-peek={peekKind(entry) ? "true" : undefined}
                  onClick={(ev) => {
                    ev.stopPropagation();
                    setPreviewId(entry.id);
                    setZoom(true);
                  }}
                  title={t("Open the big preview (Space)")}
                >
                  {isImage(entry) ? (
                    <img
                      className="sm-thumb"
                      src={mediaUrl("thumb", entry.id, entry.createdAt)}
                      alt=""
                      draggable={false}
                    />
                  ) : entry.paths.length ? (
                    <FileIcon size={14} />
                  ) : (
                    <TextIcon size={14} />
                  )}
                </button>
                <span className="sm-name" title={label(entry, 200)}>
                  {label(entry)}
                </span>
                <span className="sm-dot">.</span>
                {chosen === CUSTOM ? (
                  <input
                    className="sm-ext-input"
                    autoFocus
                    placeholder="lua"
                    value={custom[entry.id] ?? ""}
                    onChange={(ev) =>
                      setCustom((prev) => ({ ...prev, [entry.id]: ev.target.value }))
                    }
                    onKeyDown={(ev) => {
                      if (ev.key === "Enter" && !invalid) void save();
                      if (ev.key === "Escape") onClose();
                    }}
                  />
                ) : (
                  <span className="sm-ext">{ext}</span>
                )}
                <Dropdown<string>
                  value={chosen}
                  options={opts}
                  width={118}
                  onChange={(v) => setFormats((prev) => ({ ...prev, [entry.id]: v }))}
                />
              </div>
            );
          })}
          </div>

          <aside className="sm-preview" aria-label={t("Preview")}>
            {previewEntry && peekKind(previewEntry) ? (
              <>
                <div className="sm-preview-head">{label(previewEntry, 60)}</div>
                <div className="sm-preview-stage">
                  {peekKind(previewEntry) === "image" ? (
                    <img
                      className="sm-preview-img"
                      src={mediaUrl("img", previewEntry.id, previewEntry.createdAt)}
                      alt=""
                      draggable={false}
                    />
                  ) : (
                    <pre className="sm-preview-text">{peekText(previewEntry)}</pre>
                  )}
                </div>
                <div className="sm-preview-foot">
                  <span>
                    {peekKind(previewEntry) === "image"
                      ? previewEntry.gif
                        ? t("GIF · animated")
                        : t("Image")
                      : t("{n} chars", {
                          n: (previewEntry.text || "").length.toLocaleString(
                            localeOf(lang)
                          ),
                        })}
                  </span>
                  <span className="sm-preview-hint">{t("Space to enlarge")}</span>
                </div>
              </>
            ) : (
              <div className="sm-preview-empty">{t("No preview")}</div>
            )}
          </aside>
        </div>

        {error && <div className="sm-error">{error}</div>}

        <div className="sm-foot">
          <span className="sm-hint">{t("Files are written to the folder you pick next.")}</span>
          <button type="button" className="btn" onClick={onClose} disabled={busy}>
            {t("Cancel")}
          </button>
          <button
            type="button"
            className="btn primary"
            onClick={() => void save()}
            disabled={busy}
          >
            {busy ? t("Saving…") : t("Choose folder & save")}
          </button>
        </div>

        {zoom && previewEntry && peekKind(previewEntry) && (
          <div
            className="sm-zoom"
            onClick={(e) => e.target === e.currentTarget && setZoom(false)}
          >
            <div className="sm-zoom-card" role="dialog" aria-label={t("Quick look preview")}>
              <div className="sm-zoom-head">
                <span className="sm-zoom-title">{label(previewEntry, 80)}</span>
                <span className="sm-zoom-keys">{t("Space or Esc to close")}</span>
              </div>
              {peekKind(previewEntry) === "image" ? (
                <div className="sm-zoom-stage">
                  <ZoomableImage
                    src={mediaUrl("img", previewEntry.id, previewEntry.createdAt)}
                    t={t}
                  />
                </div>
              ) : (
                <pre className="sm-zoom-text">{peekText(previewEntry)}</pre>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
