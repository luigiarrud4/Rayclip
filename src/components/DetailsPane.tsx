import { useEffect, useRef, useState } from "react";
import type { Entry } from "../lib/types";
import { entryText, linkWebMeta, ocrDisponivel } from "../api";
import { displayTitle, fileUrl, formatBytes, mediaUrl, shortPath } from "../lib/format";
import { FileIcon, FolderIcon } from "../lib/icons";
import type { T } from "../lib/i18n";
import { kindLabel } from "../lib/i18n";
import { SiteIcon } from "./SiteIcon";
import { ZoomableImage } from "./ImageViewer";

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
  onOpenImage,
  zoomArmed = false,
  onZoomArmedChange,
  t,
}: {
  entry: Entry | null;
  selection?: string[];
  entries?: Entry[];
  /** Abre o visualizador grande (Quick Look) da imagem selecionada. */
  onOpenImage?: (entry: Entry) => void;
  /** Scroll dá zoom na prévia só depois que o usuário clica na imagem. */
  zoomArmed?: boolean;
  onZoomArmedChange?: (armed: boolean) => void;
  t: T;
}) {
  // A lista carrega só os primeiros 16 KB de cada texto (peso do IPC); aqui
  // buscamos o texto integral da entrada selecionada.
  const [fullText, setFullText] = useState<string | null>(null);
  // Capa do link: se o arquivo sumiu (cache limpo, banco restaurado), a imagem
  // quebra e a prévia fica vazia — aí entra o bloco com o ícone do site.
  const [coverOk, setCoverOk] = useState(true);
  // Uma tentativa por entrada e por sessão: o Chromium escondido custa um
  // render, então nada de repetir a cada seleção. Só dispara com a entrada
  // **aberta** aqui no painel e sem capa — nunca em segundo plano.
  const pedidasWeb = useRef<Set<string>>(new Set());
  // "Nenhum texto" só é verdade quando o Windows tem OCR de verdade; numa
  // máquina sem o componente (VM enxuta) tudo cai como "sem texto".
  const [temOcr, setTemOcr] = useState(true);
  useEffect(() => {
    let vivo = true;
    void ocrDisponivel().then((ok) => {
      if (vivo) setTemOcr(ok);
    });
    return () => {
      vivo = false;
    };
  }, []);
  const cover = entry?.link?.imagePath
    ? entry.link.imagePath.startsWith("data:") || entry.link.imagePath.startsWith("http")
      ? entry.link.imagePath
      : fileUrl(entry.link.imagePath)
    : "";
  useEffect(() => setCoverOk(true), [entry?.id, cover]);

  useEffect(() => {
    const id = entry?.id;
    if (!id || entry?.kind !== "link" || cover) return;
    if (pedidasWeb.current.has(id)) return;
    // Espera a seleção parar: passar pelas linhas com as setas não deve
    // disparar um render por linha.
    const t = window.setTimeout(() => {
      pedidasWeb.current.add(id);
      linkWebMeta(id).catch(() => undefined);
    }, 700);
    return () => window.clearTimeout(t);
  }, [entry?.id, entry?.kind, cover]);
  // Rede de segurança: clicar fora da imagem sai do modo zoom (senão o scroll
  // das outras partes do painel ficaria preso no zoom).
  const shotRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!zoomArmed) return;
    const onDown = (e: PointerEvent) => {
      if (!shotRef.current?.contains(e.target as Node)) onZoomArmedChange?.(false);
    };
    window.addEventListener("pointerdown", onDown, true);
    return () => window.removeEventListener("pointerdown", onDown, true);
  }, [zoomArmed, onZoomArmedChange]);
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
        <div className="details-empty">{t("Select an entry to see details")}</div>
      </aside>
    );
  }

  if (selection.length > 1) {
    const chosen = selection
      .map((id) => entries.find((e) => e.id === id))
      .filter((e): e is Entry => Boolean(e));
    return (
      <aside className="details">
        <div className="section-title">{t("Selected · {n} items", { n: selection.length })}</div>
        <div className="multi-list">
          {chosen.map((e, i) => (
            <div className="multi-item" key={e.id}>
              <span className="order-badge">{i + 1}</span>
              <span className="multi-kind">{kindLabel(e.kind, t)}</span>
              <span className="multi-title">{displayTitle(e, t)}</span>
            </div>
          ))}
        </div>
        <div className="settings-hint">
          {t(
            "Press {enter} to paste them in this order, or {esc} to clear the selection. Ctrl+click toggles an item, Shift+click takes a range.",
            { enter: "Enter", esc: "Esc" }
          )}
        </div>
      </aside>
    );
  }

  const isText = ["text", "link", "email"].includes(entry.kind);
  // Vídeo/áudio só toca se a entrada ainda aponta para um arquivo do disco
  // (a URL `rayclip://video/<id>` é resolvida no Rust a partir do banco).
  const MIDIA = /\.(mp4|m4v|mov|webm|mkv|avi|ogv|mp3|wav|flac|m4a|aac|ogg|oga)$/i;
  const temMidia =
    (entry.kind === "video" || entry.kind === "audio") &&
    entry.paths.length > 0 &&
    MIDIA.test(entry.paths[0]);
  const text = fullText ?? entry.text ?? "";
  const color = entry.kind === "color" ? entry.text?.trim() ?? "" : "";

  return (
    <aside className="details">
      {entry.kind === "image" && entry.hasImage && (
        <div
          className="preview preview-zoomable"
          title={t("Click the image to zoom")}
          ref={shotRef}
        >
          {/* Rolagem só dá zoom **depois** de clicar na imagem: antes disso o
              scroll continua rolando esta coluna normalmente. */}
          <ZoomableImage
            src={mediaUrl("img", entry.id, entry.createdAt)}
            armed={zoomArmed}
            onArmedChange={onZoomArmedChange}
            t={t}
          />
          {entry.gif && <span className="preview-badge">GIF</span>}
          <button
            type="button"
            className="preview-open"
            title={t("Open the big preview")}
            onClick={() => {
              onZoomArmedChange?.(false);
              onOpenImage?.(entry);
            }}
          >
            ⤢
          </button>
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

      {temMidia && (
        <div className="preview preview-media">
          {entry.kind === "video" ? (
            <video
              key={entry.id}
              className="media-player"
              // `preload="metadata"`: mostra a primeira imagem e a duração sem
              // baixar o arquivo inteiro. `muted` porque abrir uma entrada não
              // é pedido para tocar som no meio do trabalho.
              preload="metadata"
              controls
              muted
              playsInline
              controlsList="nodownload"
              poster={entry.hasThumb ? mediaUrl("thumb", entry.id, entry.createdAt) : undefined}
              src={mediaUrl("video", entry.id, entry.createdAt)}
            >
              <span className="media-falhou">{t("This format cannot be played here")}</span>
            </video>
          ) : (
            <audio
              key={entry.id}
              className="media-player audio"
              preload="metadata"
              controls
              muted
              src={mediaUrl("audio", entry.id, entry.createdAt)}
            />
          )}
          <span className="media-hint">{t("Muted by default")}</span>
        </div>
      )}

      {color && (
        <div className="preview">
          <div className="preview-swatch" style={{ background: color }} />
        </div>
      )}

      {entry.kind === "link" && (
        <div className="link-card">
          {cover && coverOk ? (
            entry.link?.coverVideo ? (
              // Página de GIF que entrega o loop em mp4/webm: toca mudo e em
              // repetição — é o que o Discord faz, e um mp4 de 2 MB toca sem o
              // custo de decodificar 20 MB de GIF.
              <video
                className="link-cover"
                key={cover}
                src={cover}
                autoPlay
                muted
                loop
                playsInline
                aria-hidden
                onError={() => setCoverOk(false)}
              />
            ) : (
              <img
                className="link-cover"
                src={cover}
                alt=""
                draggable={false}
                onError={() => setCoverOk(false)}
              />
            )
          ) : (
            // Sem capa (página sem og:image, arquivo de CDN, arquivo apagado):
            // a prévia não pode virar um retângulo vazio — mostra o site.
            <div className="link-ph">
              <SiteIcon entry={entry} />
              <span>{entry.link?.siteName ?? hostOf(entry.text ?? "")}</span>
            </div>
          )}
          <div className="link-card-body">
            <div className="link-site">
              <SiteIcon entry={entry} />
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
        {entry.kind === "image" ? t("Metadata") : t("Information")}
      </div>

      <div className="kv">
        <span className="k">{t("Source")}</span>
        <span className="v" title={entry.source ?? t("Unknown")}>
          <span className="source-app">
            {entry.sourceIcon && (
              <img src={sourceIconSrc(entry.sourceIcon)!} alt="" draggable={false} />
            )}
            <span>{entry.source ?? t("Unknown")}</span>
          </span>
        </span>
      </div>

      {entry.kind === "image" && (
        <>
          <Row
            k={t("Dimensions")}
            v={
              entry.imageWidth && entry.imageHeight
                ? `${entry.imageWidth} × ${entry.imageHeight}`
                : "—"
            }
          />
          <Row
            k={t("Type")}
            v={t(entry.gif ? "GIF Image" : "PNG Image")}
          />
          <Row k={t("Size")} v={formatBytes(entry.sizeBytes)} />
        </>
      )}

      {entry.kind === "link" && entry.text && (
        <>
          <Row k={t("Type")} v={t("Link")} />
          <Row k={t("URL")} v={entry.text} wrap />
          <Row k={t("Domain")} v={hostOf(entry.text)} />
        </>
      )}

      {entry.kind === "email" && (
        <>
          <Row k={t("Type")} v={t("Email")} />
          <Row k={t("Address")} v={entry.text ?? ""} wrap />
        </>
      )}

      {entry.kind === "color" && (
        <>
          <Row k={t("Type")} v={t("Color")} />
          <Row k="Hex" v={(entry.text ?? "").trim().toUpperCase()} />
          <Row k="RGB" v={hexToRgb(color)} />
        </>
      )}

      {entry.kind === "text" && (
        <>
          <Row k={t("Type")} v={t("Text")} />
          {/* length conta unidades UTF-16: um emoji vira 2. */}
          <Row k={t("Characters")} v={String([...(entry.text ?? "")].length)} />
          <Row
            k={t("Words")}
            v={String(
              (entry.text ?? "")
                .trim()
                .split(/\s+/)
                .filter(Boolean).length,
            )}
          />
          <Row
            k={t("Lines")}
            v={String(entry.text ? entry.text.split("\n").length : 0)}
          />
        </>
      )}

      {entry.paths.length > 0 && (
        <>
          <Row
            k={t("Type")}
            v={entry.paths.length > 1 ? `${entry.paths.length} ${t("Files")}` : t("File")}
          />
          {entry.paths.length === 1 && (
            <Row k={t("Name")} v={displayTitle(entry, t)} wrap />
          )}
          <Row k={t("Where")} v={shortPath(entry.paths[0])} wrap />
          <Row k={t("Size")} v={formatBytes(entry.sizeBytes)} />
        </>
      )}

      {entry.kind === "image" && (
        <section className="ocr-section">
          <div className="section-title">{t("Text in Image")}</div>
          {entry.ocrText ? (
            <pre className="ocr-text">{entry.ocrText}</pre>
          ) : (
            <div className="ocr-empty">
              {temOcr ? t("No text detected") : t("OCR unavailable on this Windows")}
            </div>
          )}
        </section>
      )}
    </aside>
  );
}
