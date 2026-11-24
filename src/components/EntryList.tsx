import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
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

/**
 * Payloads de arrasto já prontos, por id.
 *
 * Vive fora do componente porque o `dragstart` precisa do dado **no mesmo tick**
 * e porque o mouse passa por outras linhas durante o arrasto (o hover de cada uma
 * pediria um payload diferente). A URL é `blob:`, não `data:` — um `data:` de
 * centenas de KB faz o Chromium cancelar o arrasto na hora.
 */
type Arrasto = {
  name: string;
  mime: string;
  url: string;
  text: string | null;
  link: boolean;
};

/**
 * `true` enquanto um arrasto está em curso. A lista não pode ser re-renderizada
 * nesse momento: a linha sob o cursor pode mudar de identidade e o Chromium
 * cancela o arrasto no meio.
 */
let _arrastando = false;
export const arrastandoAgora = () => _arrastando;
let _payloads: Map<string, Arrasto> | null = null;
function payloadCache() {
  if (!_payloads) _payloads = new Map();
  return _payloads;
}

/** base64 → `blob:` URL do mesmo conteúdo. */
function blobUrl(base64: string, mime: string): string {
  const bin = atob(base64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: mime }));
}

/**
 * Arquivo que já está no disco → `file:///` URL.
 *
 * Sem isso um `.zip` de 25 MB era lido inteiro, virado base64 e baixado de volta
 * — sendo que o Explorer só precisa do caminho.
 */
function fileUrl(caminho: string): string {
  // `encodeURI`: espaço/acento no nome invalida o `file:` e o Chromium cancela o
  // arrasto no mesmo instante.
  return "file:///" + encodeURI(caminho.replace(/\\/g, "/"));
}

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
  // Arrasto para fora: o payload (nome + tipo + URL de dados) é buscado já no
  // `mousedown`/hover, porque o `dragstart` precisa dele **síncrono**. Sem
  // payload pronto, o arrasto é cancelado em vez de abrir um fantasma vazio.
  const arrastou = useRef(false);
  // Hover parado por 250 ms adianta o payload (inclusive de arquivos do disco)
  // sem ler 30 MB cada vez que o mouse atravessa a lista.
  const espera = useRef<number | null>(null);
  const aoEntrar = () => {
    onHover();
    if (espera.current !== null) window.clearTimeout(espera.current);
    espera.current = window.setTimeout(() => {
      espera.current = null;
      buscarPayload();
    }, 250);
  };
  const aoSair = () => {
    if (espera.current !== null) {
      window.clearTimeout(espera.current);
      espera.current = null;
    }
  };

  const buscarPayload = () => {
    const cache = payloadCache();
    if (cache.has(entry.id)) return;
    void api
      .dragPayload(entry.id)
      .then((p) => {
        const c = payloadCache();
        // Alguns poucos por vez: cada payload de imagem são centenas de KB.
        if (c.size > 6) {
          const velho = c.keys().next().value as string;
          URL.revokeObjectURL(c.get(velho)?.url ?? "");
          c.delete(velho);
        }
        c.set(entry.id, {
          name: p.name,
          mime: p.mime,
          url: blobUrl(p.data, p.mime),
          text: p.text,
          link: p.link,
        });
      })
      .catch(() => {});
  };

  return (
    <div
      className="row"
      data-selected={selected}
      data-multi={order ? "true" : undefined}
      data-id={entry.id}
      onMouseEnter={aoEntrar}
      onMouseLeave={aoSair}
      onMouseDown={() => {
        arrastou.current = false;
        buscarPayload();
      }}
      draggable
      onDragStart={(e) => {
        const p = payloadCache().get(entry.id);
        if (!p) {
          e.preventDefault();
          buscarPayload();
          return;
        }
        arrastou.current = true;
        _arrastando = true;
        e.dataTransfer.effectAllowed = "copy";
        // Receita do Chromium que vira `CF_HDROP` no Explorer/outros apps…
        e.dataTransfer.setData("DownloadURL", `${p.mime}:${p.name}:${p.url}`);
        // …e, para texto, também o texto puro: soltar no Notepad/Discord/Word
        // cola o conteúdo em vez de anexar um arquivo.
        if (p.text) {
          e.dataTransfer.setData("text/plain", p.text);
          if (p.link) e.dataTransfer.setData("text/uri-list", p.text.trim());
        }
        void api.dragStart();
      }}
      onDragEnd={() => {
        _arrastando = false;
        window.setTimeout(() => {
          arrastou.current = false;
        }, 60);
        void api.dragEnd();
      }}
      onClick={(e) => {
        // Um arrasto não deve também selecionar/colar.
        if (arrastou.current) {
          arrastou.current = false;
          return;
        }
        onClick({ ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey });
      }}
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
