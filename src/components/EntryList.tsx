import { useEffect, useMemo, useRef, useState } from "react";
import type { Entry } from "../lib/types";
import { displayTitle, mediaUrl } from "../lib/format";
import type { T } from "../lib/i18n";
import { KindIcon, PinIcon, SearchIcon } from "../lib/icons";
import { SiteIcon } from "./SiteIcon";

export interface Bucket {
  label: string;
  entries: Entry[];
}

const ROW_H = 31;
const LABEL_H = 26;
const OVERSCAN = 0.75;

/** Miniatura da linha: poster estático para GIF (não anima na lista). */
function thumbSrc(entry: Entry): string | null {
  // Sempre a miniatura: o backend gera (e guarda) uma quando a entrada não tem.
  // Antes a lista pedia a imagem inteira — 348 MB em 1.305 linhas.
  if (entry.kind === "image" && entry.hasImage) {
    return mediaUrl("thumb", entry.id, entry.createdAt);
  }
  if (entry.hasThumb) return mediaUrl("thumb", entry.id, entry.createdAt);
  return null;
}

function EntryRow({
  entry,
  selected,
  order,
  ocrHit,
  renaming,
  onRename,
  onHover,
  onClick,
  onPaste,
  onContextMenu,
  t,
}: {
  entry: Entry;
  selected: boolean;
  /** 1-based quando faz parte de uma seleção múltipla. */
  order?: number;
  ocrHit?: boolean;
  /** `true` enquanto o título desta linha está sendo editado. */
  renaming?: boolean;
  onRename?: (title: string, cancel?: boolean) => void;
  onHover: () => void;
  onClick: (mods: { ctrl: boolean; shift: boolean }) => void;
  onPaste: () => void;
  onContextMenu: () => void;
  t: T;
}) {
  const color = entry.kind === "color" ? entry.text?.trim() ?? "" : "";
  const isColor = entry.kind === "color" && /^#|^rgb/.test(color);
  const src = thumbSrc(entry);
  const renameDone = useRef(false);

  return (
    <div
      className="row"
      data-selected={selected}
      data-multi={order ? "true" : undefined}
      data-id={entry.id}
      onMouseEnter={onHover}
      onClick={(e) => onClick({ ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })}
      onDoubleClick={onPaste}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onContextMenu();
      }}
    >
      <div className="glyph" data-tile={!src && !isColor}>
        {src ? (
          <img src={src} alt="" draggable={false} loading="lazy" decoding="async" />
        ) : isColor ? (
          <span className="swatch" style={{ background: color }} />
        ) : entry.kind === "link" ? (
          // Favicon do site; sem ele (offline, cache apagado), a inicial do
          // domínio — nunca o clipe genérico.
          <SiteIcon entry={entry} />
        ) : (
          <KindIcon kind={entry.kind} />
        )}
      </div>
      {renaming ? (
        <input
          className="rename-input"
          defaultValue={entry.title || displayTitle(entry, t)}
          autoFocus
          spellCheck={false}
          draggable={false}
          onFocus={(e) => {
            renameDone.current = false;
            e.currentTarget.select();
          }}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
          onMouseDown={(e) => e.stopPropagation()}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") {
              e.preventDefault();
              renameDone.current = true;
              onRename?.(e.currentTarget.value);
            } else if (e.key === "Escape") {
              e.preventDefault();
              renameDone.current = true;
              onRename?.(e.currentTarget.value, true);
            }
          }}
          onBlur={(e) => {
            if (renameDone.current) return;
            renameDone.current = true;
            onRename?.(e.currentTarget.value);
          }}
        />
      ) : (
        <span className="title">{displayTitle(entry, t)}</span>
      )}
      {ocrHit && (
        <span className="ocr-badge" title="Matched text inside the image">
          Aa
        </span>
      )}
      {order !== undefined && (
        <span className="order-badge" title={`Paste order #${order}`}>
          {order}
        </span>
      )}
      {entry.pinned && (
        <span className="pin-dot">
          <PinIcon size={12} />
        </span>
      )}
    </div>
  );
}

