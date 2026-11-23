import { useCallback, useEffect, useRef, useState } from "react";

/** Zoom mínimo: dá para **reduzir** abaixo de 100% (importante para prints grandes). */
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 8;
const STEP = 1.15;

function clamp(v: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, v));
}

/**
 * Imagem com zoom (scroll do mouse) e arrasto (pan).
 *
 * `armed = false` é o estado inicial do painel de detalhes: **o scroll não é
 * interceptado** (a coluna rola normal) e o primeiro clique "arma" o zoom.
 *
 * O `wheel` é registrado com `{ passive: false }` numa ref, e não via
 * `onWheel` do React: o React escuta wheel como passivo, então `preventDefault`
 * era ignorado e a coluna de detalhes rolava junto com o zoom (bug da v0.1.13).
 */
export function ZoomableImage({
  src,
  onZoom,
  armed = true,
  onArmedChange,
  onTap,
  fit = true,
  className,
  t = (s: string) => s,
}: {
  src: string;
  onZoom?: (zoom: number) => void;
  /** Quando `false`, o scroll não dá zoom até o usuário clicar na imagem. */
  armed?: boolean;
  onArmedChange?: (armed: boolean) => void;
  /** Clique simples (sem arrastar), já armado. */
  onTap?: () => void;
  fit?: boolean;
  className?: string;
  t?: (s: string) => string;
}) {
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);
  // Arrastar para mover a imagem não pode virar "clique".
  const moved = useRef(false);
  const [dragging, setDragging] = useState(false);
  const zoomRef = useRef(1);
  // pan precisa ser lido dentro do listener de wheel sem recriá-lo a cada movimento
  const panRef = useRef(pan);
  useEffect(() => {
    panRef.current = pan;
  }, [pan]);

  const setBoth = useCallback((z: number, p = { x: 0, y: 0 }) => {
    const nz = clamp(z, MIN_ZOOM, MAX_ZOOM);
    zoomRef.current = nz;
    setZoom(nz);
    setPan(nz === 1 ? { x: 0, y: 0 } : p);
  }, []);

  useEffect(() => {
    onZoom?.(zoom);
  }, [zoom, onZoom]);

  // Nova imagem / saiu do modo zoom: volta ao enquadramento.
  useEffect(() => {
    setBoth(1);
  }, [src, armed, setBoth]);

  // Wheel não-passivo: só intercepta quando o zoom está armado.
  useEffect(() => {
    const el = wrapRef.current;
    if (!el || !armed) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const factor = e.deltaY < 0 ? STEP : 1 / STEP;
      setBoth(zoomRef.current * factor, panRef.current);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [armed, setBoth]);

  // `Esc` sai do modo zoom (o app usaria o Esc para esconder a janela).
  useEffect(() => {
    if (!armed || !onArmedChange) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Com um campo de texto em foco (renomear, buscar) o Esc é dele.
      const el = document.activeElement as HTMLElement | null;
      const tag = el?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || el?.isContentEditable) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      setBoth(1);
      onArmedChange(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [armed, onArmedChange, setBoth]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (!armed || zoom <= 1) return;
    (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    moved.current = false;
    setDragging(true);
    drag.current = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag.current) return;
    const d = drag.current;
    if (Math.abs(e.clientX - d.x) > 3 || Math.abs(e.clientY - d.y) > 3) moved.current = true;
    setBoth(zoom, { x: d.px + (e.clientX - d.x), y: d.py + (e.clientY - d.y) });
  };
  const onPointerUp = () => {
    drag.current = null;
    setDragging(false);
  };
  const onClick = () => {
    if (moved.current) {
      moved.current = false;
      return;
    }
    if (!armed) {
      onArmedChange?.(true);
      return;
    }
    onTap?.();
  };

  return (
    <div
      ref={wrapRef}
      className={`zoomwrap${zoom > 1 ? " zoomed" : ""}${dragging ? " grabbing" : ""}${
        className ? ` ${className}` : ""
      }`}
      data-armed={armed}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onClick={onClick}
      onDoubleClick={() => setBoth(1)}
      title={
        !armed
          ? t("Click to zoom")
          : zoom > 1
            ? t("Scroll to zoom · drag to move · double-click to reset · Esc to exit")
            : t("Scroll to zoom · double-click to reset · Esc to exit")
      }
    >
      <img
        src={src}
        alt=""
        draggable={false}
        data-fit={fit || undefined}
        style={{
          transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
          transition: dragging ? "none" : "transform 90ms ease-out",
        }}
      />
      {zoom !== 1 && <span className="zoom-badge">{Math.round(zoom * 100)}%</span>}
      {armed && zoom === 1 && onArmedChange && (
        <span className="zoom-hint">{t("scroll to zoom · Esc to exit")}</span>
      )}
    </div>
  );
}

/**
 * Quick Look do app: card grande, centrado, **redimensionável** pelo canto,
 * com zoom no scroll (para dentro e para fora) e arrasto na imagem.
 * `Esc`/`Espaço` fecham.
 */
export function ImageViewer({
  src,
  title,
  gif,
  onClose,
  t,
}: {
  src: string;
  title: string;
  gif?: boolean;
  onClose: () => void;
  t: (s: string) => string;
}) {
  const [size, setSize] = useState(() => ({
    w: Math.min(720, Math.max(360, window.innerWidth - 120)),
    h: Math.min(400, Math.max(260, window.innerHeight - 140)),
  }));
  const [zoom, setZoom] = useState(1);
  const resize = useRef<{ x: number; y: number; w: number; h: number } | null>(null);

  // Esc/Espaço fecham **aqui** e não vazam para os atalhos do app.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === " " || e.code === "Space") {
        e.preventDefault();
        e.stopImmediatePropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const onResizeMove = (e: React.PointerEvent) => {
    if (!resize.current) return;
    const d = resize.current;
    setSize({
      w: clamp(d.w + (e.clientX - d.x), 320, window.innerWidth - 24),
      h: clamp(d.h + (e.clientY - d.y), 240, window.innerHeight - 24),
    });
  };

  return (
    <div className="viewer-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        className="viewer-card"
        style={{ width: size.w, height: size.h }}
        role="dialog"
        aria-label={t("Image preview")}
      >
        <div className="viewer-head">
          <span className="viewer-title">{title}</span>
          <span className="viewer-keys">
            {gif ? "GIF · " : ""}
            {t(
              "Scroll to zoom · drag to move · corner to resize · Esc to close"
            )}
          </span>
        </div>
        <div className="viewer-body">
          <ZoomableImage src={src} onZoom={setZoom} />
        </div>
        <div className="viewer-foot">
          <span>{Math.round(zoom * 100)}%</span>
          <span className="viewer-hint">{t("Space or Esc to close")}</span>
        </div>
        <div
          className="viewer-resize"
          title={t("Drag to resize")}
          onPointerDown={(e) => {
            e.preventDefault();
            (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
            resize.current = { x: e.clientX, y: e.clientY, w: size.w, h: size.h };
          }}
          onPointerMove={onResizeMove}
          onPointerUp={() => {
            resize.current = null;
          }}
        />
      </div>
    </div>
  );
}
