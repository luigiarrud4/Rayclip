import { useEffect, useRef, useState } from "react";

export type Option<T extends string | number = number> = { value: T; label: string };

// Um menu aberto por vez: abrir outro fecha o anterior.
const openMenus = new Set<() => void>();

/** Dropdown próprio (o <select> nativo abre um popup branco ilegível no WebView2). */
export function Dropdown<T extends string | number = number>({
  value,
  options,
  onChange,
  width = 132,
}: {
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
  width?: number;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const current = options.find((o) => o.value === value) ?? options[0];

  // Só reposiciona o item ativo ao ABRIR (options é recriado a cada render,
  // então sem essa guarda cada tecla era desfeita).
  const wasOpen = useRef(false);
  useEffect(() => {
    if (open && !wasOpen.current) {
      setActive(Math.max(0, options.findIndex((o) => o.value === value)));
    }
    wasOpen.current = open;
  }, [open, options, value]);

  // So um dropdown aberto por vez, e o body so perde a marca quando o
  // ULTIMO fecha: cada instancia escrevia dataset.menu = "" no proprio
  // fechamento e liberava o teclado da App com outro menu ainda aberto.
  const closeSelf = useRef(() => setOpen(false));
  useEffect(() => {
    if (!open) {
      document.body.dataset.menu = openMenus.size > 0 ? "1" : "";
      return;
    }
    for (const other of openMenus) {
      if (other !== closeSelf.current) other();
    }
    openMenus.add(closeSelf.current);
    document.body.dataset.menu = "1";
    const onDown = (e: MouseEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
      } else if (e.key === "ArrowDown") {
        setActive((i) => Math.min(options.length - 1, i + 1));
      } else if (e.key === "ArrowUp") {
        setActive((i) => Math.max(0, i - 1));
      } else if (e.key === "Enter") {
        // options[active] pode nao existir (lista vazia/mudou): crash antes.
        const opt = options[active];
        if (opt) onChange(opt.value);
        setOpen(false);
      } else {
        return;
      }
      e.preventDefault();
      e.stopImmediatePropagation();
    };
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      openMenus.delete(closeSelf.current);
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      document.body.dataset.menu = openMenus.size > 0 ? "1" : "";
    };
  }, [open, active, options, onChange, value]);

  return (
    <div className="dropdown" ref={root} style={{ width }}>
      <button
        type="button"
        className="dropdown-btn"
        data-open={open}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span>{current?.label}</span>
        <svg width="9" height="9" viewBox="0 0 12 12" aria-hidden>
          <path
            d="M3 4.5 6 7.5 9 4.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {open && (
        <div className="dropdown-menu" role="listbox">
          {options.map((o, i) => (
            <button
              key={o.value}
              type="button"
              role="option"
              aria-selected={o.value === value}
              className="dropdown-item"
              data-active={i === active}
              data-selected={o.value === value}
              onMouseEnter={() => setActive(i)}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