export function EntryList({
  buckets,
  selectedId,
  selection,
  onSelect,
  onHover,
  onActivate,
  onPaste,
  hasEntries,
  totalEntries,
  query,
  isOcrHit,
  onContextMenu,
  renamingId,
  onRename,
  onNearEnd,
  t,
}: {
  buckets: Bucket[];
  selectedId: string | null;
  /** Entradas marcadas com Ctrl/Shift, na ordem em que serão coladas. */
  selection: string[];
  onSelect: (id: string, mods: { ctrl: boolean; shift: boolean }) => void;
  onHover: (id: string) => void;
  onActivate: (id: string) => void;
  onPaste: (id: string) => void;
  hasEntries: boolean;
  totalEntries: number;
  query: string;
  isOcrHit?: (e: Entry) => boolean;
  onContextMenu: (id: string) => void;
  /** Chamado quando a rolagem chega perto do fim (pede a próxima página). */
  onNearEnd?: () => void;
  /** Id da entrada com o título em edição (ou `null`). */
  renamingId?: string | null;
  onRename: (id: string, title: string, cancel?: boolean) => void;
  t: T;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(480);

  // Lista "achatada" com alturas fixas: permite renderizar só o que aparece.
  const items = useMemo(() => {
    const out: { label?: string; entry?: Entry; top: number; h: number }[] = [];
    let y = 0;
    for (const bucket of buckets) {
      // Bloco sem rótulo = resultado de busca, que vem em ordem de relevância.
      if (bucket.label) {
        out.push({ label: bucket.label, top: y, h: LABEL_H });
        y += LABEL_H;
      }
      for (const entry of bucket.entries) {
        out.push({ entry, top: y, h: ROW_H });
        y += ROW_H;
      }
    }
    return { rows: out, height: y };
  }, [buckets]);

  // O container `.vlist` só existe quando há entradas, então religamos o
  // listener quando a lista deixa de estar vazia (senão o scroll não atualiza).
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onScroll = () => {
      setScrollTop(el.scrollTop);
      // Faltando 3 telas de rolagem para o fim: pede mais. O `loadMore` do App
      // ignora o pedido se não houver mais nada ou se já estiver carregando.
      if (
        onNearEnd &&
        el.scrollTop + el.clientHeight >= el.scrollHeight - el.clientHeight * 3
      ) {
        onNearEnd();
      }
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    setScrollTop(el.scrollTop);
    setViewH(el.clientHeight || 480);
    const ro = new ResizeObserver(() => setViewH(el.clientHeight || 480));
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
    };
  }, [hasEntries, onNearEnd]);

  // Mantém o item selecionado visível sem renderizar o histórico inteiro.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el || !selectedId) return;
    const row = items.rows.find((r) => r.entry?.id === selectedId);
    if (!row) return;
    const top = el.scrollTop;
    const bottom = top + el.clientHeight;
    if (row.top < top) el.scrollTop = Math.max(0, row.top - 6);
    else if (row.top + row.h > bottom) el.scrollTop = row.top + row.h - el.clientHeight + 6;
  }, [selectedId, items]);

  if (!hasEntries) {
    if (totalEntries > 0) {
      return (
        <div className="empty">
          <span className="glyph">
            <SearchIcon size={17} />
          </span>
          <div>{t("No results for “{q}”", { q: query.trim() })}</div>
          <div style={{ opacity: 0.7 }}>{t("Try a different search term.")}</div>
        </div>
      );
    }
    return (
      <div className="empty">
        <span className="glyph">
          <KindIcon kind="text" />
        </span>
        <div>{t("No clipboard entries yet")}</div>
        <div style={{ opacity: 0.7 }}>{t("Copy something and it will show up here.")}</div>
      </div>
    );
  }

  const start = Math.max(0, scrollTop - viewH * OVERSCAN);
  const end = scrollTop + viewH * (1 + OVERSCAN);
  const visible = items.rows.filter((r) => r.top + r.h >= start && r.top <= end);

  return (
    <div className="vlist" ref={scrollRef}>
      <div className="vlist-inner" style={{ height: items.height }}>
        {visible.map((r) =>
          r.entry ? (
            <div className="vslot" style={{ top: r.top, height: r.h }} key={r.entry.id}>
              <EntryRow
                entry={r.entry}
                selected={r.entry.id === selectedId}
                order={
                  selection.includes(r.entry.id)
                    ? selection.indexOf(r.entry.id) + 1
                    : undefined
                }
                ocrHit={isOcrHit?.(r.entry)}
                renaming={r.entry.id === renamingId}
                onRename={(title, cancel) => onRename(r.entry!.id, title, cancel)}
                onHover={() => onHover(r.entry!.id)}
                onClick={(mods) => onSelect(r.entry!.id, mods)}
                onPaste={() => onPaste(r.entry!.id)}
                onContextMenu={() => onContextMenu(r.entry!.id)}
                t={t}
              />
            </div>
          ) : (
            <div className="vslot" style={{ top: r.top, height: r.h }} key={`l-${r.label}-${r.top}`}>
              <div className="section-label">{r.label}</div>
            </div>
          )
        )}
      </div>
    </div>
  );
}
