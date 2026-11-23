import { useEffect, useMemo, useState } from "react";
import type { Entry, SaveItem } from "../lib/types";
import { Dropdown, type Option } from "./Dropdown";
import * as api from "../api";
import { FileIcon, ImageIcon, TextIcon } from "../lib/icons";

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

/** Lista de entradas selecionadas com o formato de cada uma antes de salvar. */
export function SaveManyDialog({
  entries,
  onClose,
  onDone,
}: {
  entries: Entry[];
  onClose: () => void;
  onDone: (msg: string, kind?: "ok" | "err") => void;
}) {
  const [formats, setFormats] = useState<Record<string, string>>(() =>
    Object.fromEntries(entries.map((e) => [e.id, defaultFormat(e)]))
  );
  const [custom, setCustom] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    const opts: Option<string>[] = [{ value: ALL, label: "Same format…" }];
    if (kinds.hasImage) {
      for (const f of IMAGE_FORMATS) {
        opts.push({ value: `image|${f}`, label: `Images → ${f.toUpperCase()}` });
      }
      opts.push({ value: `image|${CUSTOM}`, label: "Images → Custom…" });
    }
    if (kinds.hasText) {
      for (const f of ["txt", "md", "json", "csv", "html", "rtf", "url"]) {
        opts.push({ value: `text|${f}`, label: `Text → ${f.toUpperCase()}` });
      }
      opts.push({ value: `text|${CUSTOM}`, label: "Text → Custom…" });
    }
    return opts;
  }, [kinds]);

  // Esc fecha; as setas/Enter pertencem ao diálogo (o resto do app fica quieto).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) {
        e.preventDefault();
        e.stopImmediatePropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [busy, onClose]);

  const save = async () => {
    if (invalid) {
      setError("Type an extension for the highlighted items");
      return;
    }
    setBusy(true);
    setError(null);
    const items: SaveItem[] = rows.map((r) => ({ id: r.entry.id, format: r.ext }));
    try {
      const written = await api.saveMany(items);
      if (!written) {
        onDone("Save cancelled");
      } else {
        const folder = written[0].slice(0, Math.max(0, written[0].lastIndexOf("\\")));
        onDone(`${written.length} file${written.length > 1 ? "s" : ""} saved to ${folder}`);
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
      <div className="save-many" role="dialog" aria-label="Save selected items">
        <div className="sm-head">
          <div className="sm-title">Save {entries.length} items</div>
          <div className="sm-sub">
            Pick a format for each entry — images keep animation, text is written as-is.
          </div>
          <div className="sm-all">
            <span className="sm-all-label">Apply to all</span>
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
              onClick={() => applyAll(defaultFormat(entries[0]))}
              title="Set every row to the same format"
            >
              Reset
            </button>
          </div>
        </div>

        <div className="sm-list">
          {rows.map(({ entry, chosen, ext }) => {
            const opts: Option<string>[] = [
              ...formatsFor(entry).map((f) => ({ value: f, label: f.toUpperCase() })),
              { value: CUSTOM, label: "Custom…" },
            ];
            return (
              <div className="sm-row" key={entry.id} data-invalid={!ext}>
                <span className={`sm-icon${isImage(entry) ? " image" : ""}`}>
                  {isImage(entry) ? (
                    <ImageIcon size={14} />
                  ) : entry.paths.length ? (
                    <FileIcon size={14} />
                  ) : (
                    <TextIcon size={14} />
                  )}
                </span>
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
                      if (ev.key === "Enter") void save();
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

        {error && <div className="sm-error">{error}</div>}

        <div className="sm-foot">
          <span className="sm-hint">Files are written to the folder you pick next.</span>
          <button type="button" className="btn" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button type="button" className="btn primary" onClick={() => void save()} disabled={busy}>
            {busy ? "Saving…" : "Choose folder & save"}
          </button>
        </div>
      </div>
    </div>
  );
}
