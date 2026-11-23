import { useEffect, useState } from "react";
import type { Entry } from "../lib/types";
import { entryText } from "../api";
import { displayTitle, fileUrl, formatBytes, mediaUrl, shortPath } from "../lib/format";
import { FileIcon, FolderIcon } from "../lib/icons";

function hexToRgb(hex: string) {
  const h = hex.replace("#", "");
  const full =
    h.length === 3
      ? h
          .split("")
          .map((c) => c + c)
          .join("")
      : h.slice(0, 6);
  const n = parseInt(full, 16);
  if (Number.isNaN(n)) return "";
  return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
}

function sourceIconSrc(path?: string | null) {
  return path ? fileUrl(path) : null;
}

function Row({ k, v, wrap }: { k: string; v: string; wrap?: boolean }) {
  return (
    <div className="kv">
      <span className="k">{k}</span>
      <span className="v" data-wrap={wrap} title={v}>
        {v}
      </span>
    </div>
  );
}

function hostOf(url: string) {
  try {
    return new URL(url.startsWith("http") ? url : `https://${url}`).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function DetailsPane({
  entry,
  selection = [],
  entries = [],
}: {
  entry: Entry | null;
  selection?: string[];
  entries?: Entry[];
}) {
  // A lista carrega só os primeiros 16 KB de cada texto (peso do IPC); aqui
  // buscamos o texto integral da entrada selecionada.
  const [fullText, setFullText] = useState<string | null>(null);
  const entryId = entry?.id ?? null;
  useEffect(() => {
    setFullText(null);
    if (!entryId) return;
    let alive = true;
    entryText(entryId)
      .then((t) => {
        if (alive) setFullText(t);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [entryId]);

  if (!entry) {
    return (
      <aside className="details">
        <div className="details-empty">Select an entry to see details</div>
      </aside>
    );
  }

  if (selection.length > 1) {
    const chosen = selection
      .map((id) => entries.find((e) => e.id === id))
      .filter((e): e is Entry => Boolean(e));
    return (
      <aside className="details">
        <div className="section-title">Selected · {selection.length} items</div>
        <div className="multi-list">
          {chosen.map((e, i) => (
            <div className="multi-item" key={e.id}>
              <span className="order-badge">{i + 1}</span>
              <span className="multi-kind">{e.kind}</span>
              <span className="multi-title">{displayTitle(e)}</span>
            </div>
          ))}
        </div>
        <div className="settings-hint">
          Press <strong>Enter</strong> to paste them in this order, or
          <strong> Esc</strong> to clear the selection. Ctrl+click toggles an item,
          Shift+click takes a range.
        </div>
      </aside>
    );
  }

  const isText = ["text", "link", "email"].includes(entry.kind);
  const text = fullText ?? entry.text ?? "";
  const color = entry.kind === "color" ? entry.text?.trim() ?? "" : "";

  return (
    <aside className="details">
      {entry.kind === "image" && entry.hasImage && (
        <div className="preview">
          <img
            src={mediaUrl("img", entry.id, entry.createdAt)}
            alt=""
            draggable={false}
          />
          {entry.gif && <span className="preview-badge">GIF</span>}
        </div>
      )}

      {entry.kind !== "image" && !color && !isText && entry.hasThumb && (
        <div className="preview">
          <img
            src={mediaUrl("thumb", entry.id, entry.createdAt)}
            alt=""
            draggable={false}
          />
        </div>
      )}

      {color && (
        <div className="preview">
          <div className="preview-swatch" style={{ background: color }} />
        </div>
      )}

      {entry.kind === "link" && (
        <div className="link-card">
          {entry.link?.imagePath && (
            <img
              className="link-cover"
              src={
                entry.link.imagePath.startsWith("data:")
                  ? entry.link.imagePath
                  : fileUrl(entry.link.imagePath)
              }
              alt=""
              draggable={false}
            />
          )}
          <div className="link-card-body">
            <div className="link-site">
              {entry.link?.icon ? (
                <img className="favicon" src={entry.link.icon} alt="" draggable={false} />
              ) : (
                <span className="link-site-dot" />
              )}
              <span>{entry.link?.siteName ?? hostOf(entry.text ?? "")}</span>
            </div>
            <div className="link-title">{entry.link?.title ?? entry.text ?? ""}</div>
            {entry.link?.description && (
              <div className="link-desc">{entry.link.description}</div>
            )}
          </div>
        </div>
      )}

      {isText && entry.kind !== "link" && text && (
        <div className="preview">
          <pre className="preview-text">{text}</pre>
        </div>
      )}

      {entry.paths.length > 0 && (
        <div className="preview">
          <div className="preview-files">
            {entry.paths.slice(0, 5).map((p) => {
              const name = p.split(/[\\/]/).pop() ?? p;
              const isDir = !name.includes(".");
              return (
                <div className="preview-file" key={p} title={p}>
                  {isDir ? <FolderIcon size={14} /> : <FileIcon size={14} />}
                  <span>{name}</span>
                </div>
              );
            })}
            {entry.paths.length > 5 && (
              <div className="preview-file">
                <span style={{ opacity: 0.6 }}>+{entry.paths.length - 5} more</span>
              </div>
            )}
          </div>
        </div>
      )}

      <div className="section-title">
        {entry.kind === "image" ? "Metadata" : "Information"}
      </div>

      <div className="kv">
        <span className="k">Source</span>
        <span className="v" title={entry.source ?? "Unknown"}>
          <span className="source-app">
            {entry.sourceIcon && (
              <img src={sourceIconSrc(entry.sourceIcon)!} alt="" draggable={false} />
            )}
            <span>{entry.source ?? "Unknown"}</span>
          </span>
        </span>
      </div>

      {entry.kind === "image" && (
        <>
          <Row
            k="Dimensions"
            v={
              entry.imageWidth && entry.imageHeight
                ? `${entry.imageWidth} × ${entry.imageHeight}`
                : "—"
            }
          />
          <Row
            k="Type"
            v={entry.gif ? "GIF Image" : "PNG Image"}
          />
          <Row k="Size" v={formatBytes(entry.sizeBytes)} />
        </>
      )}

      {entry.kind === "link" && entry.text && (
        <>
          <Row k="Type" v="Link" />
          <Row k="URL" v={entry.text} wrap />
          <Row k="Domain" v={hostOf(entry.text)} />
        </>
      )}

      {entry.kind === "email" && (
        <>
          <Row k="Type" v="Email" />
          <Row k="Address" v={entry.text ?? ""} wrap />
        </>
      )}

      {entry.kind === "color" && (
        <>
          <Row k="Type" v="Color" />
          <Row k="Hex" v={(entry.text ?? "").trim().toUpperCase()} />
          <Row k="RGB" v={hexToRgb(color)} />
        </>
      )}

      {entry.kind === "text" && (
        <>
          <Row k="Type" v="Text" />
          <Row k="Characters" v={String((entry.text ?? "").length)} />
          <Row k="Lines" v={String((entry.text ?? "").split("\n").length)} />
        </>
      )}

      {entry.paths.length > 0 && (
        <>
          <Row
            k="Type"
            v={entry.paths.length > 1 ? `${entry.paths.length} Files` : "File"}
          />
          {entry.paths.length === 1 && (
            <Row k="Name" v={displayTitle(entry)} wrap />
          )}
          <Row k="Where" v={shortPath(entry.paths[0])} wrap />
          <Row k="Size" v={formatBytes(entry.sizeBytes)} />
        </>
      )}

      {entry.kind === "image" && (
        <section className="ocr-section">
          <div className="section-title">Text in Image</div>
          {entry.ocrText ? (
            <pre className="ocr-text">{entry.ocrText}</pre>
          ) : (
            <div className="ocr-empty">No text detected</div>
          )}
        </section>
      )}
    </aside>
  );
}
